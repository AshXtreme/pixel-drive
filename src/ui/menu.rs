//! In-Game Modal Pause Menu & Multi-Slot Save State Manager for PixelDrive.
//!
//! Provides state management, item hierarchy, normalized hitboxes, and action dispatching
//! for the in-game modal pause menu and multi-slot save/load manager (Slots 1–5).

use crate::input::TouchRect;

/// Save/Load operating mode for the multi-slot state selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SlotMode {
    #[default]
    Save,
    Load,
}

impl SlotMode {
    pub fn toggle(&self) -> Self {
        match self {
            SlotMode::Save => SlotMode::Load,
            SlotMode::Load => SlotMode::Save,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            SlotMode::Save => "SAVE STATE TO SLOT",
            SlotMode::Load => "LOAD STATE FROM SLOT",
        }
    }
}

/// Current display state of the in-game pause menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuState {
    #[default]
    Hidden,
    MainMenu,
    SaveLoadSlotSelect { mode: SlotMode },
    Settings,
    LayoutEditor,
    FastForwardSelect,
    Cheats,
}

impl MenuState {
    /// Returns true if any modal menu is active and visible.
    pub fn is_visible(&self) -> bool {
        *self != MenuState::Hidden
    }

    /// WGSL shader state index.
    pub fn shader_index(&self) -> u32 {
        match self {
            MenuState::Hidden => 0,
            MenuState::MainMenu => 1,
            MenuState::SaveLoadSlotSelect { mode: SlotMode::Save } => 2,
            MenuState::SaveLoadSlotSelect { mode: SlotMode::Load } => 3,
            MenuState::Settings => 4,
            MenuState::LayoutEditor => 5,
            MenuState::FastForwardSelect => 6,
            MenuState::Cheats => 7,
        }
    }
}

/// Selectable items within the Main Menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuItem {
    Resume,
    LoadRom,
    SaveLoadStates,
    ResetGame,
    Settings,
    Cheats,
}

impl MenuItem {
    /// 1-based index matching the WGSL shader uniform constants.
    pub fn shader_index(&self) -> u32 {
        match self {
            MenuItem::Resume => 1,
            MenuItem::LoadRom => 2,
            MenuItem::SaveLoadStates => 3,
            MenuItem::ResetGame => 4,
            MenuItem::Settings => 5,
            MenuItem::Cheats => 6,
        }
    }

    /// Primary display label for the item.
    pub fn label(&self) -> &'static str {
        match self {
            MenuItem::Resume => "Resume Game",
            MenuItem::LoadRom => "Load New ROM",
            MenuItem::SaveLoadStates => "Save / Load States",
            MenuItem::ResetGame => "Reset Game",
            MenuItem::Settings => "Settings",
            MenuItem::Cheats => "👾 Cheat Codes",
        }
    }

    /// Subtitle or phase status indicator.
    pub fn subtitle(&self) -> Option<&'static str> {
        match self {
            MenuItem::Cheats => Some("Action Replay / GameShark"),
            _ => None,
        }
    }
}

/// Selectable interactive targets within the Save/Load Slot Selector screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SaveLoadItem {
    Slot(u8), // 1..=5
    Back,
    ToggleMode,
}

impl SaveLoadItem {
    pub fn shader_index(&self) -> u32 {
        match self {
            SaveLoadItem::Slot(s) => *s as u32,
            SaveLoadItem::Back => 6,
            SaveLoadItem::ToggleMode => 7,
        }
    }
}

/// Selectable interactive items within the Settings screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsItem {
    CustomizeControls,
    Opacity,
    Scale,
    Theme,
    FastForwardSpeed,
    Back,
}

