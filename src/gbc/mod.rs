pub mod apu;
pub mod cpu;
pub mod joypad;
pub mod mbc;
pub mod mmu;
pub mod ppu;
pub mod timer;

use crate::audio::AudioProducer;
use crate::core::{Button, EmulatorCore};
use cpu::Cpu;
use log::info;
use mmu::MemoryBus;
use ppu::Ppu;
use std::path::Path;
use timer::Timer;

pub const GBC_WIDTH: u32 = 160;
pub const GBC_HEIGHT: u32 = 144;
pub const GBC_CYCLES_PER_FRAME: u32 = 70_224; // 4.194304 MHz / ~59.73 FPS

pub struct GbcCore {
    pub cpu: Cpu,
    pub mmu: MemoryBus,
    pub ppu: Ppu,
    pub timer: Timer,
    pub is_rom_loaded: bool,
    frame_count: u32,
    audio_producer: Option<AudioProducer>,
    pub rom_path: Option<std::path::PathBuf>,
}

pub const MAX_GBC_ROM_SIZE: usize = 8 * 1024 * 1024; // 8 MB max
pub const MAX_GBC_STATE_SIZE: usize = 16 * 1024 * 1024; // 16 MB max

impl Default for GbcCore {
    fn default() -> Self {
        Self::new()
    }
}

impl GbcCore {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            mmu: MemoryBus::new(),
            ppu: Ppu::new(),
            timer: Timer::new(),
            is_rom_loaded: false,
            frame_count: 0,
            audio_producer: None,
            rom_path: None,
        }
    }

    /// Attach audio sample producer.
    pub fn set_audio_producer(&mut self, producer: Option<AudioProducer>) {
        self.mmu.apu.set_audio_producer(producer.clone());
        self.audio_producer = producer;
    }

    /// Load raw ROM byte buffer into memory bus and reset core state.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        info!(
            "Loaded {} bytes into GBC MMU. Resetting CPU, PPU, Timer, and Memory state.",
            rom_bytes.len()
        );
        self.cpu = Cpu::new();
        self.ppu = Ppu::new();
        self.timer = Timer::new();
        let mut new_mmu = MemoryBus::new();
        new_mmu.load_rom(rom_bytes);

        if new_mmu.is_gbc {
            self.cpu.registers.init_gbc_defaults();
        }

        if let Some(ref prod) = self.audio_producer {
            new_mmu.apu.set_audio_producer(Some(prod.clone()));
        }

        self.mmu = new_mmu;
        self.is_rom_loaded = true;
    }

    /// Loads a ROM directly from a filesystem path, unpacking `.zip` archives if necessary.
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let path_ref = path.as_ref();
        info!("Loading ROM file into GBC Core: {}", path_ref.display());

        let bytes = if path_ref
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            info!("Extracting ROM from ZIP archive: {}", path_ref.display());
            let file = std::fs::File::open(path_ref)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut rom_bytes = None;

            for i in 0..archive.len() {
                let mut file_entry = archive.by_index(i)?;
                let name = file_entry.name().to_lowercase();
                if name.ends_with(".gb") || name.ends_with(".gbc") {
                    info!("Found ROM entry in ZIP: {}", file_entry.name());
                    let mut buffer = Vec::new();
                    std::io::Read::read_to_end(
                        &mut std::io::Read::take(&mut file_entry, (MAX_GBC_ROM_SIZE + 1) as u64),
                        &mut buffer,
                    )?;
                    if buffer.len() > MAX_GBC_ROM_SIZE {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "GBC ROM inside ZIP archive exceeds maximum allowed size (8MB)",
                        ));
                    }
                    rom_bytes = Some(buffer);
                    break;
                }
            }

            match rom_bytes {
                Some(b) => b,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No valid .gb or .gbc ROM file found inside ZIP archive",
                    ));
                }
            }
        } else {
            let data = std::fs::read(path_ref)?;
            if data.len() > MAX_GBC_ROM_SIZE {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "GBC ROM file exceeds maximum allowed size (8MB)",
                ));
            }
            data
        };

        self.load_rom(&bytes);
        self.rom_path = Some(path_ref.to_path_buf());
        Ok(())
    }

    /// Serializes full real-time GBC simulation state into a binary byte buffer.
    pub fn save_state(&self) -> Result<Vec<u8>, bincode::Error> {
        let state = GbcState {
            cpu: self.cpu.clone(),
            mmu: self.mmu.clone(),
            ppu: self.ppu.clone(),
            timer: self.timer.clone(),
            frame_count: self.frame_count,
            is_rom_loaded: self.is_rom_loaded,
        };
        bincode::serialize(&state)
    }

    /// Deserializes full real-time GBC simulation state from a binary byte buffer.
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), bincode::Error> {
        if data.len() > MAX_GBC_STATE_SIZE {
            return Err(bincode::ErrorKind::Custom(
                "Save state exceeds maximum allowable size (16MB)".to_string(),
            )
            .into());
        }
        let state: GbcState = bincode::deserialize(data)?;
        self.cpu = state.cpu;
        self.mmu = state.mmu;
        self.ppu = state.ppu;
        self.timer = state.timer;
        self.frame_count = state.frame_count;
        self.is_rom_loaded = state.is_rom_loaded;
        if let Some(ref prod) = self.audio_producer {
            self.mmu.apu.set_audio_producer(Some(prod.clone()));
        }
        Ok(())
    }
}

