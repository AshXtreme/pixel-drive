pub mod cpu;
pub mod joypad;
pub mod mmu;
pub mod ppu;

use crate::core::{Button, EmulatorCore};
use cpu::Cpu;
use log::info;
use mmu::MemoryBus;
use ppu::Ppu;
use std::path::Path;

pub const GBC_WIDTH: u32 = 160;
pub const GBC_HEIGHT: u32 = 144;
pub const GBC_CYCLES_PER_FRAME: u32 = 70_224; // 4.194304 MHz / ~59.73 FPS

pub struct GbcCore {
    pub cpu: Cpu,
    pub mmu: MemoryBus,
    pub ppu: Ppu,
    frame_count: u32,
}

impl GbcCore {
    pub fn new() -> Self {
        Self {
            cpu: Cpu::new(),
            mmu: MemoryBus::new(),
            ppu: Ppu::new(),
            frame_count: 0,
        }
    }

    /// Load raw ROM byte buffer into memory bus and reset core state.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        info!("Loaded {} bytes into GBC MMU. Resetting CPU, PPU, and Memory state.", rom_bytes.len());
        self.cpu = Cpu::new();
        self.ppu = Ppu::new();
        let mut new_mmu = MemoryBus::new();
        new_mmu.load_rom(rom_bytes);
        self.mmu = new_mmu;
    }

    /// Load a .gb / .gbc ROM file from disk into memory.
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let path_ref = path.as_ref();
        info!("Loading ROM file into GBC Core: {}", path_ref.display());
        let bytes = std::fs::read(path_ref)?;
        self.load_rom(&bytes);
        Ok(())
    }
}

impl EmulatorCore for GbcCore {
    fn step_frame(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);

        let mut cycles_this_frame: u32 = 0;
        while cycles_this_frame < GBC_CYCLES_PER_FRAME {
            let cycles = self.cpu.step(&mut self.mmu);
            self.ppu.step(cycles, &mut self.mmu);
            cycles_this_frame = cycles_this_frame.saturating_add(cycles as u32);
        }
    }

    fn framebuffer(&self) -> &[u8] {
        self.ppu.framebuffer()
    }

    fn display_dimensions(&self) -> (u32, u32) {
        (GBC_WIDTH, GBC_HEIGHT)
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!("GBC Input: {:?} -> {}", button, if pressed { "Pressed" } else { "Released" });
        self.mmu.joypad.handle_input(button, pressed);
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        Vec::new()
    }
}