impl SettingsItem {
    pub fn shader_index(&self) -> u32 {
        match self {
            SettingsItem::CustomizeControls => 1,
            SettingsItem::Opacity => 2,
            SettingsItem::Scale => 3,
            SettingsItem::Theme => 4,
            SettingsItem::FastForwardSpeed => 5,
            SettingsItem::Back => 6,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SettingsItem::CustomizeControls => "Customize Controls",
            SettingsItem::Opacity => "Button Opacity",
            SettingsItem::Scale => "Overall Scale",
            SettingsItem::Theme => "UI Theme",
            SettingsItem::FastForwardSpeed => "Fast-Forward Speed",
            SettingsItem::Back => "Back",
        }
    }
}

/// Selectable items within the Fast-Forward Speed Selection modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FastForwardItem {
    Normal,
    Speed2x,
    Speed4x,
    Speed8x,
    Uncapped,
    Back,
}

impl FastForwardItem {
    pub fn shader_index(&self) -> u32 {
        match self {
            FastForwardItem::Normal => 1,
            FastForwardItem::Speed2x => 2,
            FastForwardItem::Speed4x => 3,
            FastForwardItem::Speed8x => 4,
            FastForwardItem::Uncapped => 5,
            FastForwardItem::Back => 6,
        }
    }
}

/// Selectable toolbar buttons in the Layout Editor screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutEditorToolbarItem {
    Save,
    ResetDefaults,
    Cancel,
}

impl LayoutEditorToolbarItem {
    pub fn shader_index(&self) -> u32 {
        match self {
            LayoutEditorToolbarItem::Save => 1,
            LayoutEditorToolbarItem::ResetDefaults => 2,
            LayoutEditorToolbarItem::Cancel => 3,
        }
    }
}

/// Actions dispatched by menu interaction to the main runtime loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction {
    Resume,
    LoadRom,
    SaveLoad,
    Reset,
    Settings,
    Cheats,
    Close,
    SelectSlot { slot: u8, mode: SlotMode },
    ToggleSlotMode,
    BackToMainMenu,
    OpenLayoutEditor,
    OpenFastForwardSelect,
    SelectFastForward(u8),
    SetOpacity(f32),
    SetScale(f32),
    CycleTheme,
    SaveLayout,
    ResetLayout,
    CancelLayout,
}

/// Normalized layout geometry for the Main Menu modal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MenuLayout {
    pub modal_rect: TouchRect,
    pub item_rects: [(MenuItem, TouchRect); 6],
}

impl Default for MenuLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuLayout {
    /// Constructs default centered modal layout.
    pub fn new() -> Self {
        let modal_x = 0.22;
        let modal_y = 0.10;
        let modal_w = 0.56;
        let modal_h = 0.80;

        let btn_x = 0.26;
        let btn_w = 0.48;
        let btn_h = 0.082;
        let start_y = 0.205;
        let gap_y = 0.096;

        Self {
            modal_rect: TouchRect::new(modal_x, modal_y, modal_w, modal_h),
            item_rects: [
                (MenuItem::Resume, TouchRect::new(btn_x, start_y, btn_w, btn_h)),
                (MenuItem::LoadRom, TouchRect::new(btn_x, start_y + gap_y, btn_w, btn_h)),
                (MenuItem::SaveLoadStates, TouchRect::new(btn_x, start_y + gap_y * 2.0, btn_w, btn_h)),
                (MenuItem::ResetGame, TouchRect::new(btn_x, start_y + gap_y * 3.0, btn_w, btn_h)),
                (MenuItem::Settings, TouchRect::new(btn_x, start_y + gap_y * 4.0, btn_w, btn_h)),
                (MenuItem::Cheats, TouchRect::new(btn_x, start_y + gap_y * 5.0, btn_w, btn_h)),
            ],
        }
    }

    /// Tests if a normalized point (px, py) touches any menu item hitbox.
    pub fn hit_test(&self, px: f32, py: f32) -> Option<MenuItem> {
        for (item, rect) in &self.item_rects {
            if rect.contains(px, py) {
                return Some(*item);
            }
        }
        None
    }