/// Serializable state container for GbcCore snapshots.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GbcState {
    pub cpu: Cpu,
    pub mmu: MemoryBus,
    pub ppu: Ppu,
    pub timer: Timer,
    pub frame_count: u32,
    pub is_rom_loaded: bool,
}

impl EmulatorCore for GbcCore {
    fn step_frame(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);

        if !self.is_rom_loaded {
            self.ppu.draw_splash_pattern(self.frame_count);
            return;
        }

        let is_double_speed = self.mmu.is_double_speed();
        let max_cycles = if is_double_speed {
            GBC_CYCLES_PER_FRAME * 2
        } else {
            GBC_CYCLES_PER_FRAME
        };

        let mut cycles_this_frame: u32 = 0;
        let mut dot_remainder: u8 = 0;

        while cycles_this_frame < max_cycles {
            let cpu_cycles = self.cpu.step(&mut self.mmu);
            self.timer.step(cpu_cycles, &mut self.mmu);

            let dot_cycles = if is_double_speed {
                let total = cpu_cycles + dot_remainder;
                dot_remainder = total & 1;
                total >> 1
            } else {
                cpu_cycles
            };

            if dot_cycles > 0 {
                self.ppu.step(dot_cycles, &mut self.mmu);
                self.mmu.apu.step(dot_cycles);
            }

            cycles_this_frame = cycles_this_frame.saturating_add(cpu_cycles as u32);
        }
    }

    fn framebuffer(&self) -> &[u8] {
        self.ppu.framebuffer()
    }

    fn display_dimensions(&self) -> (u32, u32) {
        (GBC_WIDTH, GBC_HEIGHT)
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!(
            "GBC Input: {:?} -> {}",
            button,
            if pressed { "Pressed" } else { "Released" }
        );
        self.mmu.joypad.handle_input(button, pressed);
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        self.mmu.apu.drain_audio()
    }

    fn get_save_data(&self) -> Option<&[u8]> {
        self.mmu.mbc.get_ram()
    }

    fn load_save_data(&mut self, data: &[u8]) -> bool {
        self.mmu.mbc.load_ram(data);
        true
    }

    fn save_path(&self) -> Option<std::path::PathBuf> {
        self.rom_path
            .as_ref()
            .map(|p| crate::save::SaveManager::get_save_path(p))
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        self.save_state().ok()
    }

    fn load_state(&mut self, data: &[u8]) -> bool {
        self.load_state(data).is_ok()
    }

    fn reset(&mut self) {
        if self.is_rom_loaded {
            let rom = self.mmu.mbc.get_rom().to_vec();
            let ram = self.mmu.mbc.get_ram().map(|r| r.to_vec());
            self.load_rom(&rom);
            if let Some(sram) = ram {
                self.load_save_data(&sram);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    #[test]
    fn test_load_rom_bytes() {
        let mut core = GbcCore::new();
        let rom = vec![0x00, 0xAF, 0xC3, 0x00, 0x01];
        core.load_rom(&rom);
        assert_eq!(core.mmu.read_byte(0x0000), 0x00);
        assert_eq!(core.mmu.read_byte(0x0001), 0xAF);
    }

    #[test]
    fn test_load_zip_archive() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join("test_game.zip");

        {
            let file = std::fs::File::create(&zip_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("game.gbc", SimpleFileOptions::default())?;
            zip.write_all(&[0x00, 0xAF, 0x00, 0x18])?;
            zip.finish()?;
        }

        let mut core = GbcCore::new();
        core.load_rom_file(&zip_path)?;
        assert_eq!(core.mmu.read_byte(0x0001), 0xAF);

        let _ = std::fs::remove_file(zip_path);
        Ok(())
    }

    #[test]
    fn test_pokemon_yellow_execution() {
        let rom_path = "/Users/ashutoshsamal/Downloads/Pokemon - Yellow Version (UE) [C][!].gbc";
        if !std::path::Path::new(rom_path).exists() {
            return;
        }

        let mut core = GbcCore::new();
        core.load_rom_file(rom_path).unwrap();
        assert!(core.mmu.is_gbc);
        assert_eq!(core.cpu.registers.a, 0x11);

        // Step 120 frames (~2 seconds of execution)
        for _ in 0..120 {
            core.step_frame();
        }

        assert!(core.cpu.registers.pc != 0x0100);
        let active_pixels = core
            .ppu
            .framebuffer()
            .chunks(4)
            .filter(|p| p[0] != 255 || p[1] != 255 || p[2] != 255)
            .count();
        assert!(active_pixels > 0);
    }

    #[test]
    fn test_pokemon_crystal_execution() {
        let rom_path =
            "/Users/ashutoshsamal/Downloads/Pokemon - Crystal Version (USA, Europe) (Rev A).zip";
        if !std::path::Path::new(rom_path).exists() {
            return;
        }

        let mut core = GbcCore::new();
        core.load_rom_file(rom_path).unwrap();
        assert!(core.mmu.is_gbc);
        assert_eq!(core.cpu.registers.a, 0x11);

        // Step 240 frames and trigger inputs
        for f in 0..240 {
            if f == 60 {
                core.handle_input(Button::Start, true);
            } else if f == 70 {
                core.handle_input(Button::Start, false);
            } else if f == 120 {
                core.handle_input(Button::A, true);
            } else if f == 130 {
                core.handle_input(Button::A, false);
            }
            core.step_frame();
        }

        assert!(core.cpu.registers.pc != 0x0100);
        // Verify VRAM received font and map data via HDMA / GDMA
        let vram_non_zero = (0x8000..0x9FFF)
            .filter(|&a| core.mmu.read_byte(a) != 0)
            .count();
        assert!(
            vram_non_zero > 0,
            "VRAM should contain tile data loaded by HDMA"
        );

        let active_pixels = core
            .ppu
            .framebuffer()
            .chunks(4)
            .filter(|p| p[0] != 255 || p[1] != 255 || p[2] != 255)
            .count();
        assert!(active_pixels > 0, "Framebuffer should have rendered pixels");

        let audio = core.audio_buffer();
        assert!(!audio.is_empty(), "GBC Core should produce audio samples");
    }

    #[test]
    fn test_gbc_save_and_load_state() {
        let mut core = GbcCore::new();
        let rom = vec![0x00, 0xAF, 0xC3, 0x00, 0x01];
        core.load_rom(&rom);
        core.cpu.registers.pc = 0x1234;
        core.cpu.registers.a = 0xFE;
        core.frame_count = 999;

        // Serialize state
        let state_bytes = core
            .save_state()
            .expect("State serialization should succeed");
        assert!(!state_bytes.is_empty());

        // Mutate core state
        core.cpu.registers.pc = 0x0000;
        core.cpu.registers.a = 0x00;
        core.frame_count = 0;

        // Deserialize state
        core.load_state(&state_bytes)
            .expect("State deserialization should succeed");
        assert_eq!(core.cpu.registers.pc, 0x1234);
        assert_eq!(core.cpu.registers.a, 0xFE);
        assert_eq!(core.frame_count, 999);
    }

    #[test]
    fn test_gbc_save_state_disk_persistence_across_restart() {
        let rom_stem = "TestGbc_DiskRestart";
        let slot = 1;

        // Clean up any pre-existing test state file
        let state_path = crate::save::SaveManager::get_state_path_from_stem(rom_stem, slot);
        let _ = std::fs::remove_file(&state_path);

        // 1. Initial run: Capture and save state to disk
        {
            let mut core = GbcCore::new();
            let rom = vec![0x00, 0xAF, 0xC3, 0x00, 0x01];
            core.load_rom(&rom);
            core.cpu.registers.pc = 0x5678;
            core.cpu.registers.a = 0x42;
            core.frame_count = 12345;

            let state_data = core.save_state().expect("State snapshot should succeed");
            crate::save::SaveManager::save_state_to_disk(rom_stem, slot, &state_data)
                .expect("Saving state to disk should succeed");

            assert!(crate::save::SaveManager::state_exists_on_disk(
                rom_stem, slot
            ));
        } // `core` is dropped here, simulating app exit

        // 2. Restart run: Create fresh new core and restore state from disk
        {
            let mut new_core = GbcCore::new();
            let rom = vec![0x00, 0xAF, 0xC3, 0x00, 0x01];
            new_core.load_rom(&rom);

            // Fresh core defaults
            assert_ne!(new_core.cpu.registers.pc, 0x5678);

            // Read state from disk and restore
            let loaded_data = crate::save::SaveManager::load_state_from_disk(rom_stem, slot)
                .expect("Should load state from disk after restart");
            new_core
                .load_state(&loaded_data)
                .expect("Should deserialize state");

            assert_eq!(new_core.cpu.registers.pc, 0x5678);
            assert_eq!(new_core.cpu.registers.a, 0x42);
            assert_eq!(new_core.frame_count, 12345);
        }

        // Clean up
        let _ = std::fs::remove_file(&state_path);
    }
}
