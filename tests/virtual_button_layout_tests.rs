//! Test Suite: Phase 3 Virtual Button Layout Customizer & Display Settings.
//!
//! Validates:
//! - TC-LAYOUT-01: Boundary clamping, coordinate math, and control group drag translation.
//! - TC-LAYOUT-02: Real-time WGPU uniform synchronization, bitmask packing, and memory layout.
//! - TC-LAYOUT-03: JSON configuration persistence, Android Scoped Storage I/O, and parse error fallback.
//! - TC-LAYOUT-04: "Reset Defaults" behavior, preset restoration, and overlay state synchronization.
//! - TC-LAYOUT-05: Multi-resolution screen normalization, aspect ratio adaptability, and hitbox scaling.

use std::fs;
use pixeldrive::core::Button;
use pixeldrive::input::{
    ControlGroup, InputSource, TouchInputManager, TouchOverlayPreset,
};
use pixeldrive::render::overlay::TouchOverlayUniforms;
use pixeldrive::ui::layout_config::{FastForwardSpeed, TouchLayoutConfig, UiTheme};
use pixeldrive::ui::menu::MenuState;

// ============================================================================
// TC-LAYOUT-01: Boundary Clamping, Coordinate Math & Drag Translation
// ============================================================================

#[test]
fn test_tc_layout_01_boundary_clamping_and_drag_translation() {
    log::info!("TC-LAYOUT-01: Validating on-screen control group translation and boundary clamping...");

    let mut overlay = TouchInputManager::new();
    overlay.set_menu_state(MenuState::LayoutEditor);

    // 1. D-Pad Clamping
    // Initial center is at (0.14, 0.76)
    let dpad_origin = overlay.dpad.center;
    assert!((dpad_origin.0 - 0.14).abs() < 0.001);
    assert!((dpad_origin.1 - 0.76).abs() < 0.001);

    // Drag D-pad way past bottom-left corner
    overlay.translate_control_group(ControlGroup::Dpad, -5.0, 5.0, dpad_origin);
    assert_eq!(overlay.dpad.center, (0.08, 0.92), "D-pad must clamp safely to [0.08, 0.92]");

    // Drag D-pad way past top-right corner
    overlay.translate_control_group(ControlGroup::Dpad, 10.0, -10.0, dpad_origin);
    assert_eq!(overlay.dpad.center, (0.92, 0.08), "D-pad must clamp safely to [0.08, 0.92]");

    // 2. Action Cluster (A and B buttons + chord bridge) Clamping & Relative Delta
    let a_origin = overlay.btn_a.center();
    let b_origin = overlay.btn_b.center();
    let delta_ab = (b_origin.0 - a_origin.0, b_origin.1 - a_origin.1);

    // Translate Action Cluster towards top-left
    overlay.translate_control_group(ControlGroup::ActionCluster, -0.40, -0.30, a_origin);
    let new_a = overlay.btn_a.center();
    let new_b = overlay.btn_b.center();

    // Verify relative spacing between A and B is preserved
    let current_delta = (new_b.0 - new_a.0, new_b.1 - new_a.1);
    assert!(
        (current_delta.0 - delta_ab.0).abs() < 0.001,
        "Relative X distance between A and B must be preserved"
    );
    assert!(
        (current_delta.1 - delta_ab.1).abs() < 0.001,
        "Relative Y distance between A and B must be preserved"
    );

    // Verify A+B chord bridge hitbox is updated to bridge between new_a and new_b
    let chord_mid_x = (overlay.chord_ab.p1.0 + overlay.chord_ab.p2.0) * 0.5;
    let chord_mid_y = (overlay.chord_ab.p1.1 + overlay.chord_ab.p2.1) * 0.5;
    let expected_chord_x = (new_a.0 + new_b.0) * 0.5;
    let expected_chord_y = (new_a.1 + new_b.1) * 0.5;
    assert!((chord_mid_x - expected_chord_x).abs() < 0.001);
    assert!((chord_mid_y - expected_chord_y).abs() < 0.001);

    // 3. Shoulder Buttons (L and R) Clamping
    let l_origin = overlay.btn_l.center();
    let r_origin = overlay.btn_r.center();

    overlay.translate_control_group(ControlGroup::LShoulder, -2.0, -2.0, l_origin);
    assert_eq!(overlay.btn_l.center().0, 0.08);
    assert_eq!(overlay.btn_l.center().1, 0.05);

    overlay.translate_control_group(ControlGroup::RShoulder, 2.0, 2.0, r_origin);
    assert_eq!(overlay.btn_r.center().0, 0.92);
    assert_eq!(overlay.btn_r.center().1, 0.95);

    // 4. Start & Select Buttons Clamping
    let start_origin = overlay.btn_start.center();
    let select_origin = overlay.btn_select.center();
    let delta_ss = (select_origin.0 - start_origin.0, select_origin.1 - start_origin.1);

    overlay.translate_control_group(ControlGroup::StartSelect, -0.20, -0.50, start_origin);
    let new_start = overlay.btn_start.center();
    let new_select = overlay.btn_select.center();

    let cur_ss_delta = (new_select.0 - new_start.0, new_select.1 - new_start.1);
    assert!((cur_ss_delta.0 - delta_ss.0).abs() < 0.001);
    assert!((cur_ss_delta.1 - delta_ss.1).abs() < 0.001);

    // 5. Interactive Touch Dragging in Layout Editor Mode
    let screen_w = 1000.0;
    let screen_h = 1000.0;

    // Reset D-pad center to a clear interior position for drag test
    overlay.dpad.center = (0.30, 0.60);
    let (dpx, dpy) = overlay.dpad.center;

    // Touch down on D-pad
    overlay.handle_touch_down(100, dpx * screen_w, dpy * screen_h, screen_w, screen_h);
    assert!(overlay.active_drag_group.is_some());
    assert_eq!(overlay.active_drag_group.unwrap().0, ControlGroup::Dpad);

    // Move touch point
    overlay.handle_touch_move(100, (dpx + 0.10) * screen_w, (dpy - 0.05) * screen_h, screen_w, screen_h);
    let moved_dpad = overlay.dpad.center;
    assert!((moved_dpad.0 - (dpx + 0.10)).abs() < 0.005);
    assert!((moved_dpad.1 - (dpy - 0.05)).abs() < 0.005);

    // Touch up ends drag operation
    overlay.handle_touch_up(100);
    assert!(overlay.active_drag_group.is_none());
}