    /// Checks if a normalized point (px, py) is strictly outside the modal card.
    pub fn is_outside_modal(&self, px: f32, py: f32) -> bool {
        !self.modal_rect.contains(px, py)
    }
}

/// Normalized layout geometry for the Settings Menu modal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SettingsLayout {
    pub modal_rect: TouchRect,
    pub item_rects: [(SettingsItem, TouchRect); 6],
}

impl Default for SettingsLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsLayout {
    pub const SLIDER_X: f32 = 0.44;
    pub const SLIDER_WIDTH: f32 = 0.28;

    pub fn new() -> Self {
        let modal_x = 0.22;
        let modal_y = 0.10;
        let modal_w = 0.56;
        let modal_h = 0.80;

        let btn_x = 0.26;
        let btn_w = 0.48;
        let btn_h = 0.082;
        let start_y = 0.205;
        let gap_y = 0.096;

        Self {
            modal_rect: TouchRect::new(modal_x, modal_y, modal_w, modal_h),
            item_rects: [
                (SettingsItem::CustomizeControls, TouchRect::new(btn_x, start_y, btn_w, btn_h)),
                (SettingsItem::Opacity, TouchRect::new(btn_x, start_y + gap_y, btn_w, btn_h)),
                (SettingsItem::Scale, TouchRect::new(btn_x, start_y + gap_y * 2.0, btn_w, btn_h)),
                (SettingsItem::Theme, TouchRect::new(btn_x, start_y + gap_y * 3.0, btn_w, btn_h)),
                (SettingsItem::FastForwardSpeed, TouchRect::new(btn_x, start_y + gap_y * 4.0, btn_w, btn_h)),
                (SettingsItem::Back, TouchRect::new(btn_x, start_y + gap_y * 5.0, btn_w, btn_h)),
            ],
        }
    }

    pub fn hit_test(&self, px: f32, py: f32) -> Option<SettingsItem> {
        for (item, rect) in &self.item_rects {
            if rect.contains(px, py) {
                return Some(*item);
            }
        }
        None
    }

    /// Checks if a point touches the Opacity slider track.
    pub fn is_opacity_slider(&self, px: f32, py: f32) -> bool {
        let op_rect = self.item_rects[1].1;
        py >= op_rect.y && py <= (op_rect.y + op_rect.height) && px >= Self::SLIDER_X
    }

    /// Converts normalized touch X to opacity value (0.15 to 1.00).
    pub fn calculate_opacity(px: f32) -> f32 {
        let t = ((px - Self::SLIDER_X) / Self::SLIDER_WIDTH).clamp(0.0, 1.0);
        0.15 + t * 0.85
    }

    /// Checks if a point touches the Scale slider track.
    pub fn is_scale_slider(&self, px: f32, py: f32) -> bool {
        let sc_rect = self.item_rects[2].1;
        py >= sc_rect.y && py <= (sc_rect.y + sc_rect.height) && px >= Self::SLIDER_X
    }

    /// Converts normalized touch X to scale value (0.60 to 1.50).
    pub fn calculate_scale(px: f32) -> f32 {
        let t = ((px - Self::SLIDER_X) / Self::SLIDER_WIDTH).clamp(0.0, 1.0);
        0.60 + t * 0.90
    }

    pub fn is_outside_modal(&self, px: f32, py: f32) -> bool {
        !self.modal_rect.contains(px, py)
    }
}

/// Normalized layout geometry for the Fast-Forward Speed Selection modal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FastForwardLayout {
    pub modal_rect: TouchRect,
    pub item_rects: [(FastForwardItem, TouchRect); 6],
}

