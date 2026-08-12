#![allow(dead_code)]

use crate::core::Button;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GbaButton {
    A = 0,
    B = 1,
    Select = 2,
    Start = 3,
    Right = 4,
    Left = 5,
    Up = 6,
    Down = 7,
    R = 8,
    L = 9,
}

/// GBA Keypad Controller handling active-low KEYINPUT (0x04000130) and KEYCNT (0x04000132)
pub struct GbaKeypad {
    /// 16-bit Active-Low KEYINPUT register. 0 = Pressed, 1 = Released.
    pub keyinput: u16,

    /// 16-bit KEYCNT interrupt control register.
    pub keycnt: u16,
}

impl Default for GbaKeypad {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaKeypad {
    pub fn new() -> Self {
        Self {
            keyinput: 0x03FF, // Default: all 10 buttons released
            keycnt: 0,
        }
    }

    /// Reset keypad state to defaults
    pub fn reset(&mut self) {
        self.keyinput = 0x03FF;
        self.keycnt = 0;
    }

    /// Update specific GBA button state (Active-Low)
    pub fn set_button(&mut self, button: GbaButton, pressed: bool) {
        let bit = button as u16;
        if pressed {
            self.keyinput &= !(1 << bit); // Active low: 0 when pressed
        } else {
            self.keyinput |= 1 << bit;   // Active low: 1 when released
        }
    }

    /// Map unified core Button state to GBA Keypad
    pub fn handle_input(&mut self, button: Button, pressed: bool) {
        match button {
            Button::A => self.set_button(GbaButton::A, pressed),
            Button::B => self.set_button(GbaButton::B, pressed),
            Button::Select => self.set_button(GbaButton::Select, pressed),
            Button::Start => self.set_button(GbaButton::Start, pressed),
            Button::Right => self.set_button(GbaButton::Right, pressed),
            Button::Left => self.set_button(GbaButton::Left, pressed),
            Button::Up => self.set_button(GbaButton::Up, pressed),
            Button::Down => self.set_button(GbaButton::Down, pressed),
            Button::L => self.set_button(GbaButton::L, pressed),
            Button::R => self.set_button(GbaButton::R, pressed),
        }
    }

    /// Read byte from KEYINPUT / KEYCNT I/O space
    pub fn read_u8(&self, addr: u32) -> u8 {
        match addr {
            0x04000130 => self.keyinput as u8,
            0x04000131 => (self.keyinput >> 8) as u8,
            0x04000132 => self.keycnt as u8,
            0x04000133 => (self.keycnt >> 8) as u8,
            _ => 0,
        }
    }

    /// Write byte to KEYCNT I/O space (KEYINPUT is read-only)
    pub fn write_u8(&mut self, addr: u32, val: u8) {
        match addr {
            0x04000132 => self.keycnt = (self.keycnt & 0xFF00) | val as u16,
            0x04000133 => self.keycnt = (self.keycnt & 0x00FF) | ((val as u16) << 8),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypad_active_low_press() {
        let mut keypad = GbaKeypad::new();
        assert_eq!(keypad.keyinput, 0x03FF); // All released

        // Press A button
        keypad.handle_input(Button::A, true);
        assert_eq!(keypad.keyinput & 1, 0); // Bit 0 should be 0 (pressed)

        // Release A button
        keypad.handle_input(Button::A, false);
        assert_eq!(keypad.keyinput & 1, 1); // Bit 0 should be 1 (released)
    }

    #[test]
    fn test_shoulder_buttons() {
        let mut keypad = GbaKeypad::new();
        keypad.handle_input(Button::L, true);
        keypad.handle_input(Button::R, true);

        assert_eq!((keypad.keyinput >> 9) & 1, 0); // L shoulder pressed
        assert_eq!((keypad.keyinput >> 8) & 1, 0); // R shoulder pressed
    }
}