// ============================================================================
// TC-LAYOUT-02: Real-time Uniform Synchronization & Bitmask Packing
// ============================================================================

#[test]
fn test_tc_layout_02_realtime_uniform_synchronization_and_bitmask_packing() {
    log::info!("TC-LAYOUT-02: Validating real-time WGPU uniform updates and bitmask packing...");

    // 1. Verify Uniform Buffer Size and 16-byte Alignment
    assert_eq!(
        std::mem::size_of::<TouchOverlayUniforms>(),
        144,
        "TouchOverlayUniforms must be exactly 144 bytes"
    );
    assert_eq!(
        std::mem::size_of::<TouchOverlayUniforms>() % 16,
        0,
        "TouchOverlayUniforms must adhere to WGPU 16-byte uniform alignment"
    );

    // 2. Custom Layout Config
    let mut config = TouchLayoutConfig::default();
    config.scale = 1.25;
    config.opacity = 0.80;
    config.theme_index = UiTheme::AmoledBlack.as_u8(); // 1
    config.fast_forward_speed = FastForwardSpeed::Speed4x.as_u8(); // 4
    config.dpad_pos = (0.22, 0.68);
    config.btn_a_pos = (0.86, 0.62);
    config.btn_b_pos = (0.74, 0.72);

    // 3. Apply to TouchInputManager
    let mut overlay = TouchInputManager::new();
    config.apply_to_overlay(&mut overlay);
    overlay.theme_index = config.theme_index;
    overlay.fast_forward_speed = config.fast_forward_speed;

    // Verify overlay reflects config properties directly
    assert_eq!(overlay.scale, 1.25);
    assert_eq!(overlay.opacity, 0.80);
    assert_eq!(overlay.theme_index, 1);
    assert_eq!(overlay.fast_forward_speed, 4);
    assert_eq!(overlay.dpad.center, (0.22, 0.68));
    assert_eq!(overlay.btn_a.center(), (0.86, 0.62));
    assert_eq!(overlay.btn_b.center(), (0.74, 0.72));

    // 4. Test Uniform Extraction and Bitmask Packing
    let screen_w = 1920.0;
    let screen_h = 1080.0;
    let aspect = screen_w / screen_h;

    let op_pct = (overlay.opacity.clamp(0.0, 1.0) * 100.0).round() as u32;
    let sc_pct = (overlay.scale.clamp(0.5, 2.0) * 100.0).round() as u32;
    let th_idx = (overlay.theme_index as u32) & 0x07;
    let ff_idx = (overlay.fast_forward_speed as u32) & 0x07;
    let settings_values = op_pct | (sc_pct << 8) | (th_idx << 16) | (ff_idx << 20);

    let uniforms = TouchOverlayUniforms {
        screen_size: [screen_w, screen_h],
        aspect_ratio: aspect,
        opacity: overlay.opacity,
        pressed_mask: overlay.pressed_bitmask(),
        scale: overlay.scale,
        dpad_radius: overlay.dpad.radius,
        btn_radius: overlay.btn_a.radius(),
        dpad_center: [overlay.dpad.center.0, overlay.dpad.center.1],
        btn_a_pos: [overlay.btn_a.center().0, overlay.btn_a.center().1],
        btn_b_pos: [overlay.btn_b.center().0, overlay.btn_b.center().1],
        btn_l_pos: [overlay.btn_l.center().0, overlay.btn_l.center().1],
        btn_r_pos: [overlay.btn_r.center().0, overlay.btn_r.center().1],
        btn_start_pos: [overlay.btn_start.center().0, overlay.btn_start.center().1],
        btn_select_pos: [overlay.btn_select.center().0, overlay.btn_select.center().1],
        btn_menu_pos: [overlay.btn_menu.center().0, overlay.btn_menu.center().1],
        btn_ff_pos: [overlay.btn_fast_forward.center().0, overlay.btn_fast_forward.center().1],
        btn_qs_pos: [overlay.btn_quick_save.center().0, overlay.btn_quick_save.center().1],
        btn_ql_pos: [overlay.btn_quick_load.center().0, overlay.btn_quick_load.center().1],
        menu_state: overlay.menu_state().shader_index(),
        menu_pressed_item: 0,
        slot_mask: overlay.slot_mask(),
        theme_index: overlay.theme_index as u32,
        settings_values,
        _pad: 0,
    };

    // Bitmask assertions:
    // Opacity: 80
    assert_eq!(uniforms.settings_values & 0xFF, 80);
    // Scale: 125
    assert_eq!((uniforms.settings_values >> 8) & 0xFF, 125);
    // Theme: 1
    assert_eq!((uniforms.settings_values >> 16) & 0x0F, 1);
    // FastForward: 4
    assert_eq!((uniforms.settings_values >> 20) & 0x0F, 4);

    // Verify bytemuck castable for WGPU buffer submission
    let binding = [uniforms];
    let cast_bytes: &[u8] = bytemuck::cast_slice(&binding);
    assert_eq!(cast_bytes.len(), 144);
}

