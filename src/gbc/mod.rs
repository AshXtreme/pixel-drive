pub mod cpu;
pub mod mmu;

use crate::core::{Button, EmulatorCore};
use cpu::Cpu;
use log::info;
use mmu::MemoryBus;
use std::path::Path;

pub const GBC_WIDTH: u32 = 160;
pub const GBC_HEIGHT: u32 = 144;
pub const GBC_CYCLES_PER_FRAME: u32 = 70_224; // 4.194304 MHz / ~59.73 FPS

pub struct GbcCore {
    pub cpu: Cpu,
    pub mmu: MemoryBus,
    framebuffer: Vec<u8>,
    frame_count: u32,
}

impl GbcCore {
    pub fn new() -> Self {
        let size = (GBC_WIDTH * GBC_HEIGHT * 4) as usize;
        let mut core = Self {
            cpu: Cpu::new(),
            mmu: MemoryBus::new(),
            framebuffer: vec![0; size],
            frame_count: 0,
        };
        core.update_test_pattern();
        core
    }

    /// Load raw ROM byte buffer into memory bus.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        info!("Loaded {} bytes into GBC MMU.", rom_bytes.len());
        self.mmu.load_rom(rom_bytes);
    }

    /// Load a .gb / .gbc ROM file from disk into memory.
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let path_ref = path.as_ref();
        info!("Loading ROM file into GBC Core: {}", path_ref.display());
        let bytes = std::fs::read(path_ref)?;
        self.load_rom(&bytes);
        Ok(())
    }

    fn update_test_pattern(&mut self) {
        let t = self.frame_count as f32 * 0.05;
        for y in 0..GBC_HEIGHT {
            for x in 0..GBC_WIDTH {
                let idx = ((y * GBC_WIDTH + x) * 4) as usize;
                let r = ((x as f32 * 0.1 + t).sin() * 127.0 + 128.0) as u8;
                let g = ((y as f32 * 0.1 + t * 1.5).cos() * 127.0 + 128.0) as u8;
                let b = (((x + y) as f32 * 0.05 + t * 2.0).sin() * 127.0 + 128.0) as u8;

                self.framebuffer[idx] = r;
                self.framebuffer[idx + 1] = g;
                self.framebuffer[idx + 2] = b;
                self.framebuffer[idx + 3] = 255;
            }
        }
    }
}

impl EmulatorCore for GbcCore {
    fn step_frame(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);

        let mut cycles_this_frame: u32 = 0;
        while cycles_this_frame < GBC_CYCLES_PER_FRAME {
            let cycles = self.cpu.step(&mut self.mmu);
            cycles_this_frame = cycles_this_frame.saturating_add(cycles as u32);
        }

        self.update_test_pattern();
    }

    fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    fn display_dimensions(&self) -> (u32, u32) {
        (GBC_WIDTH, GBC_HEIGHT)
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!("GBC Input: {:?} -> {}", button, if pressed { "Pressed" } else { "Released" });
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        Vec::new()
    }
}


