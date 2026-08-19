use super::{InputSource, JoypadState};
use crate::core::Button;

/// Gamepad physical controller input driver.
#[derive(Debug, Clone, Default)]
pub struct GamepadInput {
    connected: bool,
    active_name: Option<String>,
    state: JoypadState,
}

#[allow(dead_code)]
impl GamepadInput {
    /// Constructs a new gamepad input driver.
    pub fn new() -> Self {
        Self {
            connected: false,
            active_name: None,
            state: JoypadState::default(),
        }
    }

    /// Sets the active button state on the gamepad driver.
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.state.set_pressed(button, pressed);
    }

    /// Checks if a physical gamepad is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Sets connection status and device descriptor name.
    pub fn set_connected(&mut self, connected: bool, name: Option<String>) {
        self.connected = connected;
        self.active_name = name;
        if !connected {
            self.state = JoypadState::default();
        }
    }

    /// Returns the human-readable name of the connected gamepad, if available.
    pub fn device_name(&self) -> Option<&str> {
        self.active_name.as_deref()
    }
}

impl InputSource for GamepadInput {
    fn name(&self) -> &'static str {
        "Gamepad"
    }

    fn poll(&mut self) -> JoypadState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamepad_input_driver() {
        let mut gp = GamepadInput::new();
        assert!(!gp.is_connected());
        assert_eq!(gp.poll().to_bits(), 0);

        gp.set_connected(true, Some("DualSense Wireless Controller".to_string()));
        assert!(gp.is_connected());
        assert_eq!(gp.device_name(), Some("DualSense Wireless Controller"));

        gp.set_button(Button::A, true);
        gp.set_button(Button::R, true);

        let state = gp.poll();
        assert!(state.is_pressed(Button::A));
        assert!(state.is_pressed(Button::R));
        assert!(!state.is_pressed(Button::B));

        gp.set_connected(false, None);
        assert!(!gp.is_connected());
        assert_eq!(gp.poll().to_bits(), 0);
    }
}
