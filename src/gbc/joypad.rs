use crate::core::Button;

/// Joypad handler for Game Boy hardware (0xFF00 JOYP I/O register).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Joypad {
    pub a: bool,
    pub b: bool,
    pub select: bool,
    pub start: bool,
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,

    select_buttons: bool,    // Bit 5 (0 = selected)
    select_directions: bool, // Bit 4 (0 = selected)
}

#[allow(dead_code)]
impl Joypad {
    pub fn new() -> Self {
        Self {
            a: false,
            b: false,
            select: false,
            start: false,
            right: false,
            left: false,
            up: false,
            down: false,
            select_buttons: false,
            select_directions: false,
        }
    }

    /// Handles button press and release state updates.
    pub fn handle_input(&mut self, button: Button, pressed: bool) {
        match button {
            Button::A => self.a = pressed,
            Button::B => self.b = pressed,
            Button::Select => self.select = pressed,
            Button::Start => self.start = pressed,
            Button::Right => self.right = pressed,
            Button::Left => self.left = pressed,
            Button::Up => self.up = pressed,
            Button::Down => self.down = pressed,
            Button::L | Button::R => {}
        }
    }

    /// Writes JOYP selection bits (bits 5 and 4).
    pub fn write_joyp(&mut self, val: u8) {
        self.select_buttons = (val & 0x20) == 0;
        self.select_directions = (val & 0x10) == 0;
    }

    /// Reads JOYP matrix active-low button state.
    pub fn read_joyp(&self) -> u8 {
        let mut res = 0xC0; // Bits 7 and 6 are unmapped (always 1)

        if !self.select_buttons {
            res |= 0x20;
        }
        if !self.select_directions {
            res |= 0x10;
        }

        let mut nibble = 0x0F;

        if self.select_directions {
            if self.right {
                nibble &= !0x01;
            }
            if self.left {
                nibble &= !0x02;
            }
            if self.up {
                nibble &= !0x04;
            }
            if self.down {
                nibble &= !0x08;
            }
        }

        if self.select_buttons {
            if self.a {
                nibble &= !0x01;
            }
            if self.b {
                nibble &= !0x02;
            }
            if self.select {
                nibble &= !0x04;
            }
            if self.start {
                nibble &= !0x08;
            }
        }

        res | nibble
    }
}

impl Default for Joypad {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_joypad_matrix_selection() {
        let mut joypad = Joypad::new();
        joypad.handle_input(Button::A, true);
        joypad.handle_input(Button::Right, true);

        // Select Action Buttons (Bit 5 = 0, Bit 4 = 1 => 0x10 written)
        joypad.write_joyp(0x10);
        let val = joypad.read_joyp();
        // A is pressed => bit 0 is 0 => 0x1E
        assert_eq!(val & 0x0F, 0x0E);

        // Select Direction Buttons (Bit 5 = 1, Bit 4 = 0 => 0x20 written)
        joypad.write_joyp(0x20);
        let val = joypad.read_joyp();
        // Right is pressed => bit 0 is 0 => 0x0E
        assert_eq!(val & 0x0F, 0x0E);
    }
}
