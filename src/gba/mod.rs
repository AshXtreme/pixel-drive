#![allow(dead_code)]

pub mod arm;
pub mod bios;
pub mod cpu;
pub mod dma;
pub mod keypad;
pub mod libretro;
pub mod mmu;
pub mod ppu;
pub mod thumb;

use crate::core::{Button, EmulatorCore};
pub use cpu::{Cpu, CpuMode};
pub use libretro::LibretroCore;
use log::{info, warn};
pub use mmu::GbaMemoryBus;
use std::path::Path;

pub const GBA_WIDTH: u32 = 240;
pub const GBA_HEIGHT: u32 = 160;
pub const GBA_CYCLES_PER_FRAME: usize = 280_896; // 16.78 MHz / 59.73 FPS

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbaHeader {
    pub title: String,
    pub game_code: String,
    pub maker_code: String,
    pub version: u8,
    pub is_valid: bool,
}

impl GbaHeader {
    /// Parse and validate standard 32-bit GBA ROM header structure (192+ bytes)
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0xC0 {
            return None;
        }

        // Nintendo GBA Header magic byte at offset 0xB2 must be 0x96
        if bytes[0x0B2] != 0x96 {
            return None;
        }

        let title = String::from_utf8_lossy(&bytes[0xA0..0xAC])
            .trim_matches('\0')
            .trim()
            .to_string();

        let game_code = String::from_utf8_lossy(&bytes[0xAC..0xB0])
            .trim_matches('\0')
            .trim()
            .to_string();

        let maker_code = String::from_utf8_lossy(&bytes[0xB0..0xB2])
            .trim_matches('\0')
            .trim()
            .to_string();

        let version = bytes[0xBC];

        Some(Self {
            title,
            game_code,
            maker_code,
            version,
            is_valid: true,
        })
    }
}

pub struct GbaCore {
    pub cpu: Cpu,
    pub mmu: GbaMemoryBus,
    pub header: Option<GbaHeader>,
    pub libretro: Option<LibretroCore>,
}

impl Default for GbaCore {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaCore {
    pub fn new() -> Self {
        let libretro = if let Some(core_path) = libretro::find_available_core() {
            match LibretroCore::load(&core_path) {
                Ok(core) => {
                    info!("GbaCore: Active backend -> Libretro ({})", core.library_name);
                    Some(core)
                }
                Err(err) => {
                    warn!(
                        "GbaCore: Failed to initialize Libretro core at {}: {}. Falling back to Built-in Rust core.",
                        core_path.display(),
                        err
                    );
                    None
                }
            }
        } else {
            info!("GbaCore: No dynamic Libretro core found in ./cores/ or exe dir. Using Built-in Rust core.");
            None
        };

        let mut core = Self {
            cpu: Cpu::new(),
            mmu: GbaMemoryBus::new(),
            header: None,
            libretro,
        };
        core.reset_boot_state();
        core
    }

    /// Reset CPU registers and CPSR flags to hardware default boot state.
    pub fn reset_boot_state(&mut self) {
        self.cpu.regs.reset();
        self.mmu.reset();
        let has_real = self.mmu.check_and_load_bios();

        if has_real {
            self.cpu.regs.set_pc(0x00000000);
            self.cpu.regs.set_thumb_mode(false); // ARM Mode
            self.cpu.regs.set_mode(CpuMode::Supervisor); // Supervisor Mode (0x13)
        } else {
            self.cpu.regs.set_pc(0x08000000);
            self.cpu.regs.set_sp(0x03007F00); // IWRAM Stack Pointer
            self.cpu.regs.set_mode(CpuMode::System); // System Mode (0x1F)
            self.cpu.regs.set_thumb_mode(false); // ARM Mode
        }
    }

    /// Load raw ROM byte buffer into GBA MMU memory space and reset CPU boot state.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        self.header = GbaHeader::parse(rom_bytes);
        self.mmu.load_rom(rom_bytes);
        self.reset_boot_state();

        if let Some(ref mut lr) = self.libretro {
            let loaded = lr.load_rom(rom_bytes);
            if !loaded {
                warn!("LibretroCore failed to load ROM. Emulation may fallback or encounter issues.");
            }
        }

        if let Some(ref header) = self.header {
            info!(
                "Loaded GBA ROM into MMU: '{}' [GameCode: {}, Maker: {}] ({} bytes). Initial PC: 0x{:08X}",
                header.title,
                header.game_code,
                header.maker_code,
                rom_bytes.len(),
                self.cpu.regs.r[15]
            );
        } else {
            info!(
                "Loaded raw GBA ROM into MMU ({} bytes). Initial PC: 0x{:08X}",
                rom_bytes.len(),
                self.cpu.regs.r[15]
            );
        }
    }

