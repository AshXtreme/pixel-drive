//! Multi-Touch Input Engine and Virtual On-Screen Gamepad Tracker.
//!
//! Provides normalized coordinate tracking, 8-way directional D-pad calculations,
//! circular/pill hitboxes, A+B chord bridge detection, customizable layout presets,
//! dynamic floating center D-pad mode, and WGPU overlay state synchronization.

use crate::core::Button;
use std::collections::HashMap;

pub use crate::ui::menu::{
    LayoutEditorLayout, LayoutEditorToolbarItem, MenuItem, MenuItem as TouchMenuItem,
    MenuLayout, MenuState, MenuState as TouchMenuState, SaveLoadItem, SaveLoadLayout,
    SettingsItem, SettingsLayout, SlotMode,
};

use super::{InputSource, JoypadState};

/// Draggable control group in the on-screen layout editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlGroup {
    Dpad,
    ActionCluster,
    LShoulder,
    RShoulder,
    StartSelect,
}

/// Phase of a touch event pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

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

    /// Center of the bounding box.
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    /// Checks if a normalized point (px, py) is inside the rectangle.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= (self.x + self.width) && py >= self.y && py <= (self.y + self.height)
    }
}

/// Active touch pointer data model tracking normalized position and lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub norm_x: f32,
    pub norm_y: f32,
    pub start_x: f32,
    pub start_y: f32,
    pub phase: TouchPhase,
}

impl TouchPoint {
    pub fn new(id: u64, x: f32, y: f32, phase: TouchPhase) -> Self {
        Self {
            id,
            x,
            y,
            norm_x: x,
            norm_y: y,
            start_x: x,
            start_y: y,
            phase,
        }
    }
}

/// Button identifier for virtual on-screen controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirtualButtonId {
    A,
    B,
    L,
    R,
    Start,
    Select,
    FastForward,
    Menu,
    QuickSave,
    QuickLoad,
}

/// Shape geometry of a virtual button hitbox.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ButtonShape {
    Circle {
        center: (f32, f32),
        radius: f32,
    },
    Pill {
        rect: TouchRect,
        corner_radius: f32,
    },
}

impl ButtonShape {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        match *self {
            ButtonShape::Circle { center, radius } => {
                let dx = px - center.0;
                let dy = py - center.1;
                (dx * dx + dy * dy) <= (radius * radius)
            }
            ButtonShape::Pill { rect, corner_radius } => {
                if !rect.contains(px, py) {
                    return false;
                }
                // Rounded corner distance check
                let min_x = rect.x + corner_radius;
                let max_x = rect.x + rect.width - corner_radius;
                let min_y = rect.y + corner_radius;
                let max_y = rect.y + rect.height - corner_radius;

                let clamped_x = px.clamp(min_x, max_x);
                let clamped_y = py.clamp(min_y, max_y);
                let dx = px - clamped_x;
                let dy = py - clamped_y;
                (dx * dx + dy * dy) <= (corner_radius * corner_radius)
            }
        }
    }

    pub fn center(&self) -> (f32, f32) {
        match *self {
            ButtonShape::Circle { center, .. } => center,
            ButtonShape::Pill { rect, .. } => rect.center(),
        }
    }

    pub fn radius(&self) -> f32 {
        match *self {
            ButtonShape::Circle { radius, .. } => radius,
            ButtonShape::Pill { corner_radius, .. } => corner_radius,
        }
    }
}

/// Hitbox definition for an on-screen virtual button.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualButton {
    pub id: VirtualButtonId,
    pub shape: ButtonShape,
}

impl VirtualButton {
    pub fn new_circle(id: VirtualButtonId, center_x: f32, center_y: f32, radius: f32) -> Self {
        Self {
            id,
            shape: ButtonShape::Circle {
                center: (center_x, center_y),
                radius,
            },
        }
    }

    pub fn new_pill(
        id: VirtualButtonId,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
    ) -> Self {
        Self {
            id,
            shape: ButtonShape::Pill {
                rect: TouchRect::new(x, y, width, height),
                corner_radius,
            },
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        self.shape.contains(px, py)
    }

    pub fn center(&self) -> (f32, f32) {
        self.shape.center()
    }

    pub fn radius(&self) -> f32 {
        self.shape.radius()
    }

    pub fn to_rect(&self) -> TouchRect {
        match self.shape {
            ButtonShape::Circle { center, radius } => TouchRect::new(
                center.0 - radius,
                center.1 - radius,
                radius * 2.0,
                radius * 2.0,
            ),
            ButtonShape::Pill { rect, .. } => rect,
        }
    }
}

/// Geometric bridge hitbox between A and B buttons enabling simultaneous A+B triggering.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChordHitbox {
    pub p1: (f32, f32),
    pub p2: (f32, f32),
    pub radius: f32,
}

impl ChordHitbox {
    pub fn new(p1: (f32, f32), p2: (f32, f32), radius: f32) -> Self {
        Self { p1, p2, radius }
    }

    /// Checks if a normalized point (px, py) falls within the bridge region between p1 and p2.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let vx = self.p2.0 - self.p1.0;
        let vy = self.p2.1 - self.p1.1;
        let len_sq = vx * vx + vy * vy;
        if len_sq <= 1e-6 {
            return false;
        }

        let wx = px - self.p1.0;
        let wy = py - self.p1.1;
        let t = (wx * vx + wy * vy) / len_sq;

        // Only activate chord when touch is in the bridge zone between buttons (25% to 75%)
        if t < 0.20 || t > 0.80 {
            return false;
        }

        let proj_x = self.p1.0 + t * vx;
        let proj_y = self.p1.1 + t * vy;
        let dx = px - proj_x;
        let dy = py - proj_y;
        (dx * dx + dy * dy) <= (self.radius * self.radius)
    }
}

/// 8-way directional virtual D-Pad calculator with fixed or dynamic floating center.
#[derive(Debug, Clone, PartialEq)]
pub struct VirtualDPad {
    pub center: (f32, f32),
    pub radius: f32,
    pub deadzone: f32,
    pub diagonal_threshold: f32,
    pub dynamic_center: bool,
    pub active_center: Option<(f32, f32)>,
    pub tracking_touch_id: Option<u64>,
}

impl VirtualDPad {
    pub fn new(cx: f32, cy: f32, radius: f32, deadzone: f32) -> Self {
        Self {
            center: (cx, cy),
            radius,
            deadzone,
            diagonal_threshold: 0.3826, // sin(22.5 deg) for clean 8-way sectors
            dynamic_center: false,
            active_center: None,
            tracking_touch_id: None,
        }
    }

    /// Outer bounding box for layout and touch ingestion.
    pub fn to_rect(&self) -> TouchRect {
        TouchRect::new(
            self.center.0 - self.radius,
            self.center.1 - self.radius,
            self.radius * 2.0,
            self.radius * 2.0,
        )
    }

    /// Checks if point (px, py) is within the touchable bounds of the D-Pad.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        let center = self.active_center.unwrap_or(self.center);
        let dx = px - center.0;
        let dy = py - center.1;
        (dx * dx + dy * dy) <= (self.radius * self.radius * 1.50) // outer touch margin
    }

    /// Calculates 8-way directional bools: (up, down, left, right) from touch position.
    pub fn calculate_direction(&self, px: f32, py: f32) -> (bool, bool, bool, bool) {
        if !self.contains(px, py) {
            return (false, false, false, false);
        }

        let center = self.active_center.unwrap_or(self.center);
        let dx = px - center.0;
        let dy = py - center.1;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < self.deadzone || self.radius <= 0.0 {
            return (false, false, false, false);
        }

        // Normalized direction vector
        let ndx = dx / dist;
        let ndy = dy / dist;

        let mut up = false;
        let mut down = false;
        let mut left = false;
        let mut right = false;

        let diag_thresh = self.diagonal_threshold;

        if ndy < -diag_thresh {
            up = true;
        } else if ndy > diag_thresh {
            down = true;
        }

        if ndx < -diag_thresh {
            left = true;
        } else if ndx > diag_thresh {
            right = true;
        }

        (up, down, left, right)
    }

    /// Handles touch down on D-pad. If dynamic center is enabled, establishes floating center.
    pub fn handle_touch_down(&mut self, id: u64, px: f32, py: f32) {
        if self.tracking_touch_id.is_none() && self.contains(px, py) {
            self.tracking_touch_id = Some(id);
            if self.dynamic_center {
                self.active_center = Some((px, py));
            } else {
                self.active_center = Some(self.center);
            }
        }
    }

    /// Handles touch release on D-pad.
    pub fn handle_touch_up(&mut self, id: u64) {
        if self.tracking_touch_id == Some(id) {
            self.tracking_touch_id = None;
            self.active_center = None;
        }
    }
}