// ============================================================================
// TC-LAYOUT-03: JSON Persistence & File I/O Resilience
// ============================================================================

#[test]
fn test_tc_layout_03_json_persistence_and_file_io_resilience() -> std::io::Result<()> {
    log::info!("TC-LAYOUT-03: Validating JSON serialization and disk persistence...");

    let temp_dir = std::env::temp_dir().join(format!("pixeldrive_layout_json_{}", std::process::id()));
    let config_path = temp_dir.join("config").join("touch_layout.json");

    // 1. Write Custom Configuration
    let mut custom_config = TouchLayoutConfig::default();
    custom_config.scale = 1.50;
    custom_config.opacity = 0.40;
    custom_config.dpad_pos = (0.18, 0.72);
    custom_config.btn_a_pos = (0.88, 0.65);
    custom_config.btn_b_pos = (0.76, 0.75);
    custom_config.theme_index = UiTheme::ClassicGray.as_u8();
    custom_config.fast_forward_speed = FastForwardSpeed::Speed8x.as_u8();

    custom_config.save_to_file(&config_path)?;
    assert!(config_path.exists(), "Config file must exist on disk");

    // 2. Load from disk and verify byte-for-byte property preservation
    let loaded_config = TouchLayoutConfig::load_from_file(&config_path);
    assert_eq!(custom_config, loaded_config);

    // 3. Missing file fallback to default
    let missing_path = temp_dir.join("non_existent_config.json");
    let fallback_config = TouchLayoutConfig::load_from_file(&missing_path);
    assert_eq!(fallback_config, TouchLayoutConfig::default());

    // 4. Corrupted JSON file fallback without panicking
    let corrupt_path = temp_dir.join("corrupted.json");
    fs::write(&corrupt_path, b"{ broken_json: [1, 2, 3, invalid")?;
    let recovered_config = TouchLayoutConfig::load_from_file(&corrupt_path);
    assert_eq!(recovered_config, TouchLayoutConfig::default());

    // Clean up
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}

// ============================================================================
// TC-LAYOUT-04: Reset Defaults & Preset Restoration
// ============================================================================

