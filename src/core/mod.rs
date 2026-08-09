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
}