/// Layout positioning presets for virtual touch overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TouchOverlayPreset {
    #[default]
    Standard,
    Compact,
    Wide,
    Ergonomic,
}

impl TouchOverlayPreset {
    pub fn name(&self) -> &'static str {
        match self {
            TouchOverlayPreset::Standard => "Standard",
            TouchOverlayPreset::Compact => "Compact",
            TouchOverlayPreset::Wide => "Wide",
            TouchOverlayPreset::Ergonomic => "Ergonomic",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            TouchOverlayPreset::Standard => TouchOverlayPreset::Compact,
            TouchOverlayPreset::Compact => TouchOverlayPreset::Wide,
            TouchOverlayPreset::Wide => TouchOverlayPreset::Ergonomic,
            TouchOverlayPreset::Ergonomic => TouchOverlayPreset::Standard,
        }
    }
}

/// Action events triggered by virtual touch controls (e.g. menu, fast-forward, modal selection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    ToggleFastForward,
    OpenMenu,
    CloseMenu,
    QuickSave,
    QuickLoad,
    MenuSelect(MenuItem),
    SelectSlot { slot: u8, mode: SlotMode },
    ToggleSlotMode,
    MenuBack,
    SettingsSelect(SettingsItem),
    LayoutEditorAction(LayoutEditorToolbarItem),
}

/// Pressed bitmask constants matching GB/GBA layout and UI HUD elements for the shader pipeline.
pub mod touch_bits {
    pub const BTN_A: u32 = 1 << 0;
    pub const BTN_B: u32 = 1 << 1;
    pub const BTN_SELECT: u32 = 1 << 2;
    pub const BTN_START: u32 = 1 << 3;
    pub const DPAD_RIGHT: u32 = 1 << 4;
    pub const DPAD_LEFT: u32 = 1 << 5;
    pub const DPAD_UP: u32 = 1 << 6;
    pub const DPAD_DOWN: u32 = 1 << 7;
    pub const BTN_R: u32 = 1 << 8;
    pub const BTN_L: u32 = 1 << 9;
    pub const BTN_MENU: u32 = 1 << 10;
    pub const BTN_FAST_FORWARD: u32 = 1 << 11;
    pub const CHORD_AB: u32 = 1 << 12;
    pub const BTN_QUICK_SAVE: u32 = 1 << 13;
    pub const BTN_QUICK_LOAD: u32 = 1 << 14;
}

/// Multi-Touch Input Engine & On-Screen Overlay Manager.
#[derive(Debug, Clone)]
pub struct TouchInputManager {
    pub visible: bool,
    pub opacity: f32,
    pub scale: f32,
    pub haptics_enabled: bool,
    pub dynamic_dpad: bool,
    pub auto_hide_on_gamepad: bool,
    pub preset: TouchOverlayPreset,
    pub safe_insets: [f32; 4], // [top, bottom, left, right]
    pub theme_index: u8,

    // In-game Modal Pause Menu State
    pub menu_state: MenuState,
    pub menu_layout: MenuLayout,
    pub save_load_layout: SaveLoadLayout,
    pub settings_layout: SettingsLayout,
    pub layout_editor_layout: LayoutEditorLayout,
    pub pressed_menu_item: Option<MenuItem>,
    pub pressed_save_load_item: Option<SaveLoadItem>,
    pub pressed_settings_item: Option<SettingsItem>,
    pub pressed_editor_toolbar_item: Option<LayoutEditorToolbarItem>,
    pub slot_mask: u32,

    // Layout Editor Interactive Drag State
    pub active_drag_group: Option<(ControlGroup, u64, (f32, f32))>,
    pub drag_start_touch: Option<(f32, f32)>,

    // Virtual Controls Geometry
    pub dpad: VirtualDPad,
    pub btn_a: VirtualButton,
    pub btn_b: VirtualButton,
    pub chord_ab: ChordHitbox,
    pub btn_l: VirtualButton,
    pub btn_r: VirtualButton,
    pub btn_start: VirtualButton,
    pub btn_select: VirtualButton,
    pub btn_menu: VirtualButton,
    pub btn_fast_forward: VirtualButton,
    pub btn_quick_save: VirtualButton,
    pub btn_quick_load: VirtualButton,

    // Active Pointer Tracking
    active_touches: HashMap<u64, TouchPoint>,
    pending_actions: Vec<TouchAction>,
    pressed_mask: u32,
    prev_pressed_mask: u32,
}

/// Alias for backwards compatibility with existing codebase.
pub type TouchOverlay = TouchInputManager;

impl Default for TouchInputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchInputManager {
    /// Creates a new TouchInputManager instance with default ergonomic layout.
    pub fn new() -> Self {
        Self::with_preset(TouchOverlayPreset::Standard)
    }

    /// Constructs TouchInputManager initialized with a specific preset.
    pub fn with_preset(preset: TouchOverlayPreset) -> Self {
        let mut manager = Self {
            visible: true,
            opacity: 0.65,
            scale: 1.0,
            haptics_enabled: true,
            dynamic_dpad: false,
            auto_hide_on_gamepad: false,
            preset,
            safe_insets: [0.0, 0.0, 0.0, 0.0],
            theme_index: 0,

            menu_state: MenuState::Hidden,
            menu_layout: MenuLayout::new(),
            save_load_layout: SaveLoadLayout::new(),
            settings_layout: SettingsLayout::new(),
            layout_editor_layout: LayoutEditorLayout::new(),
            pressed_menu_item: None,
            pressed_save_load_item: None,
            pressed_settings_item: None,
            pressed_editor_toolbar_item: None,
            slot_mask: 0,

            active_drag_group: None,
            drag_start_touch: None,

            // Default geometry placeholders (recomputed in apply_preset)
            dpad: VirtualDPad::new(0.14, 0.76, 0.11, 0.025),
            btn_a: VirtualButton::new_circle(VirtualButtonId::A, 0.90, 0.70, 0.055),
            btn_b: VirtualButton::new_circle(VirtualButtonId::B, 0.78, 0.80, 0.055),
            chord_ab: ChordHitbox::new((0.90, 0.70), (0.78, 0.80), 0.040),
            btn_l: VirtualButton::new_pill(VirtualButtonId::L, 0.03, 0.04, 0.16, 0.07, 0.035),
            btn_r: VirtualButton::new_pill(VirtualButtonId::R, 0.81, 0.04, 0.16, 0.07, 0.035),
            btn_select: VirtualButton::new_pill(VirtualButtonId::Select, 0.38, 0.90, 0.09, 0.05, 0.025),
            btn_start: VirtualButton::new_pill(VirtualButtonId::Start, 0.53, 0.90, 0.09, 0.05, 0.025),
            btn_menu: VirtualButton::new_circle(VirtualButtonId::Menu, 0.44, 0.07, 0.035),
            btn_fast_forward: VirtualButton::new_circle(VirtualButtonId::FastForward, 0.56, 0.07, 0.035),
            btn_quick_save: VirtualButton::new_circle(VirtualButtonId::QuickSave, 0.32, 0.07, 0.035),
            btn_quick_load: VirtualButton::new_circle(VirtualButtonId::QuickLoad, 0.68, 0.07, 0.035),

            active_touches: HashMap::new(),
            pending_actions: Vec::new(),
            pressed_mask: 0,
            prev_pressed_mask: 0,
        };

        manager.apply_preset(preset);
        manager
    }