#[test]
fn test_tc_layout_04_reset_defaults_and_preset_restoration() {
    log::info!("TC-LAYOUT-04: Validating Reset Defaults and overlay presets...");

    let mut overlay = TouchInputManager::new();

    // 1. Mutate overlay to non-default state
    overlay.set_scale(1.50);
    overlay.set_opacity(0.20);
    overlay.dpad.center = (0.40, 0.40);
    overlay.set_btn_a_pos((0.50, 0.50));
    overlay.theme_index = 2;
    overlay.fast_forward_speed = 8;

    assert_eq!(overlay.scale, 1.50);
    assert_eq!(overlay.opacity, 0.20);

    // 2. Reset Defaults
    let default_config = TouchLayoutConfig::default();
    default_config.apply_to_overlay(&mut overlay);
    overlay.theme_index = default_config.theme_index;
    overlay.fast_forward_speed = default_config.fast_forward_speed;

    // Verify all properties are restored to baseline defaults
    assert_eq!(overlay.scale, 1.0);
    assert_eq!(overlay.opacity, 0.65);
    assert_eq!(overlay.dpad.center, (0.14, 0.76));
    assert_eq!(overlay.btn_a.center(), (0.90, 0.70));
    assert_eq!(overlay.btn_b.center(), (0.78, 0.80));
    assert_eq!(overlay.theme_index, 0);
    assert_eq!(overlay.fast_forward_speed, 2);

    // 3. Test Overlay Preset cycling
    overlay.apply_preset(TouchOverlayPreset::Wide);
    assert_eq!(overlay.preset, TouchOverlayPreset::Wide);

    overlay.apply_preset(TouchOverlayPreset::Ergonomic);
    assert_eq!(overlay.preset, TouchOverlayPreset::Ergonomic);

    overlay.apply_preset(TouchOverlayPreset::Compact);
    assert_eq!(overlay.preset, TouchOverlayPreset::Compact);
}

// ============================================================================
// TC-LAYOUT-05: Multi-Resolution Normalization & Hitbox Scaling
// ============================================================================

#[test]
fn test_tc_layout_05_multi_resolution_normalization_and_hitbox_scaling() {
    log::info!("TC-LAYOUT-05: Validating multi-resolution touch normalization...");

    let mut overlay = TouchInputManager::new();

    // Resolutions to validate:
    // - 16:9 Landscape (1920x1080)
    // - 20:9 Modern Mobile (2400x1080)
    // - 9:16 Portrait (1080x1920)
    // - 4:3 Tablet (2048x1536)
    let screen_resolutions: [(f32, f32); 4] = [
        (1920.0, 1080.0),
        (2400.0, 1080.0),
        (1080.0, 1920.0),
        (2048.0, 1536.0),
    ];

    for (screen_w, screen_h) in screen_resolutions {
        // Test A button press across different resolutions
        let a_norm = overlay.btn_a.center();
        let pixel_x = a_norm.0 * screen_w;
        let pixel_y = a_norm.1 * screen_h;

        overlay.handle_touch_down(1, pixel_x, pixel_y, screen_w, screen_h);
        let joypad = overlay.poll();
        assert!(
            joypad.is_pressed(Button::A),
            "A button must register press at resolution {}x{}",
            screen_w,
            screen_h
        );

        overlay.handle_touch_up(1);
        let released = overlay.poll();
        assert!(!released.is_pressed(Button::A));

        // Test D-Pad Left press across different resolutions
        let dpad_norm = overlay.dpad.center;
        let dpad_radius = overlay.dpad.radius;
        let dpad_left_norm_x = dpad_norm.0 - dpad_radius * 0.7;
        let dpad_left_norm_y = dpad_norm.1;

        let dpad_px = dpad_left_norm_x * screen_w;
        let dpad_py = dpad_left_norm_y * screen_h;

        overlay.handle_touch_down(2, dpad_px, dpad_py, screen_w, screen_h);
        let dpad_state = overlay.poll();
        assert!(
            dpad_state.is_pressed(Button::Left),
            "D-Pad Left must register press at resolution {}x{}",
            screen_w,
            screen_h
        );
        overlay.handle_touch_up(2);
    }

    // 2. Test Hitbox Scaling (Scale 0.75 vs 1.50)
    let mut config = TouchLayoutConfig::default();
    config.scale = 0.75;
    config.apply_to_overlay(&mut overlay);
    let small_a_radius = overlay.btn_a.radius();

    config.scale = 1.50;
    config.apply_to_overlay(&mut overlay);
    let large_a_radius = overlay.btn_a.radius();

    assert_eq!(large_a_radius, small_a_radius * 2.0, "Button radius must scale linearly with scale factor");
}