impl Default for FastForwardLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl FastForwardLayout {
    pub fn new() -> Self {
        let modal_x = 0.22;
        let modal_y = 0.10;
        let modal_w = 0.56;
        let modal_h = 0.80;

        let btn_x = 0.26;
        let btn_w = 0.48;
        let btn_h = 0.082;
        let start_y = 0.205;
        let gap_y = 0.096;

        Self {
            modal_rect: TouchRect::new(modal_x, modal_y, modal_w, modal_h),
            item_rects: [
                (FastForwardItem::Normal, TouchRect::new(btn_x, start_y, btn_w, btn_h)),
                (FastForwardItem::Speed2x, TouchRect::new(btn_x, start_y + gap_y, btn_w, btn_h)),
                (FastForwardItem::Speed4x, TouchRect::new(btn_x, start_y + gap_y * 2.0, btn_w, btn_h)),
                (FastForwardItem::Speed8x, TouchRect::new(btn_x, start_y + gap_y * 3.0, btn_w, btn_h)),
                (FastForwardItem::Uncapped, TouchRect::new(btn_x, start_y + gap_y * 4.0, btn_w, btn_h)),
                (FastForwardItem::Back, TouchRect::new(btn_x, start_y + gap_y * 5.0, btn_w, btn_h)),
            ],
        }
    }

    pub fn hit_test(&self, px: f32, py: f32) -> Option<FastForwardItem> {
        for (item, rect) in &self.item_rects {
            if rect.contains(px, py) {
                return Some(*item);
            }
        }
        None
    }

    pub fn is_outside_modal(&self, px: f32, py: f32) -> bool {
        !self.modal_rect.contains(px, py)
    }
}

/// Normalized layout geometry for the Layout Editor top toolbar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutEditorLayout {
    pub toolbar_rect: TouchRect,
    pub save_rect: TouchRect,
    pub reset_rect: TouchRect,
    pub cancel_rect: TouchRect,
}

impl Default for LayoutEditorLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutEditorLayout {
    pub fn new() -> Self {
        let bar_y = 0.86;
        let bar_h = 0.085;
        let btn_w = 0.22;
        let btn_h = 0.065;

        Self {
            toolbar_rect: TouchRect::new(0.10, bar_y, 0.80, bar_h),
            save_rect: TouchRect::new(0.14, bar_y + 0.010, btn_w, btn_h),
            reset_rect: TouchRect::new(0.39, bar_y + 0.010, btn_w, btn_h),
            cancel_rect: TouchRect::new(0.64, bar_y + 0.010, btn_w, btn_h),
        }
    }

    pub fn hit_test(&self, px: f32, py: f32) -> Option<LayoutEditorToolbarItem> {
        if self.save_rect.contains(px, py) {
            return Some(LayoutEditorToolbarItem::Save);
        }
        if self.reset_rect.contains(px, py) {
            return Some(LayoutEditorToolbarItem::ResetDefaults);
        }
        if self.cancel_rect.contains(px, py) {
            return Some(LayoutEditorToolbarItem::Cancel);
        }
        None
    }
}

/// Normalized layout geometry for the Multi-Slot Save/Load Modal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SaveLoadLayout {
    pub modal_rect: TouchRect,
    pub slot_rects: [(u8, TouchRect); 5],
    pub back_rect: TouchRect,
    pub toggle_mode_rect: TouchRect,
}

impl Default for SaveLoadLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl SaveLoadLayout {
    /// Constructs centered multi-slot modal layout with 5 slot rows and bottom actions.
    pub fn new() -> Self {
        let modal_x = 0.22;
        let modal_y = 0.10;
        let modal_w = 0.56;
        let modal_h = 0.80;

        let btn_x = 0.26;
        let btn_w = 0.48;
        let btn_h = 0.082;
        let start_y = 0.205;
        let gap_y = 0.096;

        let bottom_y = 0.725;
        let bottom_h = 0.065;
        let half_btn_w = 0.225;

        Self {
            modal_rect: TouchRect::new(modal_x, modal_y, modal_w, modal_h),
            slot_rects: [
                (1, TouchRect::new(btn_x, start_y, btn_w, btn_h)),
                (2, TouchRect::new(btn_x, start_y + gap_y, btn_w, btn_h)),
                (3, TouchRect::new(btn_x, start_y + gap_y * 2.0, btn_w, btn_h)),
                (4, TouchRect::new(btn_x, start_y + gap_y * 3.0, btn_w, btn_h)),
                (5, TouchRect::new(btn_x, start_y + gap_y * 4.0, btn_w, btn_h)),
            ],
            back_rect: TouchRect::new(btn_x, bottom_y, half_btn_w, bottom_h),
            toggle_mode_rect: TouchRect::new(btn_x + half_btn_w + 0.03, bottom_y, half_btn_w, bottom_h),
        }
    }

