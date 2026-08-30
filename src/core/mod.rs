#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    A,
    B,
    Select,
    Start,
    Right,
    Left,
    Up,
    Down,
    L,
    R,
}

pub trait EmulatorCore {
    /// Advances core simulation by 1 frame (~16.6ms)
    fn step_frame(&mut self);

    /// Returns raw RGBA pixel buffer to draw
    fn framebuffer(&self) -> &[u8];

    /// Returns native display resolution (width, height)
    fn display_dimensions(&self) -> (u32, u32);

    /// Handles controller button state updates
    fn handle_input(&mut self, button: Button, pressed: bool);

    /// Returns queued stereo audio samples
    #[allow(dead_code)]
    fn audio_buffer(&mut self) -> Vec<f32>;

    /// Returns a reference to battery-backed Save RAM if supported by the active cartridge/core
    fn get_save_data(&self) -> Option<&[u8]> {
        None
    }

    /// Ingests saved battery RAM data into the active cartridge/core
    fn load_save_data(&mut self, _data: &[u8]) -> bool {
        false
    }

    /// Returns the save path for the currently loaded ROM
    fn save_path(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// Serializes full real-time emulation state snapshot to bytes
    fn save_state(&self) -> Option<Vec<u8>> {
        None
    }

    /// Restores real-time emulation state snapshot from bytes
    fn load_state(&mut self, _data: &[u8]) -> bool {
        false
    }

    /// Resets the emulation core to its initial state
    fn reset(&mut self) {}
}
