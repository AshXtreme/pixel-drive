mod audio;
mod core;
mod gba;
mod gbc;
mod save;
mod ui;

use audio::{AudioPlayer, AudioProducer};
use core::{Button, EmulatorCore};
use gba::GbaCore;
use gbc::GbcCore;
use log::{info, warn};
use pixels::{Pixels, SurfaceTexture};
use std::time::Instant;
use ui::{GuiAction, GuiRenderer};
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

/// Flushes battery-backed save data from the active core to its `.sav` file on disk.
fn flush_core_save(core: &dyn EmulatorCore) {
    if let Some(save_path) = core.save_path() {
        if let Some(save_data) = core.get_save_data() {
            if !save_data.is_empty() {
                if let Err(err) = save::SaveManager::write_save_file(&save_path, save_data) {
                    warn!("Failed to flush save file {:?}: {}", save_path, err);
                }
            }
        }
    }
}

/// Apply a core switch: update width/height, resize the pixel buffer and immediately
/// resize the Metal surface so both stay in sync. Returns true on full success.
fn apply_core_switch(
    active_core: &dyn EmulatorCore,
    core_width: &mut u32,
    core_height: &mut u32,
    pixels: &mut Pixels,
    window: &winit::window::Window,
) {
    let (w, h) = active_core.display_dimensions();
    *core_width = w;
    *core_height = h;

    if let Err(err) = pixels.resize_buffer(w, h) {
        warn!("resize_buffer failed: {:?}", err);
    }

    // Keep the Metal swapchain surface in sync with the new buffer dimensions
    let win_size = window.inner_size();
    if win_size.width > 0 && win_size.height > 0 {
        if let Err(err) = pixels.resize_surface(win_size.width, win_size.height) {
            warn!("resize_surface after core switch failed: {:?}", err);
        }
    }
}

