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
            MenuState::Cheats => 5,
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
            MenuItem::Cheats => "Cheat Codes",
        }
    }

    /// Subtitle or phase status indicator.
    pub fn subtitle(&self) -> Option<&'static str> {
        match self {
            MenuItem::Settings => Some("Phase 3"),
            MenuItem::Cheats => Some("Phase 4"),
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

/// Actions dispatched by menu interaction to the main runtime loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