    /// Tests if a normalized point (px, py) touches any slot row or action button.
    pub fn hit_test(&self, px: f32, py: f32) -> Option<SaveLoadItem> {
        for (slot, rect) in &self.slot_rects {
            if rect.contains(px, py) {
                return Some(SaveLoadItem::Slot(*slot));
            }
        }
        if self.back_rect.contains(px, py) {
            return Some(SaveLoadItem::Back);
        }
        if self.toggle_mode_rect.contains(px, py) {
            return Some(SaveLoadItem::ToggleMode);
        }
        None
    }

    /// Checks if a normalized point (px, py) is strictly outside the modal card.
    pub fn is_outside_modal(&self, px: f32, py: f32) -> bool {
        !self.modal_rect.contains(px, py)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_state_defaults() {
        let state = MenuState::default();
        assert_eq!(state, MenuState::Hidden);
        assert!(!state.is_visible());
        assert_eq!(state.shader_index(), 0);

        let active = MenuState::MainMenu;
        assert!(active.is_visible());
        assert_eq!(active.shader_index(), 1);

        let save_mode = MenuState::SaveLoadSlotSelect { mode: SlotMode::Save };
        assert!(save_mode.is_visible());
        assert_eq!(save_mode.shader_index(), 2);

        let load_mode = MenuState::SaveLoadSlotSelect { mode: SlotMode::Load };
        assert!(load_mode.is_visible());
        assert_eq!(load_mode.shader_index(), 3);
    }

    #[test]
    fn test_menu_layout_hit_testing() {
        let layout = MenuLayout::new();

        // Hit Resume
        let resume_center = layout.item_rects[0].1.center();
        assert_eq!(layout.hit_test(resume_center.0, resume_center.1), Some(MenuItem::Resume));

        // Hit LoadRom
        let load_center = layout.item_rects[1].1.center();
        assert_eq!(layout.hit_test(load_center.0, load_center.1), Some(MenuItem::LoadRom));

        // Hit Outside
        assert!(layout.is_outside_modal(0.05, 0.05));
        assert!(layout.is_outside_modal(0.95, 0.95));
        assert_eq!(layout.hit_test(0.05, 0.05), None);
    }

    #[test]
    fn test_saveload_layout_hit_testing() {
        let layout = SaveLoadLayout::new();

        // Hit Slot 1
        let (s1_x, s1_y) = layout.slot_rects[0].1.center();
        assert_eq!(layout.hit_test(s1_x, s1_y), Some(SaveLoadItem::Slot(1)));

        // Hit Slot 5
        let (s5_x, s5_y) = layout.slot_rects[4].1.center();
        assert_eq!(layout.hit_test(s5_x, s5_y), Some(SaveLoadItem::Slot(5)));

        // Hit Back Button
        let (bx, by) = layout.back_rect.center();
        assert_eq!(layout.hit_test(bx, by), Some(SaveLoadItem::Back));

        // Hit Toggle Mode Button
        let (tx, ty) = layout.toggle_mode_rect.center();
        assert_eq!(layout.hit_test(tx, ty), Some(SaveLoadItem::ToggleMode));
    }

    #[test]
    fn test_slot_mode_toggle() {
        let mode = SlotMode::Save;
        assert_eq!(mode.toggle(), SlotMode::Load);
        assert_eq!(mode.toggle().toggle(), SlotMode::Save);
    }

    #[test]
    fn test_settings_layout_hit_testing() {
        let layout = SettingsLayout::new();

        // Hit Customize Controls
        let (c_x, c_y) = layout.item_rects[0].1.center();
        assert_eq!(layout.hit_test(c_x, c_y), Some(SettingsItem::CustomizeControls));

        // Hit Theme
        let (t_x, t_y) = layout.item_rects[3].1.center();
        assert_eq!(layout.hit_test(t_x, t_y), Some(SettingsItem::Theme));

        // Hit Back
        let (b_x, b_y) = layout.item_rects[5].1.center();
        assert_eq!(layout.hit_test(b_x, b_y), Some(SettingsItem::Back));
    }

    #[test]
    fn test_layout_editor_hit_testing() {
        let layout = LayoutEditorLayout::new();

        // Hit Save
        let (s_x, s_y) = layout.save_rect.center();
        assert_eq!(layout.hit_test(s_x, s_y), Some(LayoutEditorToolbarItem::Save));

        // Hit Reset
        let (r_x, r_y) = layout.reset_rect.center();
        assert_eq!(layout.hit_test(r_x, r_y), Some(LayoutEditorToolbarItem::ResetDefaults));

        // Hit Cancel
        let (c_x, c_y) = layout.cancel_rect.center();
        assert_eq!(layout.hit_test(c_x, c_y), Some(LayoutEditorToolbarItem::Cancel));
    }

    #[test]
    fn test_fast_forward_layout_hit_testing() {
        let layout = FastForwardLayout::new();

        // Hit Normal 1X
        let (x1, y1) = layout.item_rects[0].1.center();
        assert_eq!(layout.hit_test(x1, y1), Some(FastForwardItem::Normal));

        // Hit 2X
        let (x2, y2) = layout.item_rects[1].1.center();
        assert_eq!(layout.hit_test(x2, y2), Some(FastForwardItem::Speed2x));

        // Hit 4X
        let (x4, y4) = layout.item_rects[2].1.center();
        assert_eq!(layout.hit_test(x4, y4), Some(FastForwardItem::Speed4x));

        // Hit 8X
        let (x8, y8) = layout.item_rects[3].1.center();
        assert_eq!(layout.hit_test(x8, y8), Some(FastForwardItem::Speed8x));

        // Hit Uncapped Max
        let (xm, ym) = layout.item_rects[4].1.center();
        assert_eq!(layout.hit_test(xm, ym), Some(FastForwardItem::Uncapped));

        // Hit Back
        let (xb, yb) = layout.item_rects[5].1.center();
        assert_eq!(layout.hit_test(xb, yb), Some(FastForwardItem::Back));
    }

    #[test]
    fn test_settings_slider_calculations() {
        let layout = SettingsLayout::new();
        let op_y = layout.item_rects[1].1.center().1;

        assert!(layout.is_opacity_slider(0.50, op_y));
        assert!(!layout.is_opacity_slider(0.30, op_y));

        let op_min = SettingsLayout::calculate_opacity(0.44);
        assert!((op_min - 0.15).abs() < 1e-4);

        let op_max = SettingsLayout::calculate_opacity(0.72);
        assert!((op_max - 1.00).abs() < 1e-4);

        let sc_y = layout.item_rects[2].1.center().1;
        assert!(layout.is_scale_slider(0.50, sc_y));
        assert!(!layout.is_scale_slider(0.30, sc_y));

        let sc_min = SettingsLayout::calculate_scale(0.44);
        assert!((sc_min - 0.60).abs() < 1e-4);

        let sc_max = SettingsLayout::calculate_scale(0.72);
        assert!((sc_max - 1.50).abs() < 1e-4);
    }
}