    /// Load a `.gba` or compressed `.zip` GBA ROM file from disk into memory, verifying header signature.
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<GbaHeader> {
        let path_ref = path.as_ref();
        info!("Loading GBA ROM file: {}", path_ref.display());

        let bytes = if path_ref
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            info!("Extracting GBA ROM from ZIP archive: {}", path_ref.display());
            let file = std::fs::File::open(path_ref)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut rom_bytes = None;

            for i in 0..archive.len() {
                let mut file_entry = archive.by_index(i)?;
                let name = file_entry.name().to_lowercase();
                if name.ends_with(".gba") {
                    info!("Found GBA ROM entry in ZIP: {}", file_entry.name());
                    let mut buffer = Vec::new();
                    std::io::Read::read_to_end(&mut file_entry, &mut buffer)?;
                    rom_bytes = Some(buffer);
                    break;
                }
            }

            match rom_bytes {
                Some(b) => b,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No valid .gba ROM file found inside ZIP archive",
                    ));
                }
            }
        } else {
            std::fs::read(path_ref)?
        };

        let header = GbaHeader::parse(&bytes).unwrap_or_else(|| {
            log::warn!("GBA ROM missing standard header magic byte (0x96). Using fallback header.");
            let title_str = path_ref
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("HOMEBREW");
            GbaHeader {
                title: title_str.chars().take(12).collect(),
                game_code: "HOME".to_string(),
                maker_code: "00".to_string(),
                version: 0,
                is_valid: false,
            }
        });

        self.load_rom(&bytes);
        Ok(header)
    }
}

impl EmulatorCore for GbaCore {
    fn step_frame(&mut self) {
        if let Some(ref mut lr) = self.libretro {
            lr.step_frame();
            return;
        }

        let mut cycles_this_frame = 0;
        while cycles_this_frame < GBA_CYCLES_PER_FRAME {
            let cycles = self.cpu.step(&mut self.mmu);
            self.mmu.step_timers(cycles);
            self.mmu.ppu.step(cycles);

            if self.mmu.ppu.vblank_irq_requested {
                self.mmu.ppu.vblank_irq_requested = false;

                let current_if = self.mmu.read_u16(0x04000202);
                let new_if = current_if | 1;
                self.mmu.io[0x202] = new_if as u8;
                self.mmu.io[0x203] = (new_if >> 8) as u8;

                let check = self.mmu.read_u16(0x03007FF8);
                self.mmu.write_u16(0x03007FF8, check | 1);
            }

            let ime = self.mmu.read_u32(0x04000208) & 1 != 0;
            if ime && !self.cpu.regs.irq_disabled() {
                let ie = self.mmu.read_u16(0x04000200);
                let if_flags = self.mmu.read_u16(0x04000202);
                if (ie & if_flags) != 0 {
                    self.cpu.trigger_irq();
                }
            }

            cycles_this_frame += cycles;
        }
    }

    fn framebuffer(&self) -> &[u8] {
        if let Some(ref lr) = self.libretro {
            lr.framebuffer()
        } else {
            self.mmu.ppu.framebuffer()
        }
    }

    fn display_dimensions(&self) -> (u32, u32) {
        if let Some(ref lr) = self.libretro {
            lr.dimensions()
        } else {
            (GBA_WIDTH, GBA_HEIGHT)
        }
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!("GBA Input: {:?} -> {}", button, if pressed { "Pressed" } else { "Released" });
        if let Some(ref mut lr) = self.libretro {
            let retro_id = match button {
                Button::A => libretro::RETRO_DEVICE_ID_JOYPAD_A,
                Button::B => libretro::RETRO_DEVICE_ID_JOYPAD_B,
                Button::Select => libretro::RETRO_DEVICE_ID_JOYPAD_SELECT,
                Button::Start => libretro::RETRO_DEVICE_ID_JOYPAD_START,
                Button::Right => libretro::RETRO_DEVICE_ID_JOYPAD_RIGHT,
                Button::Left => libretro::RETRO_DEVICE_ID_JOYPAD_LEFT,
                Button::Up => libretro::RETRO_DEVICE_ID_JOYPAD_UP,
                Button::Down => libretro::RETRO_DEVICE_ID_JOYPAD_DOWN,
                Button::L => libretro::RETRO_DEVICE_ID_JOYPAD_L,
                Button::R => libretro::RETRO_DEVICE_ID_JOYPAD_R,
            };
            lr.set_key_state(retro_id, pressed);
        }
        self.mmu.keypad.handle_input(button, pressed);
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        if let Some(ref mut lr) = self.libretro {
            lr.drain_audio()
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn create_dummy_gba_rom(title: &str, game_code: &str) -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000]; // 4KB dummy ROM
        // 0x000 - ARM jump instruction dummy (B +0x2E -> offset +0x24)
        rom[0..4].copy_from_slice(&[0x09, 0x00, 0x00, 0xEA]); // B instruction
        // 0xB2 - GBA Magic byte
        rom[0x0B2] = 0x96;

        // Title at 0xA0..0xAC
        let title_bytes = title.as_bytes();
        let t_len = title_bytes.len().min(12);
        rom[0xA0..0xA0 + t_len].copy_from_slice(&title_bytes[..t_len]);

        // Game code at 0xAC..0xB0
        let code_bytes = game_code.as_bytes();
        let c_len = code_bytes.len().min(4);
        rom[0xAC..0xAC + c_len].copy_from_slice(&code_bytes[..c_len]);

        rom
    }