fn load_rom_from_path(
    path: &std::path::Path,
    active_core: &mut Box<dyn EmulatorCore>,
    core_width: &mut u32,
    core_height: &mut u32,
    pixels: &mut Pixels,
    window: &winit::window::Window,
    audio_producer: &Option<AudioProducer>,
) -> bool {
    // Flush save data from previous core before switching
    flush_core_save(active_core.as_ref());

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let save_path = save::SaveManager::get_save_path(path);

    match ext.as_str() {
        "gb" | "gbc" => {
            info!("Ingesting Game Boy / GBC ROM: {}", path.display());
            let mut gbc = GbcCore::new();
            gbc.set_audio_producer(audio_producer.clone());
            match gbc.load_rom_file(path) {
                Ok(_) => {
                    if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                        gbc.load_save_data(&save_data);
                    }
                    *active_core = Box::new(gbc);
                    apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                    window.set_title(&format!(
                        "PixelDrive - GBC: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                    true
                }
                Err(err) => {
                    warn!("Failed to load GBC ROM: {}", err);
                    false
                }
            }
        }
        "gba" => {
            info!("Ingesting Game Boy Advance ROM: {}", path.display());
            let mut gba = GbaCore::new();
            gba.set_audio_producer(audio_producer.clone());
            match gba.load_rom_file(path) {
                Ok(header) => {
                    if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                        gba.load_save_data(&save_data);
                    }
                    let backend_label = if let Some(ref lr) = gba.libretro {
                        format!(" [Libretro: {}]", lr.library_name)
                    } else {
                        "".to_string()
                    };
                    let title = format!(
                        "PixelDrive - GBA: {} [{}]{}",
                        header.title, header.game_code, backend_label
                    );
                    *active_core = Box::new(gba);
                    apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                    window.set_title(&title);
                    true
                }
                Err(err) => {
                    warn!("Failed to load GBA ROM: {}", err);
                    false
                }
            }
        }
        "zip" => {
            info!("Ingesting ZIP ROM archive: {}", path.display());
            let mut gba = GbaCore::new();
            gba.set_audio_producer(audio_producer.clone());
            if let Ok(header) = gba.load_rom_file(path) {
                if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                    gba.load_save_data(&save_data);
                }
                let backend_label = if let Some(ref lr) = gba.libretro {
                    format!(" [Libretro: {}]", lr.library_name)
                } else {
                    "".to_string()
                };
                let title = format!(
                    "PixelDrive - GBA: {} [{}]{}",
                    header.title, header.game_code, backend_label
                );
                *active_core = Box::new(gba);
                apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                window.set_title(&title);
                true
            } else {
                let mut gbc = GbcCore::new();
                gbc.set_audio_producer(audio_producer.clone());
                if let Ok(_) = gbc.load_rom_file(path) {
                    if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                        gbc.load_save_data(&save_data);
                    }
                    *active_core = Box::new(gbc);
                    apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                    window.set_title(&format!(
                        "PixelDrive - GBC: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                    true
                } else {
                    warn!("No valid GBC or GBA ROM found inside ZIP: {}", path.display());
                    false
                }
            }
        }
        _ => {
            info!("Unknown extension '.{}', auto-detecting core: {}", ext, path.display());
            let mut gba = GbaCore::new();
            gba.set_audio_producer(audio_producer.clone());
            if let Ok(header) = gba.load_rom_file(path) {
                if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                    gba.load_save_data(&save_data);
                }
                let backend_label = if let Some(ref lr) = gba.libretro {
                    format!(" [Libretro: {}]", lr.library_name)
                } else {
                    "".to_string()
                };
                let title = format!(
                    "PixelDrive - GBA: {} [{}]{}",
                    header.title, header.game_code, backend_label
                );
                *active_core = Box::new(gba);
                apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                window.set_title(&title);
                true
            } else {
                let mut gbc = GbcCore::new();
                gbc.set_audio_producer(audio_producer.clone());
                if let Ok(_) = gbc.load_rom_file(path) {
                    if let Some(save_data) = save::SaveManager::load_save_file(&save_path) {
                        gbc.load_save_data(&save_data);
                    }
                    *active_core = Box::new(gbc);
                    apply_core_switch(active_core.as_ref(), core_width, core_height, pixels, window);
                    window.set_title(&format!(
                        "PixelDrive - GBC: {}",
                        path.file_name().unwrap_or_default().to_string_lossy()
                    ));
                    true
                } else {
                    warn!("Unsupported or unreadable ROM file: {}", path.display());
                    false
                }
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    info!("Starting PixelDrive Handheld Emulator with OSD & egui Overlay...");

    let event_loop = EventLoop::new()?;

    // Initialize Host Audio Player & Ring Buffer Producer
    let (audio_player, audio_producer) = match AudioPlayer::new() {
        Ok(player) => {
            let prod = player.producer();
            (Some(player), Some(prod))
        }
        Err(err) => {
            warn!("Failed to initialize audio player: {}. Audio output will be disabled.", err);
            (None, None)
        }
    };

    if let Some(ref prod) = audio_producer {
        gba::libretro::set_global_audio_producer(Some(prod.clone()));
    }

    // Active emulator core defaults to GBC Core (160x144)
    let mut initial_gbc = GbcCore::new();
    initial_gbc.set_audio_producer(audio_producer.clone());
    let mut active_core: Box<dyn EmulatorCore> = Box::new(initial_gbc);
    let (mut core_width, mut core_height) = active_core.display_dimensions();

    // Default window size: 4x scale for GBC (640x576)
    let window_width = core_width * 4;
    let window_height = core_height * 4;

    let window = std::sync::Arc::new(
        WindowBuilder::new()
            .with_title("PixelDrive - Game Boy Color / Game Boy Advance Emulator")
            .with_inner_size(LogicalSize::new(window_width, window_height))
            .with_min_inner_size(LogicalSize::new(core_width, core_height))
            .build(&event_loop)?,
    );

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, window.clone());
        Pixels::new(core_width, core_height, surface_texture)?
    };

    // Ensure saves directory exists
    let _ = save::SaveManager::ensure_save_directory();

    let window_size = window.inner_size();
    let mut gui = GuiRenderer::new(&window, window_size.width, window_size.height);

    let mut current_rom_path: Option<std::path::PathBuf> = None;
    let mut active_save_slot: usize = 1;
    let mut fast_forward = false;
    let mut is_paused = false;

    // Check for CLI ROM argument on startup: cargo run -- path/to/game.gba
    if let Some(cli_rom_arg) = std::env::args().nth(1) {
        let cli_path = std::path::PathBuf::from(cli_rom_arg);
        if cli_path.exists() {
            info!("CLI ROM argument detected: {}", cli_path.display());
            if load_rom_from_path(
                &cli_path,
                &mut active_core,
                &mut core_width,
                &mut core_height,
                &mut pixels,
                &window,
                &audio_producer,
            ) {
                let name = cli_path.file_name().unwrap_or_default().to_string_lossy().to_string();
                gui.loaded_rom_name = Some(name.clone());
                gui.active_core_name = if active_core.display_dimensions() == (240, 160) { "GBA".to_string() } else { "GBC".to_string() };
                gui.show_toast(format!("Loaded: {}", name));
                current_rom_path = Some(cli_path);
            }
        } else {
            warn!("CLI ROM file path does not exist: {}", cli_path.display());
        }
    }

    let mut last_frame_time = Instant::now();
    let mut last_save_time = Instant::now();
    let mut last_fps_calc = Instant::now();
    let mut fps_frame_count: u32 = 0;

    let frame_duration = std::time::Duration::from_nanos(1_000_000_000 / 60);
    let auto_save_interval = std::time::Duration::from_secs(5);

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { event, .. } => {
                // Let egui handle mouse/keyboard events first
                let consumed = gui.handle_event(&window, &event);

                match event {
                    WindowEvent::CloseRequested => {
                        info!("Closing PixelDrive.");
                        flush_core_save(active_core.as_ref());
                        elwt.exit();
                    }

                    WindowEvent::Resized(new_size) => {
                        if new_size.width > 0 && new_size.height > 0 {
                            if let Err(err) = pixels.resize_surface(new_size.width, new_size.height) {
                                warn!("Pixels surface resize error: {:?}", err);
                            }
                            gui.resize(new_size.width, new_size.height, window.scale_factor() as f32);
                        }
                    }

                    WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                        let inner = window.inner_size();
                        gui.resize(inner.width, inner.height, scale_factor as f32);
                    }

                    WindowEvent::DroppedFile(path) => {
                        info!("ROM Drag & Drop detected: {:?}", path);
                        if load_rom_from_path(
                            &path,
                            &mut active_core,
                            &mut core_width,
                            &mut core_height,
                            &mut pixels,
                            &window,
                            &audio_producer,
                        ) {
                            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                            gui.loaded_rom_name = Some(name.clone());
                            gui.active_core_name = if active_core.display_dimensions() == (240, 160) { "GBA".to_string() } else { "GBC".to_string() };
                            gui.show_toast(format!("Loaded: {}", name));
                            current_rom_path = Some(path);
                        }
                    }

                    WindowEvent::Focused(focused) => {
                        if let Some(ref player) = audio_player {
                            if focused && !is_paused {
                                player.resume();
                            } else {
                                player.pause();
                            }
                        }
                    }

                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(key_code),
                                state,
                                ..
                            },
                        ..
                    } if !consumed => {
                        let pressed = state == ElementState::Pressed;

                        // Hotkeys on key press
                        if pressed {
                            match key_code {
                                // Toggle 2x Fast-Forward speed on Tab press
                                KeyCode::Tab => {
                                    fast_forward = !fast_forward;
                                    gui.is_fast_forward = fast_forward;
                                    if let Some(ref prod) = audio_producer {
                                        prod.set_fast_forward(fast_forward);
                                    }
                                    if fast_forward {
                                        info!("⚡ Fast-Forward: Enabled (2x Speed)");
                                        gui.show_toast("Fast-Forward: 2x Speed");
                                    } else {
                                        info!("▶ Fast-Forward: Disabled (1.0x Normal Speed)");
                                        gui.show_toast("Normal Speed (1.0x)");
                                    }
                                }
                                // Toggle Audio Mute on M press
                                KeyCode::KeyM => {
                                    if let Some(ref prod) = audio_producer {
                                        let is_muted = prod.toggle_mute();
                                        gui.is_muted = is_muted;
                                        if is_muted {
                                            info!("🔇 Audio: Muted");
                                            gui.show_toast("Audio: Muted");
                                        } else {
                                            info!("🔊 Audio: Unmuted");
                                            gui.show_toast("Audio: Unmuted");
                                        }
                                    }
                                }
                                KeyCode::Digit1 => {
                                    active_save_slot = 1;
                                    gui.active_save_slot = 1;
                                    info!("Selected Save State Slot: 1");
                                    gui.show_toast("Selected Save Slot 1");
                                }
                                KeyCode::Digit2 => {
                                    active_save_slot = 2;
                                    gui.active_save_slot = 2;
                                    info!("Selected Save State Slot: 2");
                                    gui.show_toast("Selected Save Slot 2");
                                }
                                KeyCode::Digit3 => {
                                    active_save_slot = 3;
                                    gui.active_save_slot = 3;
                                    info!("Selected Save State Slot: 3");
                                    gui.show_toast("Selected Save Slot 3");
                                }
                                KeyCode::Digit4 => {
                                    active_save_slot = 4;
                                    gui.active_save_slot = 4;
                                    info!("Selected Save State Slot: 4");
                                    gui.show_toast("Selected Save Slot 4");
                                }
                                KeyCode::Digit5 => {
                                    active_save_slot = 5;
                                    gui.active_save_slot = 5;
                                    info!("Selected Save State Slot: 5");
                                    gui.show_toast("Selected Save Slot 5");
                                }
                                KeyCode::Digit6 => {
                                    active_save_slot = 6;
                                    gui.active_save_slot = 6;
                                    info!("Selected Save State Slot: 6");
                                    gui.show_toast("Selected Save Slot 6");
                                }
                                KeyCode::Digit7 => {
                                    active_save_slot = 7;
                                    gui.active_save_slot = 7;
                                    info!("Selected Save State Slot: 7");
                                    gui.show_toast("Selected Save Slot 7");
                                }
                                KeyCode::Digit8 => {
                                    active_save_slot = 8;
                                    gui.active_save_slot = 8;
                                    info!("Selected Save State Slot: 8");
                                    gui.show_toast("Selected Save Slot 8");
                                }
                                KeyCode::Digit9 => {
                                    active_save_slot = 9;
                                    gui.active_save_slot = 9;
                                    info!("Selected Save State Slot: 9");
                                    gui.show_toast("Selected Save Slot 9");
                                }
                                KeyCode::F1 => {
                                    if let Some(ref rom_p) = current_rom_path {
                                        let state_path = save::SaveManager::get_state_path(rom_p, active_save_slot);
                                        if let Some(data) = active_core.save_state() {
                                            if let Err(err) = save::SaveManager::write_save_state(&state_path, &data) {
                                                warn!("Failed to save state to {:?}: {}", state_path, err);
                                                gui.show_toast(format!("Save Failed: {}", err));
                                            } else {
                                                info!("Real-time State Saved -> Slot {} ({:?})", active_save_slot, state_path);
                                                gui.show_toast(format!("State Saved -> Slot {}", active_save_slot));
                                            }
                                        } else {
                                            warn!("Active core failed to capture real-time state snapshot");
                                            gui.show_toast("Failed to capture state snapshot");
                                        }
                                    } else {
                                        warn!("No ROM is currently loaded to save state");
                                        gui.show_toast("No ROM loaded");
                                    }
                                }
                                KeyCode::F5 | KeyCode::F2 => {
                                    if let Some(ref rom_p) = current_rom_path {
                                        let state_path = save::SaveManager::get_state_path(rom_p, active_save_slot);
                                        if let Some(data) = save::SaveManager::read_save_state(&state_path) {
                                            if active_core.load_state(&data) {
                                                info!("Real-time State Restored <- Slot {} ({:?})", active_save_slot, state_path);
                                                gui.show_toast(format!("State Restored <- Slot {}", active_save_slot));
                                            } else {
                                                warn!("Active core failed to restore state snapshot from {:?}", state_path);
                                                gui.show_toast("Failed to restore state snapshot");
                                            }
                                        } else {
                                            gui.show_toast(format!("No save state in slot {}", active_save_slot));
                                        }
                                    } else {
                                        warn!("No ROM is currently loaded to load state");
                                        gui.show_toast("No ROM loaded");
                                    }
                                }
                                _ => {}
                            }
                        }

                        let button = match key_code {
                            KeyCode::ArrowUp | KeyCode::KeyW => Some(Button::Up),
                            KeyCode::ArrowDown | KeyCode::KeyS => Some(Button::Down),
                            KeyCode::ArrowLeft | KeyCode::KeyA => Some(Button::Left),
                            KeyCode::ArrowRight | KeyCode::KeyD => Some(Button::Right),
                            KeyCode::KeyZ | KeyCode::KeyJ => Some(Button::A),
                            KeyCode::KeyX | KeyCode::KeyK => Some(Button::B),
                            KeyCode::KeyQ | KeyCode::KeyU => Some(Button::L),
                            KeyCode::KeyE | KeyCode::KeyI => Some(Button::R),
                            KeyCode::Enter => Some(Button::Start),
                            KeyCode::ShiftRight | KeyCode::ShiftLeft | KeyCode::Backspace => Some(Button::Select),
                            _ => None,
                        };

                        if let Some(btn) = button {
                            active_core.handle_input(btn, pressed);
                        }
                    }

                    WindowEvent::RedrawRequested => {
                        // FPS calculation
                        fps_frame_count += 1;
                        let now = Instant::now();
                        if now.duration_since(last_fps_calc) >= std::time::Duration::from_millis(400) {
                            let elapsed = now.duration_since(last_fps_calc).as_secs_f32();
                            gui.fps = fps_frame_count as f32 / elapsed;
                            gui.frame_time_ms = 1000.0 / gui.fps.max(1.0);
                            fps_frame_count = 0;
                            last_fps_calc = now;
                        }

                        if !is_paused {
                            let steps = if fast_forward { 2 } else { 1 };

                            for _ in 0..steps {
                                active_core.step_frame();

                                // Forward any core-buffered audio samples to host stream
                                let audio_samples = active_core.audio_buffer();
                                if !audio_samples.is_empty() {
                                    if let Some(ref prod) = audio_producer {
                                        prod.push_f32_slice(&audio_samples);
                                    }
                                }
                            }
                        }

                        let frame = pixels.frame_mut();
                        let fb = active_core.framebuffer();

                        // Guard: framebuffer and pixel buffer must match in size.
                        if frame.len() == fb.len() {
                            frame.copy_from_slice(fb);
                        } else {
                            log::debug!(
                                "Framebuffer size mismatch: pixels={} core={} — skipping frame until resize propagates",
                                frame.len(), fb.len()
                            );
                            let (w, h) = active_core.display_dimensions();
                            let _ = pixels.resize_buffer(w, h);
                        }

                        // Prepare egui UI & process actions
                        let actions = gui.prepare_ui(&window);
                        for action in actions {
                            match action {
                                GuiAction::OpenRomPicker => {
                                    if let Some(file) = rfd::FileDialog::new()
                                        .add_filter("Game Boy & GBA ROMs", &["gb", "gbc", "gba", "zip"])
                                        .pick_file()
                                    {
                                        if load_rom_from_path(
                                            &file,
                                            &mut active_core,
                                            &mut core_width,
                                            &mut core_height,
                                            &mut pixels,
                                            &window,
                                            &audio_producer,
                                        ) {
                                            let name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                                            gui.loaded_rom_name = Some(name.clone());
                                            gui.active_core_name = if active_core.display_dimensions() == (240, 160) { "GBA".to_string() } else { "GBC".to_string() };
                                            gui.show_toast(format!("Loaded: {}", name));
                                            current_rom_path = Some(file);
                                        }
                                    }
                                }
                                GuiAction::LoadRom(file) => {
                                    if load_rom_from_path(
                                        &file,
                                        &mut active_core,
                                        &mut core_width,
                                        &mut core_height,
                                        &mut pixels,
                                        &window,
                                        &audio_producer,
                                    ) {
                                        let name = file.file_name().unwrap_or_default().to_string_lossy().to_string();
                                        gui.loaded_rom_name = Some(name.clone());
                                        gui.active_core_name = if active_core.display_dimensions() == (240, 160) { "GBA".to_string() } else { "GBC".to_string() };
                                        gui.show_toast(format!("Loaded: {}", name));
                                        current_rom_path = Some(file);
                                    }
                                }
                                GuiAction::UnloadRom => {
                                    flush_core_save(active_core.as_ref());
                                    let mut gbc = GbcCore::new();
                                    gbc.set_audio_producer(audio_producer.clone());
                                    active_core = Box::new(gbc);
                                    apply_core_switch(active_core.as_ref(), &mut core_width, &mut core_height, &mut pixels, &window);
                                    gui.loaded_rom_name = None;
                                    current_rom_path = None;
                                    gui.show_toast("ROM Unloaded");
                                }
                                GuiAction::Exit => {
                                    flush_core_save(active_core.as_ref());
                                    elwt.exit();
                                }
                                GuiAction::TogglePause => {
                                    is_paused = !is_paused;
                                    gui.is_paused = is_paused;
                                    if is_paused {
                                        gui.show_toast("Emulation Paused");
                                    } else {
                                        gui.show_toast("Emulation Resumed");
                                    }
                                }
                                GuiAction::Reset => {
                                    if let Some(ref rom_p) = current_rom_path {
                                        let p = rom_p.clone();
                                        load_rom_from_path(
                                            &p,
                                            &mut active_core,
                                            &mut core_width,
                                            &mut core_height,
                                            &mut pixels,
                                            &window,
                                            &audio_producer,
                                        );
                                        gui.show_toast("Core Reset");
                                    }
                                }
                                GuiAction::ToggleFastForward => {
                                    fast_forward = !fast_forward;
                                    gui.is_fast_forward = fast_forward;
                                    if let Some(ref prod) = audio_producer {
                                        prod.set_fast_forward(fast_forward);
                                    }
                                    if fast_forward {
                                        gui.show_toast("Fast-Forward: 2x Speed");
                                    } else {
                                        gui.show_toast("Normal Speed (1.0x)");
                                    }
                                }
                                GuiAction::QuickSave(slot) => {
                                    if let Some(ref rom_p) = current_rom_path {
                                        let state_path = save::SaveManager::get_state_path(rom_p, slot);
                                        if let Some(data) = active_core.save_state() {
                                            if let Err(err) = save::SaveManager::write_save_state(&state_path, &data) {
                                                warn!("Failed to save state to {:?}: {}", state_path, err);
                                                gui.show_toast(format!("Save Failed: {}", err));
                                            } else {
                                                info!("Real-time State Saved -> Slot {} ({:?})", slot, state_path);
                                                gui.show_toast(format!("State Saved -> Slot {}", slot));
                                            }
                                        }
                                    } else {
                                        gui.show_toast("No ROM Loaded");
                                    }
                                }
                                GuiAction::QuickLoad(slot) => {
                                    if let Some(ref rom_p) = current_rom_path {
                                        let state_path = save::SaveManager::get_state_path(rom_p, slot);
                                        if let Some(data) = save::SaveManager::read_save_state(&state_path) {
                                            if active_core.load_state(&data) {
                                                info!("Real-time State Restored <- Slot {} ({:?})", slot, state_path);
                                                gui.show_toast(format!("State Restored <- Slot {}", slot));
                                            } else {
                                                gui.show_toast("Failed to load state snapshot");
                                            }
                                        } else {
                                            gui.show_toast(format!("No state in slot {}", slot));
                                        }
                                    } else {
                                        gui.show_toast("No ROM Loaded");
                                    }
                                }
                                GuiAction::SelectSlot(slot) => {
                                    active_save_slot = slot;
                                    gui.active_save_slot = slot;
                                    gui.show_toast(format!("Selected Slot {}", slot));
                                }
                                GuiAction::SetVolume(vol) => {
                                    gui.master_volume = vol;
                                    if let Some(ref player) = audio_player {
                                        player.set_volume(vol);
                                    } else if let Some(ref prod) = audio_producer {
                                        prod.set_volume(vol);
                                    }
                                }
                                GuiAction::ToggleMute => {
                                    if let Some(ref prod) = audio_producer {
                                        let is_m = prod.toggle_mute();
                                        gui.is_muted = is_m;
                                        if is_m {
                                            gui.show_toast("Audio Muted");
                                        } else {
                                            gui.show_toast("Audio Unmuted");
                                        }
                                    }
                                }
                                GuiAction::ToggleFpsHud => {
                                    // Managed inside gui state
                                }
                            }
                        }

                        // Render Pixels framebuffer + egui overlay
                        let render_res = pixels.render_with(|encoder, render_target, context| {
                            context.scaling_renderer.render(encoder, render_target);
                            gui.render(encoder, render_target, context, &window);
                            Ok(())
                        });

                        if let Err(err) = render_res {
                            warn!("Pixels render error: {:?}", err);
                        }
                    }

                    _ => {}
                }
            }

            Event::AboutToWait => {
                let now = Instant::now();

                // Periodic auto-save flush every 5 seconds
                if now.duration_since(last_save_time) >= auto_save_interval {
                    last_save_time = now;
                    flush_core_save(active_core.as_ref());
                }

                if now.duration_since(last_frame_time) >= frame_duration {
                    last_frame_time = now;
                    window.request_redraw();
                }
            }

            _ => {}
        }
    })?;

    Ok(())
}
