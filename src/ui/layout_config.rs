//! Touch Layout & UI Preference Configuration Engine for PixelDrive.
//!
//! Manages serialization, disk persistence, layout customizer presets,
//! on-screen coordinate clamping, UI themes, and fast-forward speed preferences.

use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use log::{info, warn};

use crate::input::{TouchInputManager, VirtualButtonId};

/// Supported UI & on-screen overlay color themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum UiTheme {
    #[default]
    DarkSlate = 0,
    AmoledBlack = 1,
    ClassicGray = 2,
}

impl UiTheme {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => UiTheme::AmoledBlack,
            2 => UiTheme::ClassicGray,
            _ => UiTheme::DarkSlate,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn cycle(&self) -> Self {
        match self {
            UiTheme::DarkSlate => UiTheme::AmoledBlack,
            UiTheme::AmoledBlack => UiTheme::ClassicGray,
            UiTheme::ClassicGray => UiTheme::DarkSlate,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            UiTheme::DarkSlate => "DARK SLATE",
            UiTheme::AmoledBlack => "AMOLED BLACK",
            UiTheme::ClassicGray => "CLASSIC DMG",
        }
    }
}

/// Fast-forward speed multiplier setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum FastForwardSpeed {
    Normal = 1,
    #[default]
    Speed2x = 2,
    Speed4x = 4,
    Speed8x = 8,
    Uncapped = 0,
}

impl FastForwardSpeed {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => FastForwardSpeed::Normal,
            4 => FastForwardSpeed::Speed4x,
            8 => FastForwardSpeed::Speed8x,
            0 => FastForwardSpeed::Uncapped,
            _ => FastForwardSpeed::Speed2x,
        }
    }

    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn to_index(&self) -> u8 {
        match self {
            FastForwardSpeed::Normal => 0,
            FastForwardSpeed::Speed2x => 1,
            FastForwardSpeed::Speed4x => 2,
            FastForwardSpeed::Speed8x => 3,
            FastForwardSpeed::Uncapped => 4,
        }
    }

    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => FastForwardSpeed::Normal,
            1 => FastForwardSpeed::Speed2x,
            2 => FastForwardSpeed::Speed4x,
            3 => FastForwardSpeed::Speed8x,
            4 => FastForwardSpeed::Uncapped,
            _ => FastForwardSpeed::Speed2x,
        }
    }

    pub fn cycle(&self) -> Self {
        match self {
            FastForwardSpeed::Normal => FastForwardSpeed::Speed2x,
            FastForwardSpeed::Speed2x => FastForwardSpeed::Speed4x,
            FastForwardSpeed::Speed4x => FastForwardSpeed::Speed8x,
            FastForwardSpeed::Speed8x => FastForwardSpeed::Uncapped,
            FastForwardSpeed::Uncapped => FastForwardSpeed::Normal,
        }
    }

    pub fn steps_per_frame(&self) -> usize {
        match self {
            FastForwardSpeed::Normal => 1,
            FastForwardSpeed::Speed2x => 2,
            FastForwardSpeed::Speed4x => 4,
            FastForwardSpeed::Speed8x => 8,
            FastForwardSpeed::Uncapped => 10,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FastForwardSpeed::Normal => "1X (NORMAL)",
            FastForwardSpeed::Speed2x => "2X SPEED",
            FastForwardSpeed::Speed4x => "4X SPEED",
            FastForwardSpeed::Speed8x => "8X SPEED",
            FastForwardSpeed::Uncapped => "MAX SPEED",
        }
    }
}

/// Complete touch layout and UI preference configuration model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchLayoutConfig {
    pub scale: f32,
    pub opacity: f32,
    pub dpad_pos: (f32, f32),
    pub dpad_radius: f32,
    pub btn_a_pos: (f32, f32),
    pub btn_b_pos: (f32, f32),
    pub btn_l_pos: (f32, f32),
    pub btn_r_pos: (f32, f32),
    pub btn_select_pos: (f32, f32),
    pub btn_start_pos: (f32, f32),
    pub theme_index: u8,
    pub fast_forward_speed: u8,
    #[serde(default = "default_true")]
    pub haptics_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for TouchLayoutConfig {
    fn default() -> Self {
        Self {
            scale: 1.0,
            opacity: 0.65,
            dpad_pos: (0.14, 0.76),
            dpad_radius: 0.11,
            btn_a_pos: (0.90, 0.70),
            btn_b_pos: (0.78, 0.80),
            btn_l_pos: (0.11, 0.075),
            btn_r_pos: (0.89, 0.075),
            btn_select_pos: (0.425, 0.925),
            btn_start_pos: (0.575, 0.925),
            theme_index: UiTheme::DarkSlate.as_u8(),
            fast_forward_speed: FastForwardSpeed::Speed2x.as_u8(),
            haptics_enabled: true,
        }
    }
}

