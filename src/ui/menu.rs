//! In-Game Modal Pause Menu Overlay for PixelDrive.
//!
//! Provides state management, item hierarchy, normalized hitboxes, and action dispatching
//! for the in-game modal pause menu on mobile and touch devices.

use crate::input::TouchRect;

/// Current display state of the in-game pause menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MenuState {
    #[default]
    Hidden,
    MainMenu,
    SaveSlots,
    Settings,
    Cheats,
}

impl MenuState {
    /// Returns true if the modal menu is active and visible.
    pub fn is_visible(&self) -> bool {
        *self != MenuState::Hidden
    }
}

/// Selectable items within the in-game pause menu.
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
            MenuItem::SaveLoadStates => Some("Phase 2"),
            MenuItem::Settings => Some("Phase 3"),
            MenuItem::Cheats => Some("Phase 4"),
            _ => None,
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
}

/// Normalized layout geometry for the in-game modal pause menu.
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
        // Modal outer bounds centered on the screen [0.0 .. 1.0]
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

    /// Checks if a normalized point (px, py) is strictly outside the modal card (for tap-to-dismiss).
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

        let active = MenuState::MainMenu;
        assert!(active.is_visible());
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
    fn test_menu_item_indices_and_labels() {
        assert_eq!(MenuItem::Resume.shader_index(), 1);
        assert_eq!(MenuItem::LoadRom.shader_index(), 2);
        assert_eq!(MenuItem::SaveLoadStates.shader_index(), 3);
        assert_eq!(MenuItem::ResetGame.shader_index(), 4);
        assert_eq!(MenuItem::Settings.shader_index(), 5);
        assert_eq!(MenuItem::Cheats.shader_index(), 6);

        assert_eq!(MenuItem::Resume.label(), "Resume Game");
        assert_eq!(MenuItem::SaveLoadStates.subtitle(), Some("Phase 2"));
    }
}
