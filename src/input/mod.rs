pub mod gamepad;
pub mod keyboard;
pub mod touch;

pub use gamepad::GamepadInput;
pub use keyboard::KeyboardInput;
pub use touch::{
    touch_bits, ButtonShape, ChordHitbox, TouchAction, TouchInputManager, TouchOverlay,
    TouchOverlayPreset, TouchPhase, TouchPoint, TouchRect, VirtualButton, VirtualButtonId,
    VirtualDPad,
};

use crate::core::{Button, EmulatorCore};

/// 16-bit Joypad state bitmask representing unified digital inputs across all controller types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JoypadState(u16);

impl JoypadState {
    pub const A: u16 = 1 << 0;
    pub const B: u16 = 1 << 1;
    pub const SELECT: u16 = 1 << 2;
    pub const START: u16 = 1 << 3;
    pub const RIGHT: u16 = 1 << 4;
    pub const LEFT: u16 = 1 << 5;
    pub const UP: u16 = 1 << 6;
    pub const DOWN: u16 = 1 << 7;
    pub const R: u16 = 1 << 8;
    pub const L: u16 = 1 << 9;

    /// Maps a core `Button` enum to its corresponding bitmask bit.
    pub const fn button_bit(button: Button) -> u16 {
        match button {
            Button::A => Self::A,
            Button::B => Self::B,
            Button::Select => Self::SELECT,
            Button::Start => Self::START,
            Button::Right => Self::RIGHT,
            Button::Left => Self::LEFT,
            Button::Up => Self::UP,
            Button::Down => Self::DOWN,
            Button::R => Self::R,
            Button::L => Self::L,
        }
    }

    /// Constructs a `JoypadState` directly from a raw 16-bit bitmask.
    #[allow(dead_code)]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    /// Returns the raw 16-bit integer bitmask.
    #[allow(dead_code)]
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Checks if the specified button is pressed.
    pub const fn is_pressed(self, button: Button) -> bool {
        (self.0 & Self::button_bit(button)) != 0
    }

    /// Updates the pressed state of a button.
    pub fn set_pressed(&mut self, button: Button, pressed: bool) {
        let mask = Self::button_bit(button);
        if pressed {
            self.0 |= mask;
        } else {
            self.0 &= !mask;
        }
    }

    /// Merges another `JoypadState` bitmask (logical OR).
    pub fn merge(&mut self, other: JoypadState) {
        self.0 |= other.0;
    }

    /// Calculates button transition state changes between `self` and a previous state.
    pub fn diff(self, prev: JoypadState) -> Vec<(Button, bool)> {
        const ALL_BUTTONS: [Button; 10] = [
            Button::A,
            Button::B,
            Button::Select,
            Button::Start,
            Button::Right,
            Button::Left,
            Button::Up,
            Button::Down,
            Button::R,
            Button::L,
        ];

        let mut changes = Vec::with_capacity(4);
        for &btn in &ALL_BUTTONS {
            let now = self.is_pressed(btn);
            let before = prev.is_pressed(btn);
            if now != before {
                changes.push((btn, now));
            }
        }
        changes
    }
}

/// Unified abstraction trait for all input sources (keyboard, physical gamepad, virtual touch overlay).
pub trait InputSource {
    /// Returns the human-readable identifier of the input source driver.
    #[allow(dead_code)]
    fn name(&self) -> &'static str;

    /// Polls the input device and returns its active 16-bit `JoypadState`.
    fn poll(&mut self) -> JoypadState;
}

/// Central Input Manager orchestrating keyboard, physical gamepad, and touch overlay inputs.
#[derive(Debug, Default)]
pub struct InputManager {
    pub keyboard: KeyboardInput,
    pub gamepad: GamepadInput,
    pub touch: TouchOverlay,
    prev_dispatched_state: JoypadState,
}

impl InputManager {
    /// Constructs a new InputManager instance.
    pub fn new() -> Self {
        Self {
            keyboard: KeyboardInput::new(),
            gamepad: GamepadInput::new(),
            touch: TouchOverlay::new(),
            prev_dispatched_state: JoypadState::default(),
        }
    }

    /// Polls and resolves all active input sources into a single unified `JoypadState` bitmask.
    pub fn poll_merged(&mut self) -> JoypadState {
        let mut merged = JoypadState::default();
        merged.merge(self.keyboard.poll());
        merged.merge(self.gamepad.poll());
        merged.merge(self.touch.poll());
        merged
    }

    /// Dispatches state changes to the target emulator core if any button state transitioned.
    pub fn dispatch_to_core(&mut self, core: &mut dyn EmulatorCore) {
        let current_state = self.poll_merged();
        if current_state != self.prev_dispatched_state {
            let changes = current_state.diff(self.prev_dispatched_state);
            for (btn, pressed) in changes {
                core.handle_input(btn, pressed);
            }
            self.prev_dispatched_state = current_state;
        }
    }

    /// Resets all inputs (e.g. on window focus loss or pause).
    pub fn clear_all(&mut self) {
        self.keyboard.clear();
        self.touch.clear();
        self.prev_dispatched_state = JoypadState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joypad_state_bitmask_operations() {
        let mut state = JoypadState::default();
        assert_eq!(state.to_bits(), 0);

        state.set_pressed(Button::A, true);
        state.set_pressed(Button::Start, true);
        assert!(state.is_pressed(Button::A));
        assert!(state.is_pressed(Button::Start));
        assert!(!state.is_pressed(Button::B));
        assert_eq!(state.to_bits(), JoypadState::A | JoypadState::START);

        let mut other = JoypadState::default();
        other.set_pressed(Button::B, true);
        other.set_pressed(Button::L, true);

        state.merge(other);
        assert!(state.is_pressed(Button::A));
        assert!(state.is_pressed(Button::B));
        assert!(state.is_pressed(Button::Start));
        assert!(state.is_pressed(Button::L));
        assert!(!state.is_pressed(Button::R));

        let diff = state.diff(other);
        assert!(diff.contains(&(Button::A, true)));
        assert!(diff.contains(&(Button::Start, true)));
        assert!(!diff.contains(&(Button::B, true)));
    }
}