impl TouchLayoutConfig {
    /// Clamps a normalized coordinate `(x, y)` to stay within safe viewport bounds `[0.05, 0.95]`.
    pub fn clamp_pos(pos: (f32, f32)) -> (f32, f32) {
        (
            pos.0.clamp(0.05, 0.95),
            pos.1.clamp(0.05, 0.95),
        )
    }

    /// Cycles the overlay button opacity through predefined stepped levels (20%, 40%, 60%, 80%, 100%).
    pub fn cycle_opacity(&mut self) {
        if self.opacity < 0.30 {
            self.opacity = 0.40;
        } else if self.opacity < 0.50 {
            self.opacity = 0.60;
        } else if self.opacity < 0.70 {
            self.opacity = 0.80;
        } else if self.opacity < 0.90 {
            self.opacity = 1.00;
        } else {
            self.opacity = 0.20;
        }
    }

    /// Display string for current opacity level.
    pub fn opacity_label(&self) -> &'static str {
        if self.opacity < 0.30 {
            "20%"
        } else if self.opacity < 0.50 {
            "40%"
        } else if self.opacity < 0.70 {
            "60%"
        } else if self.opacity < 0.90 {
            "80%"
        } else {
            "100%"
        }
    }

    /// Cycles the button scale between 75%, 100%, 125%, and 150%.
    pub fn cycle_scale(&mut self) {
        if self.scale < 0.85 {
            self.scale = 1.00;
        } else if self.scale < 1.10 {
            self.scale = 1.25;
        } else if self.scale < 1.35 {
            self.scale = 1.50;
        } else {
            self.scale = 0.75;
        }
    }

    /// Display string for current button scale level.
    pub fn scale_label(&self) -> &'static str {
        if self.scale < 0.85 {
            "75%"
        } else if self.scale < 1.10 {
            "100%"
        } else if self.scale < 1.35 {
            "125%"
        } else {
            "150%"
        }
    }

    /// Cycles the active UI theme.
    pub fn cycle_theme(&mut self) {
        let theme = UiTheme::from_u8(self.theme_index);
        self.theme_index = theme.cycle().as_u8();
    }

    /// Active UI theme helper.
    pub fn theme(&self) -> UiTheme {
        UiTheme::from_u8(self.theme_index)
    }

    /// Cycles fast-forward speed setting.
    pub fn cycle_fast_forward(&mut self) {
        let speed = FastForwardSpeed::from_u8(self.fast_forward_speed);
        self.fast_forward_speed = speed.cycle().as_u8();
    }

    /// Active fast-forward speed helper.
    pub fn fast_forward(&self) -> FastForwardSpeed {
        FastForwardSpeed::from_u8(self.fast_forward_speed)
    }

    /// Toggles tactile haptic feedback on or off.
    pub fn toggle_haptics(&mut self) {
        self.haptics_enabled = !self.haptics_enabled;
    }

    /// Display string for current haptic feedback state.
    pub fn haptics_label(&self) -> &'static str {
        if self.haptics_enabled {
            "ON"
        } else {
            "OFF"
        }
    }

    /// Applies configuration values to a `TouchInputManager` instance.
    pub fn apply_to_overlay(&self, overlay: &mut TouchInputManager) {
        overlay.scale = self.scale;
        overlay.opacity = self.opacity;
        overlay.set_haptics_enabled(self.haptics_enabled);

        // Apply D-Pad
        let dpad_pos = Self::clamp_pos(self.dpad_pos);
        overlay.dpad.center = dpad_pos;
        overlay.dpad.radius = self.dpad_radius * self.scale;

        // Apply Face buttons
        let a_pos = Self::clamp_pos(self.btn_a_pos);
        let b_pos = Self::clamp_pos(self.btn_b_pos);
        let a_radius = 0.055 * self.scale;
        let b_radius = 0.055 * self.scale;
        overlay.btn_a = crate::input::VirtualButton::new_circle(
            VirtualButtonId::A,
            a_pos.0,
            a_pos.1,
            a_radius,
        );
        overlay.btn_b = crate::input::VirtualButton::new_circle(
            VirtualButtonId::B,
            b_pos.0,
            b_pos.1,
            b_radius,
        );

        // Apply Shoulders
        let l_pos = Self::clamp_pos(self.btn_l_pos);
        let r_pos = Self::clamp_pos(self.btn_r_pos);
        overlay.btn_l = crate::input::VirtualButton::new_pill(
            VirtualButtonId::L,
            l_pos.0 - 0.08 * self.scale,
            l_pos.1 - 0.035 * self.scale,
            0.16 * self.scale,
            0.07 * self.scale,
            0.035 * self.scale,
        );
        overlay.btn_r = crate::input::VirtualButton::new_pill(
            VirtualButtonId::R,
            r_pos.0 - 0.08 * self.scale,
            r_pos.1 - 0.035 * self.scale,
            0.16 * self.scale,
            0.07 * self.scale,
            0.035 * self.scale,
        );

        // Apply Start / Select
        let select_pos = Self::clamp_pos(self.btn_select_pos);
        let start_pos = Self::clamp_pos(self.btn_start_pos);
        overlay.btn_select = crate::input::VirtualButton::new_pill(
            VirtualButtonId::Select,
            select_pos.0 - 0.06 * self.scale,
            select_pos.1 - 0.025 * self.scale,
            0.12 * self.scale,
            0.05 * self.scale,
            0.025 * self.scale,
        );
        overlay.btn_start = crate::input::VirtualButton::new_pill(
            VirtualButtonId::Start,
            start_pos.0 - 0.06 * self.scale,
            start_pos.1 - 0.025 * self.scale,
            0.12 * self.scale,
            0.05 * self.scale,
            0.025 * self.scale,
        );

        // Update chord bridge
        overlay.chord_ab = crate::input::ChordHitbox::new(
            a_pos,
            b_pos,
            0.040 * self.scale,
        );
    }

    /// Creates a `TouchLayoutConfig` snapshot from current `TouchInputManager` state.
    pub fn from_overlay(overlay: &TouchInputManager, theme_index: u8, fast_forward_speed: u8) -> Self {
        Self {
            scale: overlay.scale,
            opacity: overlay.opacity,
            dpad_pos: overlay.dpad.center,
            dpad_radius: overlay.dpad.radius / overlay.scale.max(0.1),
            btn_a_pos: overlay.btn_a.center(),
            btn_b_pos: overlay.btn_b.center(),
            btn_l_pos: overlay.btn_l.center(),
            btn_r_pos: overlay.btn_r.center(),
            btn_select_pos: overlay.btn_select.center(),
            btn_start_pos: overlay.btn_start.center(),
            theme_index,
            fast_forward_speed,
            haptics_enabled: overlay.is_haptics_enabled(),
        }
    }

    /// Serializes and writes configuration to disk JSON file.
    pub fn save_to_file(&self, file_path: &Path) -> std::io::Result<()> {
        if let Some(parent) = file_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json_str = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(file_path, json_str.as_bytes())?;
        info!("Touch layout configuration successfully saved to {:?}", file_path);
        Ok(())
    }

    /// Reads and deserializes configuration from disk JSON file with fallback to defaults.
    pub fn load_from_file(file_path: &Path) -> Self {
        if !file_path.exists() {
            info!("Touch layout config file {:?} does not exist; using defaults", file_path);
            return Self::default();
        }
        match fs::read_to_string(file_path) {
            Ok(json_str) => match serde_json::from_str::<Self>(&json_str) {
                Ok(config) => {
                    info!("Successfully loaded touch layout configuration from {:?}", file_path);
                    config
                }
                Err(err) => {
                    warn!("Failed to parse layout config JSON {:?}: {}; using defaults", file_path, err);
                    Self::default()
                }
            },
            Err(err) => {
                warn!("Failed to read layout config file {:?}: {}; using defaults", file_path, err);
                Self::default()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_layout_config_default_and_serialization() {
        let config = TouchLayoutConfig::default();
        assert_eq!(config.scale, 1.0);
        assert_eq!(config.opacity, 0.65);
        assert_eq!(config.theme_index, 0);
        assert_eq!(config.fast_forward_speed, 2);
        assert!(config.haptics_enabled);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: TouchLayoutConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_touch_layout_config_cycles() {
        let mut config = TouchLayoutConfig::default();

        // Haptics toggle
        assert!(config.haptics_enabled);
        assert_eq!(config.haptics_label(), "ON");
        config.toggle_haptics();
        assert!(!config.haptics_enabled);
        assert_eq!(config.haptics_label(), "OFF");
        config.toggle_haptics();
        assert!(config.haptics_enabled);

        // Opacity cycling
        config.opacity = 0.20;
        config.cycle_opacity();
        assert_eq!(config.opacity_label(), "40%");
        config.cycle_opacity();
        assert_eq!(config.opacity_label(), "60%");
        config.cycle_opacity();
        assert_eq!(config.opacity_label(), "80%");
        config.cycle_opacity();
        assert_eq!(config.opacity_label(), "100%");
        config.cycle_opacity();
        assert_eq!(config.opacity_label(), "20%");

        // Scale cycling
        config.scale = 0.75;
        config.cycle_scale();
        assert_eq!(config.scale_label(), "100%");
        config.cycle_scale();
        assert_eq!(config.scale_label(), "125%");
        config.cycle_scale();
        assert_eq!(config.scale_label(), "150%");
        config.cycle_scale();
        assert_eq!(config.scale_label(), "75%");

        // Theme cycling
        config.theme_index = 0;
        config.cycle_theme();
        assert_eq!(config.theme(), UiTheme::AmoledBlack);
        assert_eq!(config.theme().label(), "AMOLED BLACK");
        config.cycle_theme();
        assert_eq!(config.theme(), UiTheme::ClassicGray);
        assert_eq!(config.theme().label(), "CLASSIC DMG");
        config.cycle_theme();
        assert_eq!(config.theme(), UiTheme::DarkSlate);

        // Fast-Forward cycling
        config.fast_forward_speed = 1;
        config.cycle_fast_forward();
        assert_eq!(config.fast_forward(), FastForwardSpeed::Speed2x);
        assert_eq!(config.fast_forward().steps_per_frame(), 2);
        config.cycle_fast_forward();
        assert_eq!(config.fast_forward(), FastForwardSpeed::Speed4x);
        assert_eq!(config.fast_forward().steps_per_frame(), 4);
        config.cycle_fast_forward();
        assert_eq!(config.fast_forward(), FastForwardSpeed::Speed8x);
        assert_eq!(config.fast_forward().steps_per_frame(), 8);
        config.cycle_fast_forward();
        assert_eq!(config.fast_forward(), FastForwardSpeed::Uncapped);
        assert_eq!(config.fast_forward().steps_per_frame(), 10);
        config.cycle_fast_forward();
        assert_eq!(config.fast_forward(), FastForwardSpeed::Normal);
        assert_eq!(config.fast_forward().steps_per_frame(), 1);
    }

    #[test]
    fn test_coordinate_clamping() {
        let clamped = TouchLayoutConfig::clamp_pos((-0.5, 1.5));
        assert_eq!(clamped, (0.05, 0.95));

        let valid = TouchLayoutConfig::clamp_pos((0.5, 0.5));
        assert_eq!(valid, (0.5, 0.5));
    }

    #[test]
    fn test_file_persistence_roundtrip() {
        let temp_dir = std::env::temp_dir().join("pixeldrive_config_test");
        let file_path = temp_dir.join("test_touch_layout.json");

        let mut config = TouchLayoutConfig::default();
        config.scale = 1.25;
        config.opacity = 0.80;
        config.theme_index = 1;
        config.dpad_pos = (0.20, 0.70);

        config.save_to_file(&file_path).unwrap();
        let loaded = TouchLayoutConfig::load_from_file(&file_path);

        assert_eq!(config, loaded);
        let _ = fs::remove_dir_all(temp_dir);
    }
}