    /// Applies layout configuration preset.
    pub fn apply_preset(&mut self, preset: TouchOverlayPreset) {
        self.preset = preset;
        let s = self.scale;

        match preset {
            TouchOverlayPreset::Standard => {
                // D-Pad bottom left
                self.dpad = VirtualDPad::new(0.14, 0.76, 0.11 * s, 0.025 * s);
                // Action buttons bottom right (angled A/B layout)
                let a_cx = 0.90;
                let a_cy = 0.70;
                let b_cx = 0.78;
                let b_cy = 0.80;
                let btn_rad = 0.055 * s;
                self.btn_a = VirtualButton::new_circle(VirtualButtonId::A, a_cx, a_cy, btn_rad);
                self.btn_b = VirtualButton::new_circle(VirtualButtonId::B, b_cx, b_cy, btn_rad);
                self.chord_ab = ChordHitbox::new((a_cx, a_cy), (b_cx, b_cy), 0.040 * s);

                // Shoulders
                self.btn_l = VirtualButton::new_pill(VirtualButtonId::L, 0.03, 0.04, 0.16 * s, 0.07 * s, 0.035 * s);
                self.btn_r = VirtualButton::new_pill(VirtualButtonId::R, 0.97 - 0.16 * s, 0.04, 0.16 * s, 0.07 * s, 0.035 * s);

                // Center system buttons
                self.btn_select = VirtualButton::new_pill(VirtualButtonId::Select, 0.38, 0.90, 0.09 * s, 0.05 * s, 0.025 * s);
                self.btn_start = VirtualButton::new_pill(VirtualButtonId::Start, 0.53, 0.90, 0.09 * s, 0.05 * s, 0.025 * s);

                // Top center quick actions
                self.btn_quick_save = VirtualButton::new_circle(VirtualButtonId::QuickSave, 0.32, 0.07, 0.032 * s);
                self.btn_menu = VirtualButton::new_circle(VirtualButtonId::Menu, 0.44, 0.07, 0.032 * s);
                self.btn_fast_forward = VirtualButton::new_circle(VirtualButtonId::FastForward, 0.56, 0.07, 0.032 * s);
                self.btn_quick_load = VirtualButton::new_circle(VirtualButtonId::QuickLoad, 0.68, 0.07, 0.032 * s);
            }
            TouchOverlayPreset::Compact => {
                self.dpad = VirtualDPad::new(0.12, 0.80, 0.095 * s, 0.020 * s);
                let a_cx = 0.92;
                let a_cy = 0.74;
                let b_cx = 0.82;
                let b_cy = 0.83;
                let btn_rad = 0.048 * s;
                self.btn_a = VirtualButton::new_circle(VirtualButtonId::A, a_cx, a_cy, btn_rad);
                self.btn_b = VirtualButton::new_circle(VirtualButtonId::B, b_cx, b_cy, btn_rad);
                self.chord_ab = ChordHitbox::new((a_cx, a_cy), (b_cx, b_cy), 0.035 * s);

                self.btn_l = VirtualButton::new_pill(VirtualButtonId::L, 0.02, 0.03, 0.14 * s, 0.06 * s, 0.030 * s);
                self.btn_r = VirtualButton::new_pill(VirtualButtonId::R, 0.98 - 0.14 * s, 0.03, 0.14 * s, 0.06 * s, 0.030 * s);

                self.btn_select = VirtualButton::new_pill(VirtualButtonId::Select, 0.39, 0.92, 0.08 * s, 0.045 * s, 0.022 * s);
                self.btn_start = VirtualButton::new_pill(VirtualButtonId::Start, 0.53, 0.92, 0.08 * s, 0.045 * s, 0.022 * s);

                self.btn_quick_save = VirtualButton::new_circle(VirtualButtonId::QuickSave, 0.35, 0.06, 0.028 * s);
                self.btn_menu = VirtualButton::new_circle(VirtualButtonId::Menu, 0.45, 0.06, 0.028 * s);
                self.btn_fast_forward = VirtualButton::new_circle(VirtualButtonId::FastForward, 0.55, 0.06, 0.028 * s);
                self.btn_quick_load = VirtualButton::new_circle(VirtualButtonId::QuickLoad, 0.65, 0.06, 0.028 * s);
            }
            TouchOverlayPreset::Wide => {
                self.dpad = VirtualDPad::new(0.16, 0.72, 0.125 * s, 0.028 * s);
                let a_cx = 0.88;
                let a_cy = 0.68;
                let b_cx = 0.74;
                let b_cy = 0.78;
                let btn_rad = 0.062 * s;
                self.btn_a = VirtualButton::new_circle(VirtualButtonId::A, a_cx, a_cy, btn_rad);
                self.btn_b = VirtualButton::new_circle(VirtualButtonId::B, b_cx, b_cy, btn_rad);
                self.chord_ab = ChordHitbox::new((a_cx, a_cy), (b_cx, b_cy), 0.045 * s);

                self.btn_l = VirtualButton::new_pill(VirtualButtonId::L, 0.04, 0.05, 0.18 * s, 0.08 * s, 0.040 * s);
                self.btn_r = VirtualButton::new_pill(VirtualButtonId::R, 0.96 - 0.18 * s, 0.05, 0.18 * s, 0.08 * s, 0.040 * s);

                self.btn_select = VirtualButton::new_pill(VirtualButtonId::Select, 0.36, 0.88, 0.11 * s, 0.055 * s, 0.028 * s);
                self.btn_start = VirtualButton::new_pill(VirtualButtonId::Start, 0.53, 0.88, 0.11 * s, 0.055 * s, 0.028 * s);

                self.btn_quick_save = VirtualButton::new_circle(VirtualButtonId::QuickSave, 0.29, 0.08, 0.036 * s);
                self.btn_menu = VirtualButton::new_circle(VirtualButtonId::Menu, 0.43, 0.08, 0.036 * s);
                self.btn_fast_forward = VirtualButton::new_circle(VirtualButtonId::FastForward, 0.57, 0.08, 0.036 * s);
                self.btn_quick_load = VirtualButton::new_circle(VirtualButtonId::QuickLoad, 0.71, 0.08, 0.036 * s);
            }
            TouchOverlayPreset::Ergonomic => {
                // Ergonomic curved arc positioning
                self.dpad = VirtualDPad::new(0.15, 0.74, 0.12 * s, 0.026 * s);
                let a_cx = 0.91;
                let a_cy = 0.66;
                let b_cx = 0.77;
                let b_cy = 0.77;
                let btn_rad = 0.058 * s;
                self.btn_a = VirtualButton::new_circle(VirtualButtonId::A, a_cx, a_cy, btn_rad);
                self.btn_b = VirtualButton::new_circle(VirtualButtonId::B, b_cx, b_cy, btn_rad);
                self.chord_ab = ChordHitbox::new((a_cx, a_cy), (b_cx, b_cy), 0.042 * s);

                self.btn_l = VirtualButton::new_pill(VirtualButtonId::L, 0.03, 0.04, 0.17 * s, 0.075 * s, 0.038 * s);
                self.btn_r = VirtualButton::new_pill(VirtualButtonId::R, 0.97 - 0.17 * s, 0.04, 0.17 * s, 0.075 * s, 0.038 * s);

                self.btn_select = VirtualButton::new_pill(VirtualButtonId::Select, 0.37, 0.89, 0.10 * s, 0.052 * s, 0.026 * s);
                self.btn_start = VirtualButton::new_pill(VirtualButtonId::Start, 0.53, 0.89, 0.10 * s, 0.052 * s, 0.026 * s);

                self.btn_quick_save = VirtualButton::new_circle(VirtualButtonId::QuickSave, 0.32, 0.075, 0.034 * s);
                self.btn_menu = VirtualButton::new_circle(VirtualButtonId::Menu, 0.44, 0.075, 0.034 * s);
                self.btn_fast_forward = VirtualButton::new_circle(VirtualButtonId::FastForward, 0.56, 0.075, 0.034 * s);
                self.btn_quick_load = VirtualButton::new_circle(VirtualButtonId::QuickLoad, 0.68, 0.075, 0.034 * s);
            }
        }

        self.dpad.dynamic_center = self.dynamic_dpad;
    }

