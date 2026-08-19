use crate::core::Button;
use std::collections::HashMap;

use super::{InputSource, JoypadState};

/// Normalized rectangular bounding box for virtual touch controls (0.0 to 1.0 coordinates).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl TouchRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Checks if a normalized point (px, py) is inside the rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= (self.x + self.width) && py >= self.y && py <= (self.y + self.height)
    }
}

/// Touch pointer tracking active touch coordinate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u64,
    pub norm_x: f32,
    pub norm_y: f32,
}

/// Virtual on-screen touch overlay layer for mobile and touch-screen devices.
#[derive(Debug, Clone)]
pub struct TouchOverlay {
    pub visible: bool,
    pub dpad_rect: TouchRect,
    pub btn_a_rect: TouchRect,
    pub btn_b_rect: TouchRect,
    pub btn_l_rect: TouchRect,
    pub btn_r_rect: TouchRect,
    pub btn_start_rect: TouchRect,
    pub btn_select_rect: TouchRect,
    active_touches: HashMap<u64, TouchPoint>,
}

impl Default for TouchOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchOverlay {
    /// Creates a new TouchOverlay layout with default handheld ergonomics.
    pub fn new() -> Self {
        Self {
            visible: true,
            // D-Pad bottom left
            dpad_rect: TouchRect::new(0.04, 0.65, 0.26, 0.28),
            // Action buttons bottom right
            btn_b_rect: TouchRect::new(0.70, 0.72, 0.12, 0.16),
            btn_a_rect: TouchRect::new(0.84, 0.64, 0.12, 0.16),
            // Shoulder buttons top left and top right
            btn_l_rect: TouchRect::new(0.04, 0.05, 0.18, 0.10),
            btn_r_rect: TouchRect::new(0.78, 0.05, 0.18, 0.10),
            // Menu / function buttons bottom center
            btn_select_rect: TouchRect::new(0.38, 0.88, 0.10, 0.07),
            btn_start_rect: TouchRect::new(0.52, 0.88, 0.10, 0.07),
            active_touches: HashMap::new(),
        }
    }

    /// Ingests touch start / finger down event.
    pub fn handle_touch_down(&mut self, id: u64, x: f32, y: f32, screen_w: f32, screen_h: f32) {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return;
        }
        let norm_x = (x / screen_w).clamp(0.0, 1.0);
        let norm_y = (y / screen_h).clamp(0.0, 1.0);
        self.active_touches
            .insert(id, TouchPoint { id, norm_x, norm_y });
    }

    /// Ingests touch move / finger dragged event.
    pub fn handle_touch_move(&mut self, id: u64, x: f32, y: f32, screen_w: f32, screen_h: f32) {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return;
        }
        let norm_x = (x / screen_w).clamp(0.0, 1.0);
        let norm_y = (y / screen_h).clamp(0.0, 1.0);
        self.active_touches
            .insert(id, TouchPoint { id, norm_x, norm_y });
    }

    /// Ingests touch release / finger up event.
    pub fn handle_touch_up(&mut self, id: u64) {
        self.active_touches.remove(&id);
    }

    /// Ingests touch cancellation event.
    pub fn handle_touch_cancel(&mut self, id: u64) {
        self.active_touches.remove(&id);
    }

    /// Clears all active touch pointers.
    pub fn clear(&mut self) {
        self.active_touches.clear();
    }

    /// Resolves virtual D-Pad coordinate into directional buttons (8-way directional support).
    fn resolve_dpad(&self, pt: &TouchPoint, state: &mut JoypadState) {
        if !self.dpad_rect.contains(pt.norm_x, pt.norm_y) {
            return;
        }

        let center_x = self.dpad_rect.x + self.dpad_rect.width * 0.5;
        let center_y = self.dpad_rect.y + self.dpad_rect.height * 0.5;

        let dx = (pt.norm_x - center_x) / (self.dpad_rect.width * 0.5);
        let dy = (pt.norm_y - center_y) / (self.dpad_rect.height * 0.5);

        let deadzone = 0.20;
        if dx > deadzone {
            state.set_pressed(Button::Right, true);
        } else if dx < -deadzone {
            state.set_pressed(Button::Left, true);
        }

        if dy > deadzone {
            state.set_pressed(Button::Down, true);
        } else if dy < -deadzone {
            state.set_pressed(Button::Up, true);
        }
    }
}

impl InputSource for TouchOverlay {
    fn name(&self) -> &'static str {
        "TouchOverlay"
    }

    fn poll(&mut self) -> JoypadState {
        let mut state = JoypadState::default();
        if !self.visible {
            return state;
        }

        for pt in self.active_touches.values() {
            // Directional D-pad
            self.resolve_dpad(pt, &mut state);

            // Action Buttons
            if self.btn_a_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::A, true);
            }
            if self.btn_b_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::B, true);
            }

            // Shoulder Buttons
            if self.btn_l_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::L, true);
            }
            if self.btn_r_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::R, true);
            }

            // Function Buttons
            if self.btn_start_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::Start, true);
            }
            if self.btn_select_rect.contains(pt.norm_x, pt.norm_y) {
                state.set_pressed(Button::Select, true);
            }
        }

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_overlay_dpad_resolution() {
        let mut overlay = TouchOverlay::new();
        assert_eq!(overlay.poll().to_bits(), 0);

        let screen_w = 1000.0;
        let screen_h = 1000.0;

        // Touch right side of D-pad
        let dpad_right_x = (overlay.dpad_rect.x + overlay.dpad_rect.width * 0.85) * screen_w;
        let dpad_right_y = (overlay.dpad_rect.y + overlay.dpad_rect.height * 0.50) * screen_h;
        overlay.handle_touch_down(1, dpad_right_x, dpad_right_y, screen_w, screen_h);

        let state = overlay.poll();
        assert!(state.is_pressed(Button::Right));
        assert!(!state.is_pressed(Button::Left));
        assert!(!state.is_pressed(Button::Up));

        // Touch button A with second finger
        let a_x = (overlay.btn_a_rect.x + overlay.btn_a_rect.width * 0.5) * screen_w;
        let a_y = (overlay.btn_a_rect.y + overlay.btn_a_rect.height * 0.5) * screen_h;
        overlay.handle_touch_down(2, a_x, a_y, screen_w, screen_h);

        let multi_state = overlay.poll();
        assert!(multi_state.is_pressed(Button::Right));
        assert!(multi_state.is_pressed(Button::A));

        // Release first finger
        overlay.handle_touch_up(1);
        let single_state = overlay.poll();
        assert!(!single_state.is_pressed(Button::Right));
        assert!(single_state.is_pressed(Button::A));

        overlay.clear();
        assert_eq!(overlay.poll().to_bits(), 0);
    }
}