    #[test]
    fn test_gba_header_parse() {
        let rom = create_dummy_gba_rom("POKEMON FIRE", "BPRE");
        let header = GbaHeader::parse(&rom).expect("Header should parse");
        assert_eq!(header.title, "POKEMON FIRE");
        assert_eq!(header.game_code, "BPRE");
        assert!(header.is_valid);
    }

    #[test]
    fn test_invalid_gba_header() {
        let rom = vec![0u8; 0x1000]; // Magic byte missing
        assert!(GbaHeader::parse(&rom).is_none());
    }

    #[test]
    fn test_load_rom_file_and_cpu_boot_state() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let rom_path = temp_dir.join("test_game.gba");
        let rom_bytes = create_dummy_gba_rom("ZELDA MINISH", "BZME");
        std::fs::write(&rom_path, &rom_bytes)?;

        let mut core = GbaCore::new();
        let header = core.load_rom_file(&rom_path)?;
        assert_eq!(header.title, "ZELDA MINISH");
        assert_eq!(header.game_code, "BZME");
        assert_eq!(core.cpu.regs.r[15], 0x08000000); // Initial PC
        assert_eq!(core.cpu.regs.r[13], 0x03007F00); // Initial SP
        assert_eq!(core.cpu.regs.mode(), CpuMode::System);

        let _ = std::fs::remove_file(rom_path);
        Ok(())
    }

    #[test]
    fn test_load_zip_gba() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join("test_gba.zip");
        let rom_bytes = create_dummy_gba_rom("METROID FUS", "AMFE");

        {
            let file = std::fs::File::create(&zip_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("game.gba", SimpleFileOptions::default())?;
            zip.write_all(&rom_bytes)?;
            zip.finish()?;
        }

        let mut core = GbaCore::new();
        let header = core.load_rom_file(&zip_path)?;
        assert_eq!(header.title, "METROID FUS");
        assert_eq!(header.game_code, "AMFE");

        let _ = std::fs::remove_file(zip_path);
        Ok(())
    }

    #[test]
    fn test_gba_step_frame_execution() {
        let mut core = GbaCore::new();
        let rom = create_dummy_gba_rom("TEST RUN", "TEST");
        core.load_rom(&rom);
        assert_eq!(core.cpu.regs.r[15], 0x08000000);

        core.step_frame();
        if core.libretro.is_some() {
            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
        } else {
            assert!(core.cpu.regs.r[15] > 0x08000000);
            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
        }
    }

    #[test]
    fn test_pokemon_firered_execution() {
        let rom_path = "/Users/ashutoshsamal/Downloads/Pokemon_Fire_Red_1[romsretro.com]/Pokemon - FireRed Version (USA, Europe).gba";
        if !std::path::Path::new(rom_path).exists() {
            println!("ROM PATH DOES NOT EXIST: {}", rom_path);
            return;
        }

        let mut core = GbaCore::new();
        let header = core.load_rom_file(rom_path).unwrap();
        println!("Loaded ROM title: {}", header.title);

        println!("--- RUNNING 300 FRAMES OF POKEMON FIRE RED ---");
        for _frame in 0..300 {
            core.step_frame();
        }

        println!("--- RUNNING 120 FRAMES OF POKEMON FIRE RED ---");
        for frame in 0..120 {
            core.step_frame();
            if frame % 30 == 0 {
                println!(
                    "Frame {:3}: PC=0x{:08X} DISPCNT=0x{:04X}",
                    frame, core.cpu.regs.pc(), core.mmu.ppu.dispcnt
                );
            }
        }

        if core.libretro.is_some() {
            println!("Pokemon FireRed verified on Libretro core backend.");
            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
        } else {
            assert!(core.cpu.regs.pc() != 0x08000000);
            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
        }
    }

    #[test]
    fn test_real_bios_loading_and_hle_fallback() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let bios_path = temp_dir.join("gba_bios.bin");

        // 1. Without gba_bios.bin, core defaults to HLE (PC = 0x08000000)
        let core_hle = GbaCore::new();
        assert!(!core_hle.mmu.has_real_bios);
        assert_eq!(core_hle.cpu.regs.pc(), 0x08000000);
        assert_eq!(core_hle.cpu.regs.mode(), CpuMode::System);

        // 2. Create dummy 16KB gba_bios.bin
        let dummy_bios = vec![0xEA; 16384];
        std::fs::write(&bios_path, &dummy_bios)?;

        let mut bus = GbaMemoryBus::new();
        bus.load_bios(&dummy_bios);
        bus.has_real_bios = true;

        assert!(bus.has_real_bios);
        assert_eq!(bus.bios[0], 0xEA);

        let _ = std::fs::remove_file(bios_path);
        Ok(())
    }
}