    /// Sets the global scaling multiplier (0.5 to 2.0) and refreshes hitboxes.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(0.5, 2.0);
        self.apply_preset(self.preset);
    }

    /// Sets global overlay opacity (0.0 to 1.0).
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Toggles dynamic floating D-pad mode.
    pub fn set_dynamic_dpad(&mut self, dynamic: bool) {
        self.dynamic_dpad = dynamic;
        self.dpad.dynamic_center = dynamic;
    }

    /// Updates safe-area insets [top, bottom, left, right] normalized to [0.0, 1.0].
    pub fn set_safe_insets(&mut self, insets: [f32; 4]) {
        self.safe_insets = insets;
    }

    // --- Backward-compatibility Rect getters for existing tests/references ---
    pub fn dpad_rect(&self) -> TouchRect {
        self.dpad.to_rect()
    }
    pub fn btn_a_rect(&self) -> TouchRect {
        self.btn_a.to_rect()
    }
    pub fn btn_b_rect(&self) -> TouchRect {
        self.btn_b.to_rect()
    }
    pub fn btn_l_rect(&self) -> TouchRect {
        self.btn_l.to_rect()
    }
    pub fn btn_r_rect(&self) -> TouchRect {
        self.btn_r.to_rect()
    }
    pub fn btn_start_rect(&self) -> TouchRect {
        self.btn_start.to_rect()
    }
    pub fn btn_select_rect(&self) -> TouchRect {
        self.btn_select.to_rect()
    }

    /// Returns the active in-game pause menu state.
    pub fn menu_state(&self) -> MenuState {
        self.menu_state
    }

    /// Sets the active in-game pause menu state.
    pub fn set_menu_state(&mut self, state: MenuState) {
        self.menu_state = state;
        if state == MenuState::Hidden {
            self.pressed_menu_item = None;
            self.pressed_save_load_item = None;
            self.pressed_settings_item = None;
            self.pressed_editor_toolbar_item = None;
            self.active_drag_group = None;
            self.drag_start_touch = None;
        }
        self.recompute_state();
    }

    /// Returns currently touched menu item, if any.
    pub fn pressed_menu_item(&self) -> Option<MenuItem> {
        self.pressed_menu_item
    }

    /// Returns currently touched save/load slot item, if any.
    pub fn pressed_save_load_item(&self) -> Option<SaveLoadItem> {
        self.pressed_save_load_item
    }

    /// Returns currently touched settings item, if any.
    pub fn pressed_settings_item(&self) -> Option<SettingsItem> {
        self.pressed_settings_item
    }

    /// Returns currently touched editor toolbar item, if any.
    pub fn pressed_editor_toolbar_item(&self) -> Option<LayoutEditorToolbarItem> {
        self.pressed_editor_toolbar_item
    }

    /// Translates a control group during interactive layout editing.
    pub fn translate_control_group(&mut self, group: ControlGroup, dx: f32, dy: f32, origin: (f32, f32)) {
        match group {
            ControlGroup::Dpad => {
                let new_center = (
                    (origin.0 + dx).clamp(0.08, 0.92),
                    (origin.1 + dy).clamp(0.08, 0.92),
                );
                self.dpad.center = new_center;
            }
            ControlGroup::ActionCluster => {
                let delta_ab = (
                    self.btn_b.center().0 - self.btn_a.center().0,
                    self.btn_b.center().1 - self.btn_a.center().1,
                );
                let new_a = (
                    (origin.0 + dx).clamp(0.12, 0.92),
                    (origin.1 + dy).clamp(0.12, 0.92),
                );
                let new_b = (
                    (new_a.0 + delta_ab.0).clamp(0.05, 0.95),
                    (new_a.1 + delta_ab.1).clamp(0.05, 0.95),
                );
                self.set_btn_a_pos(new_a);
                self.set_btn_b_pos(new_b);
                self.update_chord_ab();
            }
            ControlGroup::LShoulder => {
                let new_l = (
                    (origin.0 + dx).clamp(0.08, 0.92),
                    (origin.1 + dy).clamp(0.05, 0.95),
                );
                self.set_btn_l_pos(new_l);
            }
            ControlGroup::RShoulder => {
                let new_r = (
                    (origin.0 + dx).clamp(0.08, 0.92),
                    (origin.1 + dy).clamp(0.05, 0.95),
                );
                self.set_btn_r_pos(new_r);
            }
            ControlGroup::StartSelect => {
                let delta_select = (
                    self.btn_select.center().0 - self.btn_start.center().0,
                    self.btn_select.center().1 - self.btn_start.center().1,
                );
                let new_start = (
                    (origin.0 + dx).clamp(0.10, 0.90),
                    (origin.1 + dy).clamp(0.05, 0.95),
                );
                let new_select = (
                    (new_start.0 + delta_select.0).clamp(0.05, 0.95),
                    (new_start.1 + delta_select.1).clamp(0.05, 0.95),
                );
                self.set_btn_start_pos(new_start);
                self.set_btn_select_pos(new_select);
            }
        }
    }

    pub fn set_btn_a_pos(&mut self, pos: (f32, f32)) {
        let r = self.btn_a.radius();
        self.btn_a = VirtualButton::new_circle(VirtualButtonId::A, pos.0, pos.1, r);
    }

    pub fn set_btn_b_pos(&mut self, pos: (f32, f32)) {
        let r = self.btn_b.radius();
        self.btn_b = VirtualButton::new_circle(VirtualButtonId::B, pos.0, pos.1, r);
    }

    pub fn set_btn_l_pos(&mut self, pos: (f32, f32)) {
        let s = self.scale;
        self.btn_l = VirtualButton::new_pill(
            VirtualButtonId::L,
            pos.0 - 0.08 * s,
            pos.1 - 0.035 * s,
            0.16 * s,
            0.07 * s,
            0.035 * s,
        );
    }

    pub fn set_btn_r_pos(&mut self, pos: (f32, f32)) {
        let s = self.scale;
        self.btn_r = VirtualButton::new_pill(
            VirtualButtonId::R,
            pos.0 - 0.08 * s,
            pos.1 - 0.035 * s,
            0.16 * s,
            0.07 * s,
            0.035 * s,
        );
    }

    pub fn set_btn_start_pos(&mut self, pos: (f32, f32)) {
        let s = self.scale;
        self.btn_start = VirtualButton::new_pill(
            VirtualButtonId::Start,
            pos.0 - 0.06 * s,
            pos.1 - 0.025 * s,
            0.12 * s,
            0.05 * s,
            0.025 * s,
        );
    }

    pub fn set_btn_select_pos(&mut self, pos: (f32, f32)) {
        let s = self.scale;
        self.btn_select = VirtualButton::new_pill(
            VirtualButtonId::Select,
            pos.0 - 0.06 * s,
            pos.1 - 0.025 * s,
            0.12 * s,
            0.05 * s,
            0.025 * s,
        );
    }

    pub fn update_chord_ab(&mut self) {
        let a = self.btn_a.center();
        let b = self.btn_b.center();
        self.chord_ab = ChordHitbox::new(a, b, 0.040 * self.scale);
    }

    /// Returns the active occupied slot bitmask for the shader uniform buffer.
    pub fn slot_mask(&self) -> u32 {
        self.slot_mask
    }

    /// Sets the active occupied slot bitmask.
    pub fn set_slot_mask(&mut self, mask: u32) {
        self.slot_mask = mask;
    }

    /// Ingests unified touch event with phase.
    pub fn handle_touch_event(
        &mut self,
        id: u64,
        x: f32,
        y: f32,
        phase: TouchPhase,
        screen_w: f32,
        screen_h: f32,
    ) {
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return;
        }
        let norm_x = (x / screen_w).clamp(0.0, 1.0);
        let norm_y = (y / screen_h).clamp(0.0, 1.0);

        // When in-game pause menu modal is visible, intercept all touch interactions
        match self.menu_state {
            MenuState::MainMenu => {
                match phase {
                    TouchPhase::Started => {
                        self.pressed_menu_item = self.menu_layout.hit_test(norm_x, norm_y);
                        let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Started);
                        self.active_touches.insert(id, pt);
                    }
                    TouchPhase::Moved => {
                        self.pressed_menu_item = self.menu_layout.hit_test(norm_x, norm_y);
                        if let Some(pt) = self.active_touches.get_mut(&id) {
                            pt.x = norm_x;
                            pt.y = norm_y;
                            pt.norm_x = norm_x;
                            pt.norm_y = norm_y;
                            pt.phase = TouchPhase::Moved;
                        }
                    }
                    TouchPhase::Ended => {
                        let hit = self.menu_layout.hit_test(norm_x, norm_y).or(self.pressed_menu_item);
                        if let Some(item) = hit {
                            self.pending_actions.push(TouchAction::MenuSelect(item));
                        }
                        self.pressed_menu_item = None;
                        self.active_touches.remove(&id);
                    }
                    TouchPhase::Cancelled => {
                        self.pressed_menu_item = None;
                        self.active_touches.remove(&id);
                    }
                }
                self.recompute_state();
                return;
            }
            MenuState::SaveLoadSlotSelect { mode } => {
                match phase {
                    TouchPhase::Started => {
                        self.pressed_save_load_item = self.save_load_layout.hit_test(norm_x, norm_y);
                        let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Started);
                        self.active_touches.insert(id, pt);
                    }
                    TouchPhase::Moved => {
                        self.pressed_save_load_item = self.save_load_layout.hit_test(norm_x, norm_y);
                        if let Some(pt) = self.active_touches.get_mut(&id) {
                            pt.x = norm_x;
                            pt.y = norm_y;
                            pt.norm_x = norm_x;
                            pt.norm_y = norm_y;
                            pt.phase = TouchPhase::Moved;
                        }
                    }
                    TouchPhase::Ended => {
                        let hit = self.save_load_layout.hit_test(norm_x, norm_y).or(self.pressed_save_load_item);
                        match hit {
                            Some(SaveLoadItem::Slot(slot)) => {
                                self.pending_actions.push(TouchAction::SelectSlot { slot, mode });
                            }
                            Some(SaveLoadItem::Back) => {
                                self.pending_actions.push(TouchAction::MenuBack);
                            }
                            Some(SaveLoadItem::ToggleMode) => {
                                self.pending_actions.push(TouchAction::ToggleSlotMode);
                            }
                            None => {}
                        }
                        self.pressed_save_load_item = None;
                        self.active_touches.remove(&id);
                    }
                    TouchPhase::Cancelled => {
                        self.pressed_save_load_item = None;
                        self.active_touches.remove(&id);
                    }
                }
                self.recompute_state();
                return;
            }
            MenuState::Settings => {
                match phase {
                    TouchPhase::Started => {
                        self.pressed_settings_item = self.settings_layout.hit_test(norm_x, norm_y);
                        let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Started);
                        self.active_touches.insert(id, pt);
                    }
                    TouchPhase::Moved => {
                        self.pressed_settings_item = self.settings_layout.hit_test(norm_x, norm_y);
                        if let Some(pt) = self.active_touches.get_mut(&id) {
                            pt.x = norm_x;
                            pt.y = norm_y;
                            pt.norm_x = norm_x;
                            pt.norm_y = norm_y;
                            pt.phase = TouchPhase::Moved;
                        }
                    }
                    TouchPhase::Ended => {
                        let hit = self.settings_layout.hit_test(norm_x, norm_y).or(self.pressed_settings_item);
                        if let Some(item) = hit {
                            self.pending_actions.push(TouchAction::SettingsSelect(item));
                        }
                        self.pressed_settings_item = None;
                        self.active_touches.remove(&id);
                    }
                    TouchPhase::Cancelled => {
                        self.pressed_settings_item = None;
                        self.active_touches.remove(&id);
                    }
                }
                self.recompute_state();
                return;
            }
            MenuState::LayoutEditor => {
                match phase {
                    TouchPhase::Started => {
                        // Check top toolbar first
                        if let Some(toolbar_item) = self.layout_editor_layout.hit_test(norm_x, norm_y) {
                            self.pressed_editor_toolbar_item = Some(toolbar_item);
                        } else {
                            // Hit test draggable control groups
                            if self.dpad.to_rect().contains(norm_x, norm_y) {
                                self.active_drag_group = Some((ControlGroup::Dpad, id, self.dpad.center));
                                self.drag_start_touch = Some((norm_x, norm_y));
                            } else if self.btn_a.contains(norm_x, norm_y)
                                || self.btn_b.contains(norm_x, norm_y)
                                || self.chord_ab.contains(norm_x, norm_y)
                            {
                                self.active_drag_group =
                                    Some((ControlGroup::ActionCluster, id, self.btn_a.center()));
                                self.drag_start_touch = Some((norm_x, norm_y));
                            } else if self.btn_l.contains(norm_x, norm_y) {
                                self.active_drag_group =
                                    Some((ControlGroup::LShoulder, id, self.btn_l.center()));
                                self.drag_start_touch = Some((norm_x, norm_y));
                            } else if self.btn_r.contains(norm_x, norm_y) {
                                self.active_drag_group =
                                    Some((ControlGroup::RShoulder, id, self.btn_r.center()));
                                self.drag_start_touch = Some((norm_x, norm_y));
                            } else if self.btn_start.contains(norm_x, norm_y)
                                || self.btn_select.contains(norm_x, norm_y)
                            {
                                self.active_drag_group =
                                    Some((ControlGroup::StartSelect, id, self.btn_start.center()));
                                self.drag_start_touch = Some((norm_x, norm_y));
                            }
                        }
                        let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Started);
                        self.active_touches.insert(id, pt);
                    }
                    TouchPhase::Moved => {
                        if let Some((group, drag_id, origin)) = self.active_drag_group {
                            if drag_id == id {
                                if let Some(start) = self.drag_start_touch {
                                    let dx = norm_x - start.0;
                                    let dy = norm_y - start.1;
                                    self.translate_control_group(group, dx, dy, origin);
                                }
                            }
                        }
                        if let Some(pt) = self.active_touches.get_mut(&id) {
                            pt.x = norm_x;
                            pt.y = norm_y;
                            pt.norm_x = norm_x;
                            pt.norm_y = norm_y;
                            pt.phase = TouchPhase::Moved;
                        }
                    }
                    TouchPhase::Ended => {
                        let hit_toolbar = self
                            .layout_editor_layout
                            .hit_test(norm_x, norm_y)
                            .or(self.pressed_editor_toolbar_item);
                        if let Some(toolbar_item) = hit_toolbar {
                            self.pending_actions.push(TouchAction::LayoutEditorAction(toolbar_item));
                        }
                        if let Some((_, drag_id, _)) = self.active_drag_group {
                            if drag_id == id {
                                self.active_drag_group = None;
                                self.drag_start_touch = None;
                            }
                        }
                        self.pressed_editor_toolbar_item = None;
                        self.active_touches.remove(&id);
                    }
                    TouchPhase::Cancelled => {
                        self.active_drag_group = None;
                        self.drag_start_touch = None;
                        self.pressed_editor_toolbar_item = None;
                        self.active_touches.remove(&id);
                    }
                }
                self.recompute_state();
                return;
            }
            MenuState::Hidden | MenuState::Cheats => {}
        }

        match phase {
            TouchPhase::Started => {
                let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Started);
                self.active_touches.insert(id, pt);
                self.dpad.handle_touch_down(id, norm_x, norm_y);

                // Check action button clicks on touch down
                if self.btn_fast_forward.contains(norm_x, norm_y) {
                    self.pending_actions.push(TouchAction::ToggleFastForward);
                } else if self.btn_menu.contains(norm_x, norm_y) {
                    self.pending_actions.push(TouchAction::OpenMenu);
                } else if self.btn_quick_save.contains(norm_x, norm_y) {
                    self.pending_actions.push(TouchAction::QuickSave);
                } else if self.btn_quick_load.contains(norm_x, norm_y) {
                    self.pending_actions.push(TouchAction::QuickLoad);
                }
            }
            TouchPhase::Moved => {
                if let Some(pt) = self.active_touches.get_mut(&id) {
                    pt.x = norm_x;
                    pt.y = norm_y;
                    pt.norm_x = norm_x;
                    pt.norm_y = norm_y;
                    pt.phase = TouchPhase::Moved;
                } else {
                    let pt = TouchPoint::new(id, norm_x, norm_y, TouchPhase::Moved);
                    self.active_touches.insert(id, pt);
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.active_touches.remove(&id);
                self.dpad.handle_touch_up(id);
            }
        }

        self.recompute_state();
    }

    /// Ingests touch start / finger down event.
    pub fn handle_touch_down(&mut self, id: u64, x: f32, y: f32, screen_w: f32, screen_h: f32) {
        self.handle_touch_event(id, x, y, TouchPhase::Started, screen_w, screen_h);
    }

    /// Ingests touch move / finger dragged event.
    pub fn handle_touch_move(&mut self, id: u64, x: f32, y: f32, screen_w: f32, screen_h: f32) {
        self.handle_touch_event(id, x, y, TouchPhase::Moved, screen_w, screen_h);
    }

    /// Ingests touch release / finger up event.
    pub fn handle_touch_up(&mut self, id: u64) {
        if let Some(pt) = self.active_touches.get(&id) {
            let norm_x = pt.norm_x;
            let norm_y = pt.norm_y;
            self.handle_touch_event(id, norm_x, norm_y, TouchPhase::Ended, 1.0, 1.0);
        } else {
            self.active_touches.remove(&id);
            self.dpad.handle_touch_up(id);
            self.recompute_state();
        }
    }

    /// Ingests touch cancellation event.
    pub fn handle_touch_cancel(&mut self, id: u64) {
        if let Some(pt) = self.active_touches.get(&id) {
            let norm_x = pt.norm_x;
            let norm_y = pt.norm_y;
            self.handle_touch_event(id, norm_x, norm_y, TouchPhase::Cancelled, 1.0, 1.0);
        } else {
            self.active_touches.remove(&id);
            self.dpad.handle_touch_up(id);
            self.recompute_state();
        }
    }

    /// Clears all active touch pointers.
    pub fn clear(&mut self) {
        self.active_touches.clear();
        self.dpad.active_center = None;
        self.dpad.tracking_touch_id = None;
        self.pending_actions.clear();
        self.pressed_mask = 0;
    }

    /// Recomputes internal pressed bitmask for fast rendering and polling.
    fn recompute_state(&mut self) {
        if !self.visible || self.menu_state != MenuState::Hidden {
            self.pressed_mask = 0;
            return;
        }

        let mut mask = 0u32;

        for pt in self.active_touches.values() {
            let px = pt.norm_x;
            let py = pt.norm_y;

            // 1. D-Pad Direction Evaluation
            let (up, down, left, right) = self.dpad.calculate_direction(px, py);
            if up {
                mask |= touch_bits::DPAD_UP;
            }
            if down {
                mask |= touch_bits::DPAD_DOWN;
            }
            if left {
                mask |= touch_bits::DPAD_LEFT;
            }
            if right {
                mask |= touch_bits::DPAD_RIGHT;
            }

            // 2. Chord A+B Evaluation (Bridge region between A and B)
            if self.chord_ab.contains(px, py) {
                mask |= touch_bits::BTN_A | touch_bits::BTN_B | touch_bits::CHORD_AB;
            }

            // 3. Face Action Buttons
            if self.btn_a.contains(px, py) {
                mask |= touch_bits::BTN_A;
            }
            if self.btn_b.contains(px, py) {
                mask |= touch_bits::BTN_B;
            }

            // 4. Shoulder Buttons
            if self.btn_l.contains(px, py) {
                mask |= touch_bits::BTN_L;
            }
            if self.btn_r.contains(px, py) {
                mask |= touch_bits::BTN_R;
            }

            // 5. System Buttons
            if self.btn_start.contains(px, py) {
                mask |= touch_bits::BTN_START;
            }
            if self.btn_select.contains(px, py) {
                mask |= touch_bits::BTN_SELECT;
            }

            // 6. Quick Action HUD Elements
            if self.btn_fast_forward.contains(px, py) {
                mask |= touch_bits::BTN_FAST_FORWARD;
            }
            if self.btn_menu.contains(px, py) {
                mask |= touch_bits::BTN_MENU;
            }
            if self.btn_quick_save.contains(px, py) {
                mask |= touch_bits::BTN_QUICK_SAVE;
            }
            if self.btn_quick_load.contains(px, py) {
                mask |= touch_bits::BTN_QUICK_LOAD;
            }
        }

        self.pressed_mask = mask;
    }

    /// Retrieves the raw 32-bit pressed element bitmask for the WGPU shader uniform buffer.
    pub fn pressed_bitmask(&self) -> u32 {
        self.pressed_mask
    }

    /// Checks if a specific virtual button is currently pressed.
    pub fn is_button_pressed(&self, id: VirtualButtonId) -> bool {
        match id {
            VirtualButtonId::A => (self.pressed_mask & touch_bits::BTN_A) != 0,
            VirtualButtonId::B => (self.pressed_mask & touch_bits::BTN_B) != 0,
            VirtualButtonId::L => (self.pressed_mask & touch_bits::BTN_L) != 0,
            VirtualButtonId::R => (self.pressed_mask & touch_bits::BTN_R) != 0,
            VirtualButtonId::Start => (self.pressed_mask & touch_bits::BTN_START) != 0,
            VirtualButtonId::Select => (self.pressed_mask & touch_bits::BTN_SELECT) != 0,
            VirtualButtonId::FastForward => (self.pressed_mask & touch_bits::BTN_FAST_FORWARD) != 0,
            VirtualButtonId::Menu => (self.pressed_mask & touch_bits::BTN_MENU) != 0,
            VirtualButtonId::QuickSave => (self.pressed_mask & touch_bits::BTN_QUICK_SAVE) != 0,
            VirtualButtonId::QuickLoad => (self.pressed_mask & touch_bits::BTN_QUICK_LOAD) != 0,
        }
    }

    /// Number of active touch pointers currently on screen.
    pub fn active_touch_count(&self) -> usize {
        self.active_touches.len()
    }

    /// Drains any pending non-joypad actions (e.g. fast-forward toggle, menu open).
    pub fn poll_actions(&mut self) -> Vec<TouchAction> {
        let actions = std::mem::take(&mut self.pending_actions);
        actions
    }

    /// Returns the 32-bit mask of virtual buttons that transitioned from unpressed to pressed
    /// since the last call, updating the previous mask cache.
    pub fn poll_newly_pressed_bits(&mut self) -> u32 {
        let newly_pressed = self.pressed_mask & !self.prev_pressed_mask;
        self.prev_pressed_mask = self.pressed_mask;
        newly_pressed
    }

    /// Checks if any virtual button has transitioned from unpressed to pressed.
    pub fn has_new_press(&mut self) -> bool {
        self.poll_newly_pressed_bits() != 0
    }

    /// Sets whether tactile haptic feedback is enabled for on-screen touch button presses.
    pub fn set_haptics_enabled(&mut self, enabled: bool) {
        self.haptics_enabled = enabled;
    }

    /// Returns whether tactile haptic feedback is enabled.
    pub fn is_haptics_enabled(&self) -> bool {
        self.haptics_enabled
    }
}

