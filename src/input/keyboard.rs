use crate::core::Button;
use std::collections::HashSet;
use winit::keyboard::KeyCode;

use super::{InputSource, JoypadState};

/// Physical keyboard input driver mapping winit key codes to joypad buttons.
#[derive(Debug, Clone, Default)]
pub struct KeyboardInput {
    pressed_keys: HashSet<KeyCode>,
}

impl KeyboardInput {
    /// Creates a new keyboard input driver.
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
        }
    }

    /// Handles a key event from the window event loop.
    pub fn handle_key(&mut self, key_code: KeyCode, pressed: bool) -> Option<Button> {
        if pressed {
            self.pressed_keys.insert(key_code);
        } else {
            self.pressed_keys.remove(&key_code);
        }

        Self::map_key_code(key_code)
    }

    /// Clears all pressed keys (e.g., on focus lost).
    pub fn clear(&mut self) {
        self.pressed_keys.clear();
    }

    /// Maps a winit physical KeyCode to a core Joypad Button.
    pub fn map_key_code(key_code: KeyCode) -> Option<Button> {
        match key_code {
            KeyCode::ArrowUp | KeyCode::KeyW => Some(Button::Up),
            KeyCode::ArrowDown | KeyCode::KeyS => Some(Button::Down),
            KeyCode::ArrowLeft | KeyCode::KeyA => Some(Button::Left),
            KeyCode::ArrowRight | KeyCode::KeyD => Some(Button::Right),
            KeyCode::KeyZ | KeyCode::KeyJ => Some(Button::A),
            KeyCode::KeyX | KeyCode::KeyK => Some(Button::B),
            KeyCode::KeyQ | KeyCode::KeyU => Some(Button::L),
            KeyCode::KeyE | KeyCode::KeyI => Some(Button::R),
            KeyCode::Enter => Some(Button::Start),
            KeyCode::ShiftRight | KeyCode::ShiftLeft | KeyCode::Backspace => Some(Button::Select),
            _ => None,
        }
    }
}

impl InputSource for KeyboardInput {
    fn name(&self) -> &'static str {
        "Keyboard"
    }

    fn poll(&mut self) -> JoypadState {
        let mut state = JoypadState::default();
        for &key in &self.pressed_keys {
            if let Some(btn) = Self::map_key_code(key) {
                state.set_pressed(btn, true);
            }
        }
        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_input_mapping() {
        let mut kb = KeyboardInput::new();
        assert_eq!(kb.poll().to_bits(), 0);

        kb.handle_key(KeyCode::KeyZ, true);
        kb.handle_key(KeyCode::ArrowUp, true);

        let state = kb.poll();
        assert!(state.is_pressed(Button::A));
        assert!(state.is_pressed(Button::Up));
        assert!(!state.is_pressed(Button::B));

        kb.handle_key(KeyCode::KeyZ, false);
        let state2 = kb.poll();
        assert!(!state2.is_pressed(Button::A));
        assert!(state2.is_pressed(Button::Up));

        kb.clear();
        assert_eq!(kb.poll().to_bits(), 0);
    }
}
