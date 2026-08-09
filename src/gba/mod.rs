#![allow(dead_code)]

use crate::core::{Button, EmulatorCore};
use log::info;

pub const GBA_WIDTH: u32 = 240;
pub const GBA_HEIGHT: u32 = 160;

pub struct GbaCore {
    framebuffer: Vec<u8>,
}

impl GbaCore {
    pub fn new() -> Self {
        let size = (GBA_WIDTH * GBA_HEIGHT * 4) as usize;
        Self {
            framebuffer: vec![0; size],
        }
    }
}

impl EmulatorCore for GbaCore {
    fn step_frame(&mut self) {
        // Placeholder step logic for GBA Core
    }

    fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    fn display_dimensions(&self) -> (u32, u32) {
        (GBA_WIDTH, GBA_HEIGHT)
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!("GBA Input: {:?} -> {}", button, if pressed { "Pressed" } else { "Released" });
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        Vec::new()
    }
}