impl InputSource for TouchInputManager {
    fn name(&self) -> &'static str {
        "TouchOverlay"
    }

    fn poll(&mut self) -> JoypadState {
        self.recompute_state();
        let mut state = JoypadState::default();
        if !self.visible || self.menu_state != MenuState::Hidden {
            return state;
        }

        if (self.pressed_mask & touch_bits::BTN_A) != 0 {
            state.set_pressed(Button::A, true);
        }
        if (self.pressed_mask & touch_bits::BTN_B) != 0 {
            state.set_pressed(Button::B, true);
        }
        if (self.pressed_mask & touch_bits::BTN_SELECT) != 0 {
            state.set_pressed(Button::Select, true);
        }
        if (self.pressed_mask & touch_bits::BTN_START) != 0 {
            state.set_pressed(Button::Start, true);
        }
        if (self.pressed_mask & touch_bits::DPAD_RIGHT) != 0 {
            state.set_pressed(Button::Right, true);
        }
        if (self.pressed_mask & touch_bits::DPAD_LEFT) != 0 {
            state.set_pressed(Button::Left, true);
        }
        if (self.pressed_mask & touch_bits::DPAD_UP) != 0 {
            state.set_pressed(Button::Up, true);
        }
        if (self.pressed_mask & touch_bits::DPAD_DOWN) != 0 {
            state.set_pressed(Button::Down, true);
        }
        if (self.pressed_mask & touch_bits::BTN_R) != 0 {
            state.set_pressed(Button::R, true);
        }
        if (self.pressed_mask & touch_bits::BTN_L) != 0 {
            state.set_pressed(Button::L, true);
        }

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_overlay_dpad_resolution() {
        let mut overlay = TouchInputManager::new();
        assert_eq!(overlay.poll().to_bits(), 0);

        let screen_w = 1000.0;
        let screen_h = 1000.0;

        // Touch right side of D-pad
        let dpad_right_x = (overlay.dpad.center.0 + overlay.dpad.radius * 0.70) * screen_w;
        let dpad_right_y = overlay.dpad.center.1 * screen_h;
        overlay.handle_touch_down(1, dpad_right_x, dpad_right_y, screen_w, screen_h);

        let state = overlay.poll();
        assert!(state.is_pressed(Button::Right));
        assert!(!state.is_pressed(Button::Left));
        assert!(!state.is_pressed(Button::Up));

        // Touch button A with second finger
        let a_x = overlay.btn_a.center().0 * screen_w;
        let a_y = overlay.btn_a.center().1 * screen_h;
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

    #[test]
    fn test_touch_overlay_dpad_8way_angles() {
        let dpad = VirtualDPad::new(0.5, 0.5, 0.2, 0.03);

        // Center deadzone
        let (up, down, left, right) = dpad.calculate_direction(0.5, 0.5);
        assert!(!up && !down && !left && !right);

        // Cardinal Up
        let (up, down, left, right) = dpad.calculate_direction(0.5, 0.35);
        assert!(up && !down && !left && !right);

        // Cardinal Down
        let (up, down, left, right) = dpad.calculate_direction(0.5, 0.65);
        assert!(!up && down && !left && !right);

        // Cardinal Left
        let (up, down, left, right) = dpad.calculate_direction(0.35, 0.5);
        assert!(!up && !down && left && !right);

        // Cardinal Right
        let (up, down, left, right) = dpad.calculate_direction(0.65, 0.5);
        assert!(!up && !down && !left && right);

        // Diagonal Up-Right
        let (up, down, left, right) = dpad.calculate_direction(0.62, 0.38);
        assert!(up && !down && !left && right);

        // Diagonal Up-Left
        let (up, down, left, right) = dpad.calculate_direction(0.38, 0.38);
        assert!(up && !down && left && !right);

        // Diagonal Down-Right
        let (up, down, left, right) = dpad.calculate_direction(0.62, 0.62);
        assert!(!up && down && !left && right);

        // Diagonal Down-Left
        let (up, down, left, right) = dpad.calculate_direction(0.38, 0.62);
        assert!(!up && down && left && !right);
    }

    #[test]
    fn test_touch_overlay_chord_ab_bridge() {
        let mut overlay = TouchInputManager::new();
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        let a_pos = overlay.btn_a.center();
        let b_pos = overlay.btn_b.center();
        // Midpoint between A and B
        let mid_x = (a_pos.0 + b_pos.0) * 0.5 * screen_w;
        let mid_y = (a_pos.1 + b_pos.1) * 0.5 * screen_h;

        overlay.handle_touch_down(1, mid_x, mid_y, screen_w, screen_h);
        let state = overlay.poll();
        assert!(state.is_pressed(Button::A), "Chord should activate button A");
        assert!(state.is_pressed(Button::B), "Chord should activate button B");
        assert_eq!(
            overlay.pressed_bitmask() & touch_bits::CHORD_AB,
            touch_bits::CHORD_AB
        );

        overlay.handle_touch_up(1);
        assert_eq!(overlay.poll().to_bits(), 0);
    }

    #[test]
    fn test_touch_overlay_multi_finger_chords() {
        let mut overlay = TouchInputManager::new();
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        // Finger 1: D-pad Diagonal Up-Right
        let dpad_ur_x = (overlay.dpad.center.0 + overlay.dpad.radius * 0.6) * screen_w;
        let dpad_ur_y = (overlay.dpad.center.1 - overlay.dpad.radius * 0.6) * screen_h;
        overlay.handle_touch_down(10, dpad_ur_x, dpad_ur_y, screen_w, screen_h);

        // Finger 2: Hold B
        let b_x = overlay.btn_b.center().0 * screen_w;
        let b_y = overlay.btn_b.center().1 * screen_h;
        overlay.handle_touch_down(20, b_x, b_y, screen_w, screen_h);

        // Finger 3: Tap A
        let a_x = overlay.btn_a.center().0 * screen_w;
        let a_y = overlay.btn_a.center().1 * screen_h;
        overlay.handle_touch_down(30, a_x, a_y, screen_w, screen_h);

        let state = overlay.poll();
        assert!(state.is_pressed(Button::Up));
        assert!(state.is_pressed(Button::Right));
        assert!(state.is_pressed(Button::B));
        assert!(state.is_pressed(Button::A));
        assert_eq!(overlay.active_touch_count(), 3);

        // Release A (tap ended)
        overlay.handle_touch_up(30);
        let state_after_a_release = overlay.poll();
        assert!(state_after_a_release.is_pressed(Button::Up));
        assert!(state_after_a_release.is_pressed(Button::Right));
        assert!(state_after_a_release.is_pressed(Button::B));
        assert!(!state_after_a_release.is_pressed(Button::A));

        // Cancellation of all remaining touches
        overlay.handle_touch_cancel(10);
        overlay.handle_touch_cancel(20);
        assert_eq!(overlay.poll().to_bits(), 0);
        assert_eq!(overlay.active_touch_count(), 0);
    }

    #[test]
    fn test_touch_overlay_dynamic_dpad_floating_center() {
        let mut overlay = TouchInputManager::new();
        overlay.set_dynamic_dpad(true);
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        // Touch down slightly offset from default D-pad center
        let initial_x = 0.16 * screen_w;
        let initial_y = 0.78 * screen_h;
        overlay.handle_touch_down(1, initial_x, initial_y, screen_w, screen_h);

        // Right at the touch down point: deadzone -> no direction
        let state = overlay.poll();
        assert_eq!(state.to_bits(), 0);

        // Drag finger right relative to dynamic floating center
        let dragged_x = (0.16 + 0.06) * screen_w;
        let dragged_y = 0.78 * screen_h;
        overlay.handle_touch_move(1, dragged_x, dragged_y, screen_w, screen_h);

        let state_dragged = overlay.poll();
        assert!(state_dragged.is_pressed(Button::Right));
        assert!(!state_dragged.is_pressed(Button::Left));

        overlay.handle_touch_up(1);
        assert_eq!(overlay.poll().to_bits(), 0);
    }

    #[test]
    fn test_touch_overlay_presets_and_scaling() {
        let mut overlay = TouchInputManager::new();
        assert_eq!(overlay.preset, TouchOverlayPreset::Standard);

        overlay.apply_preset(TouchOverlayPreset::Compact);
        assert_eq!(overlay.preset, TouchOverlayPreset::Compact);

        overlay.apply_preset(TouchOverlayPreset::Wide);
        assert_eq!(overlay.preset, TouchOverlayPreset::Wide);

        overlay.apply_preset(TouchOverlayPreset::Ergonomic);
        assert_eq!(overlay.preset, TouchOverlayPreset::Ergonomic);

        overlay.set_scale(1.5);
        assert_eq!(overlay.scale, 1.5);

        overlay.set_opacity(0.8);
        assert_eq!(overlay.opacity, 0.8);
    }

    #[test]
    fn test_touch_actions_fast_forward_and_menu() {
        let mut overlay = TouchInputManager::new();
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        let ff_pos = overlay.btn_fast_forward.center();
        overlay.handle_touch_down(1, ff_pos.0 * screen_w, ff_pos.1 * screen_h, screen_w, screen_h);

        let actions = overlay.poll_actions();
        assert_eq!(actions, vec![TouchAction::ToggleFastForward]);

        let menu_pos = overlay.btn_menu.center();
        overlay.handle_touch_down(2, menu_pos.0 * screen_w, menu_pos.1 * screen_h, screen_w, screen_h);
        let actions2 = overlay.poll_actions();
        assert_eq!(actions2, vec![TouchAction::OpenMenu]);

        let qs_pos = overlay.btn_quick_save.center();
        overlay.handle_touch_down(3, qs_pos.0 * screen_w, qs_pos.1 * screen_h, screen_w, screen_h);
        let actions3 = overlay.poll_actions();
        assert_eq!(actions3, vec![TouchAction::QuickSave]);

        let ql_pos = overlay.btn_quick_load.center();
        overlay.handle_touch_down(4, ql_pos.0 * screen_w, ql_pos.1 * screen_h, screen_w, screen_h);
        let actions4 = overlay.poll_actions();
        assert_eq!(actions4, vec![TouchAction::QuickLoad]);
    }

    #[test]
    fn test_touch_overlay_modal_menu_interactions() {
        let mut overlay = TouchInputManager::new();
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        assert_eq!(overlay.menu_state(), MenuState::Hidden);
        assert_eq!(overlay.pressed_menu_item(), None);

        // Open menu
        overlay.set_menu_state(MenuState::MainMenu);
        assert_eq!(overlay.menu_state(), MenuState::MainMenu);

        // Virtual gamepad inputs should be completely suppressed while menu is open
        let a_pos = overlay.btn_a.center();
        overlay.handle_touch_down(10, a_pos.0 * screen_w, a_pos.1 * screen_h, screen_w, screen_h);
        assert_eq!(overlay.poll().to_bits(), 0);

        // Touch on Resume menu item
        let resume_rect = overlay.menu_layout.item_rects[0].1;
        let (rx, ry) = resume_rect.center();
        overlay.handle_touch_down(1, rx * screen_w, ry * screen_h, screen_w, screen_h);
        assert_eq!(overlay.pressed_menu_item(), Some(MenuItem::Resume));

        // Touch up on Resume dispatches MenuSelect(Resume)
        overlay.handle_touch_event(1, rx * screen_w, ry * screen_h, TouchPhase::Ended, screen_w, screen_h);
        let actions = overlay.poll_actions();
        assert_eq!(actions, vec![TouchAction::MenuSelect(MenuItem::Resume)]);
        assert_eq!(overlay.pressed_menu_item(), None);

        // Touch on LoadRom menu item
        let load_rect = overlay.menu_layout.item_rects[1].1;
        let (lx, ly) = load_rect.center();
        overlay.handle_touch_down(2, lx * screen_w, ly * screen_h, screen_w, screen_h);
        assert_eq!(overlay.pressed_menu_item(), Some(MenuItem::LoadRom));
        overlay.handle_touch_event(2, lx * screen_w, ly * screen_h, TouchPhase::Ended, screen_w, screen_h);
        assert_eq!(overlay.poll_actions(), vec![TouchAction::MenuSelect(MenuItem::LoadRom)]);

        // Tap outside modal does NOT close menu (stays open until Resume is tapped)
        overlay.handle_touch_event(3, 0.05 * screen_w, 0.05 * screen_h, TouchPhase::Ended, screen_w, screen_h);
        assert_eq!(overlay.poll_actions(), vec![]);
        assert_eq!(overlay.menu_state(), MenuState::MainMenu);
    }

    #[test]
    fn test_touch_overlay_save_load_slot_modal_interactions() {
        let mut overlay = TouchInputManager::new();
        let screen_w = 1000.0;
        let screen_h = 1000.0;

        overlay.set_menu_state(MenuState::SaveLoadSlotSelect { mode: SlotMode::Save });
        assert_eq!(
            overlay.menu_state(),
            MenuState::SaveLoadSlotSelect { mode: SlotMode::Save }
        );
        overlay.set_slot_mask(0b00101);
        assert_eq!(overlay.slot_mask(), 0b00101);

        // Touch on Slot 1
        let (s1_x, s1_y) = overlay.save_load_layout.slot_rects[0].1.center();
        overlay.handle_touch_down(1, s1_x * screen_w, s1_y * screen_h, screen_w, screen_h);
        assert_eq!(overlay.pressed_save_load_item(), Some(SaveLoadItem::Slot(1)));

        // Touch up on Slot 1
        overlay.handle_touch_event(1, s1_x * screen_w, s1_y * screen_h, TouchPhase::Ended, screen_w, screen_h);
        let actions = overlay.poll_actions();
        assert_eq!(actions, vec![TouchAction::SelectSlot { slot: 1, mode: SlotMode::Save }]);
        assert_eq!(overlay.pressed_save_load_item(), None);

        // Touch on Mode Toggle button
        let (tx, ty) = overlay.save_load_layout.toggle_mode_rect.center();
        overlay.handle_touch_down(2, tx * screen_w, ty * screen_h, screen_w, screen_h);
        assert_eq!(overlay.pressed_save_load_item(), Some(SaveLoadItem::ToggleMode));
        overlay.handle_touch_event(2, tx * screen_w, ty * screen_h, TouchPhase::Ended, screen_w, screen_h);
        assert_eq!(overlay.poll_actions(), vec![TouchAction::ToggleSlotMode]);

        // Touch on Back button
        let (bx, by) = overlay.save_load_layout.back_rect.center();
        overlay.handle_touch_down(3, bx * screen_w, by * screen_h, screen_w, screen_h);
        assert_eq!(overlay.pressed_save_load_item(), Some(SaveLoadItem::Back));
        overlay.handle_touch_event(3, bx * screen_w, by * screen_h, TouchPhase::Ended, screen_w, screen_h);
        assert_eq!(overlay.poll_actions(), vec![TouchAction::MenuBack]);
    }
}
