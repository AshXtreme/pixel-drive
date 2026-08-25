//! Android NativeActivity lifecycle handler, WGPU/Pixels surface management,
//! low-latency audio lifecycle, tactile haptics, SAF scoped storage, and thermal frame pacing.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, MotionAction};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use log::{debug, error, info, warn};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::core::EmulatorCore;
use crate::gba::GbaCore;
use crate::input::{InputManager, InputSource, TouchAction, TouchOverlay};
use crate::platform::android::audio::AndroidAudioPlayer;
use crate::platform::android::haptics::AndroidHaptics;
use crate::platform::android::storage::AndroidStorage;
use crate::platform::PlatformStorage;
use crate::render::{FilterMode, ShaderPipeline, TouchOverlayRenderer};

/// Safe wrapper around Android `NativeWindow` implementing `HasWindowHandle` and `HasDisplayHandle` (rwh 0.6).
struct AndroidWindowWrapper {
    ptr: *mut std::ffi::c_void,
}

unsafe impl Send for AndroidWindowWrapper {}
unsafe impl Sync for AndroidWindowWrapper {}

impl HasWindowHandle for AndroidWindowWrapper {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, HandleError> {
        let handle = AndroidNdkWindowHandle::new(
            std::ptr::NonNull::new(self.ptr).ok_or(HandleError::Unavailable)?,
        );
        unsafe { Ok(raw_window_handle::WindowHandle::borrow_raw(RawWindowHandle::AndroidNdk(handle))) }
    }
}

impl HasDisplayHandle for AndroidWindowWrapper {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, HandleError> {
        let handle = AndroidDisplayHandle::new();
        unsafe { Ok(raw_window_handle::DisplayHandle::borrow_raw(RawDisplayHandle::Android(handle))) }
    }
}

/// Helper to create Pixels WGPU surface with automatic Vulkan -> GLES fallback.
fn create_pixels_surface(
    core_width: u32,
    core_height: u32,
    window_width: u32,
    window_height: u32,
    native_window_ptr: *mut std::ffi::c_void,
) -> Result<Pixels<'static>, pixels::Error> {
    let window_handle = AndroidWindowWrapper { ptr: native_window_ptr };
    let surface_texture = SurfaceTexture::new(window_width, window_height, window_handle);

    // Primary: OpenGL ES (EGL / GLES3) is 100% universal across all physical devices and Android emulators
    PixelsBuilder::new(core_width, core_height, surface_texture)
        .wgpu_backend(pixels::wgpu::Backends::GL)
        .present_mode(pixels::wgpu::PresentMode::Fifo)
        .build()
        .or_else(|err| {
            warn!("Pixels GL backend initialization failed ({:?}), retrying with Vulkan...", err);
            let window_handle = AndroidWindowWrapper { ptr: native_window_ptr };
            let surface_texture = SurfaceTexture::new(window_width, window_height, window_handle);
            PixelsBuilder::new(core_width, core_height, surface_texture)
                .wgpu_backend(pixels::wgpu::Backends::VULKAN)
                .present_mode(pixels::wgpu::PresentMode::AutoVsync)
                .build()
        })
}

/// Flushes battery-backed SRAM save data from the active core to scoped storage.
fn flush_core_save(core: &dyn EmulatorCore, storage: &AndroidStorage, game_title: &str) {
    if let Some(save_data) = core.get_save_data() {
        if !save_data.is_empty() {
            if let Err(err) = storage.flush_sram(game_title, save_data) {
                warn!("Failed to flush SRAM save file for {}: {}", game_title, err);
            } else {
                debug!("Flushed battery save data for {} ({} bytes)", game_title, save_data.len());
            }
        }
    }
}

/// Main entrypoint invoked by Android `NativeActivity`.
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("PixelDrive"),
    );

    std::panic::set_hook(Box::new(|panic_info| {
        error!("CRITICAL RUST PANIC in PixelDrive: {:?}", panic_info);
    }));

    info!("=== Starting PixelDrive v1.2 (Android NativeActivity) ===");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_android_app(app);
    }));

    if let Err(err) = result {
        error!("PixelDrive runtime terminated with panic: {:?}", err);
    }
}

/// Core application event and rendering loop.
fn run_android_app(app: AndroidApp) {
    // 1. Initialize Android Scoped Storage directory
    let base_storage_dir = app
        .internal_data_path()
        .or_else(|| app.external_data_path())
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/pixeldrive"));

    let storage = AndroidStorage::new(base_storage_dir);
    let current_game_title = "PixelDrive_GBA_Default".to_string();

    // 2. Initialize Native Android Tactile Haptics Engine via JNI
    let jvm_ptr = app.vm_as_ptr();
    let activity_ptr = app.activity_as_ptr();
    let haptics = if !jvm_ptr.is_null() && !activity_ptr.is_null() {
        let vm = unsafe { jni::JavaVM::from_raw(jvm_ptr.cast()) };
        match vm {
            Ok(jvm) => AndroidHaptics::new(jvm, activity_ptr),
            Err(err) => {
                warn!("Failed to initialize JavaVM for haptics: {:?}", err);
                AndroidHaptics::dummy()
            }
        }
    } else {
        AndroidHaptics::dummy()
    };

    // 3. Default core initialization (GBA 240x160)
    let mut active_core: Box<dyn EmulatorCore> = Box::new(GbaCore::new());
    let (core_width, core_height) = active_core.display_dimensions();

    // 4. Initialize Android Low-Latency Audio Stream (AAudio/Oboe <= 30ms latency)
    let mut audio_player: Option<AndroidAudioPlayer> = match AndroidAudioPlayer::new() {
        Ok(player) => {
            info!("Low-latency Android AAudio stream initialized successfully");
            Some(player)
        }
        Err(err) => {
            warn!("Failed to initialize Android AAudio stream: {:?}", err);
            None
        }
    };

    let audio_producer = audio_player.as_ref().map(|p| p.producer());

    // Load any existing SRAM save data from scoped storage
    if let Some(sram_data) = storage.load_save(&current_game_title) {
        active_core.load_save_data(&sram_data);
    } else {
        active_core.load_save_data(&[]);
    }

    // 5. Graphics and pipeline state
    let mut pixels: Option<Pixels> = None;
    let mut shader_pipeline: Option<ShaderPipeline> = None;
    let mut touch_overlay_renderer: Option<TouchOverlayRenderer> = None;
    let filter_mode = FilterMode::Nearest;
    let mut window_width: u32 = 0;
    let mut window_height: u32 = 0;

    // 6. Input management and virtual touch overlay
    let mut touch_overlay = TouchOverlay::new();
    let mut input_manager = InputManager::new();

    // 7. Lifecycle and state flags
    let mut is_paused = false;
    let mut fast_forward = false;
    let mut running = true;

    // 8. Frame pacing and thermal management
    let mut last_frame_time = Instant::now();
    let mut last_save_time = Instant::now();
    let auto_save_interval = Duration::from_secs(5);

    while running {
        // 1. Process Android NativeActivity lifecycle and window events
        let timeout = if is_paused || pixels.is_none() {
            Some(Duration::from_millis(16)) // 16ms poll cadence while waiting for window/paused
        } else {
            Some(Duration::from_millis(1)) // 1ms cadence during active frame stepping
        };

        app.poll_events(timeout, |poll_event| {
            match poll_event {
                PollEvent::Main(main_event) => {
                    match main_event {
                        MainEvent::InitWindow { .. } => {
                            info!("Android MainEvent: InitWindow");
                            if let Some(native_window) = app.native_window() {
                                window_width = (native_window.width() as u32).max(240);
                                window_height = (native_window.height() as u32).max(160);
                                info!(
                                    "Native window acquired: {}x{}",
                                    window_width, window_height
                                );

                                match create_pixels_surface(
                                    core_width,
                                    core_height,
                                    window_width,
                                    window_height,
                                    native_window.ptr().as_ptr() as *mut std::ffi::c_void,
                                ) {
                                    Ok(px) => {
                                        let surface_format = px.render_texture_format();
                                        info!("Configuring shader pipeline with surface format: {:?}", surface_format);
                                        let pipeline = ShaderPipeline::new(
                                            px.device(),
                                            surface_format,
                                        );
                                        let overlay = TouchOverlayRenderer::new(
                                            px.device(),
                                            surface_format,
                                        );
                                        shader_pipeline = Some(pipeline);
                                        touch_overlay_renderer = Some(overlay);
                                        pixels = Some(px);
                                        info!("WGPU surface, ShaderPipeline, and TouchOverlayRenderer successfully initialized!");
                                    }
                                    Err(err) => {
                                        error!("Failed to create Pixels surface: {:?}", err);
                                    }
                                }
                            }
                        }

                        MainEvent::TerminateWindow { .. } => {
                            info!("Android MainEvent: TerminateWindow — Releasing WGPU surface");
                            shader_pipeline = None;
                            touch_overlay_renderer = None;
                            pixels = None;
                        }

                        MainEvent::WindowResized { .. } => {
                            if let Some(native_window) = app.native_window() {
                                window_width = (native_window.width() as u32).max(240);
                                window_height = (native_window.height() as u32).max(160);
                                debug!(
                                    "Android WindowResized: {}x{}",
                                    window_width, window_height
                                );

                                if let Some(ref mut px) = pixels {
                                    if let Err(err) =
                                        px.resize_surface(window_width, window_height)
                                    {
                                        warn!("Failed to resize surface: {:?}", err);
                                    }
                                }
                            }
                        }

                        MainEvent::RedrawNeeded { .. } => {
                            // Handled in main frame render loop below
                        }

                        MainEvent::Pause => {
                            info!("Android MainEvent: Pause — Auto-pausing audio, emulation, and flushing SRAM");
                            is_paused = true;
                            if let Some(ref mut player) = audio_player {
                                player.pause_audio_stream();
                            }
                            flush_core_save(active_core.as_ref(), &storage, &current_game_title);
                        }

                        MainEvent::Resume { .. } => {
                            info!("Android MainEvent: Resume — Resuming audio and emulation with clean buffer");
                            is_paused = false;
                            if let Some(ref mut player) = audio_player {
                                player.resume_audio_stream();
                            }
                        }

                        MainEvent::InsetsChanged { .. } => {
                            debug!("Android MainEvent: InsetsChanged (display cutout / navigation margins)");
                        }

                        MainEvent::Destroy => {
                            info!("Android MainEvent: Destroy — Exiting PixelDrive");
                            if let Some(ref mut player) = audio_player {
                                player.pause_audio_stream();
                            }
                            flush_core_save(active_core.as_ref(), &storage, &current_game_title);
                            running = false;
                        }

                        _ => {}
                    }
                }

                PollEvent::Wake => {
                    // Woken by background thread
                }

                _ => {}
            }
        });

        // 2. Process Android Touch and Hardware Input Events
        if let Ok(mut input_iter) = app.input_events_iter() {
            while input_iter.next(|event| {
                match event {
                    InputEvent::MotionEvent(motion) => {
                        let action = motion.action();
                        let ptr_count = motion.pointer_count();

                        match action {
                            MotionAction::Down | MotionAction::PointerDown => {
                                for i in 0..ptr_count {
                                    let pointer = motion.pointer_at_index(i);
                                    touch_overlay.handle_touch_down(
                                        pointer.pointer_id() as u64,
                                        pointer.x(),
                                        pointer.y(),
                                        window_width.max(1) as f32,
                                        window_height.max(1) as f32,
                                    );
                                }
                            }
                            MotionAction::Move => {
                                for i in 0..ptr_count {
                                    let pointer = motion.pointer_at_index(i);
                                    touch_overlay.handle_touch_move(
                                        pointer.pointer_id() as u64,
                                        pointer.x(),
                                        pointer.y(),
                                        window_width.max(1) as f32,
                                        window_height.max(1) as f32,
                                    );
                                }
                            }
                            MotionAction::Up | MotionAction::PointerUp => {
                                for i in 0..ptr_count {
                                    let pointer = motion.pointer_at_index(i);
                                    touch_overlay.handle_touch_up(pointer.pointer_id() as u64);
                                }
                            }
                            MotionAction::Cancel => {
                                for i in 0..ptr_count {
                                    let pointer = motion.pointer_at_index(i);
                                    touch_overlay.handle_touch_cancel(pointer.pointer_id() as u64);
                                }
                            }
                            _ => {}
                        }

                        // Dispatch resolved touch state into active emulator core
                        let touch_state = touch_overlay.poll();
                        let prev_state = input_manager.poll_merged();
                        let changes = touch_state.diff(prev_state);
                        let mut newly_pressed = false;

                        for (btn, pressed) in changes {
                            if pressed {
                                newly_pressed = true;
                            }
                            active_core.handle_input(btn, pressed);
                        }

                        // Tactile Haptic Trigger on virtual button activation
                        if newly_pressed && touch_overlay.is_haptics_enabled() {
                            haptics.vibrate_click();
                        }

                        // Process non-joypad HUD actions (Fast-Forward, Menu)
                        for hud_action in touch_overlay.poll_actions() {
                            if touch_overlay.is_haptics_enabled() {
                                haptics.vibrate_click();
                            }
                            match hud_action {
                                TouchAction::ToggleFastForward => {
                                    fast_forward = !fast_forward;
                                    if let Some(ref player) = audio_player {
                                        player.set_fast_forward(fast_forward);
                                    }
                                }
                                TouchAction::OpenMenu => {
                                    is_paused = !is_paused;
                                    if is_paused {
                                        if let Some(ref mut player) = audio_player {
                                            player.pause_audio_stream();
                                        }
                                        flush_core_save(active_core.as_ref(), &storage, &current_game_title);
                                    } else if let Some(ref mut player) = audio_player {
                                        player.resume_audio_stream();
                                    }
                                }
                                _ => {}
                            }
                        }

                        InputStatus::Handled
                    }

                    InputEvent::KeyEvent(key) => {
                        let _pressed = key.action() == KeyAction::Down;
                        // Key mappings (Volume, Back, Controller D-Pad)
                        InputStatus::Unhandled
                    }

                    _ => InputStatus::Unhandled,
                }
            }) {}
        }

        // 3. Emulation Stepping & Frame Pacing with Thermal Management
        if !is_paused && pixels.is_some() {
            let now = Instant::now();

            // Periodic auto-save flush
            if now.duration_since(last_save_time) >= auto_save_interval {
                last_save_time = now;
                flush_core_save(active_core.as_ref(), &storage, &current_game_title);
            }

            // Sub-millisecond fractional frame pacing (59.7275 Hz normal / 119.455 Hz fast-forward)
            let target_frame_nanos = if fast_forward { 8_371_353 } else { 16_742_706 };
            let target_frame_duration = Duration::from_nanos(target_frame_nanos);
            let elapsed = now.duration_since(last_frame_time);

            if elapsed >= target_frame_duration {
                last_frame_time = if elapsed > target_frame_duration * 2 {
                    now
                } else {
                    last_frame_time + target_frame_duration
                };

                // Step core emulation
                let steps = if fast_forward { 2 } else { 1 };
                for _ in 0..steps {
                    active_core.step_frame();

                    let audio_samples = active_core.audio_buffer();
                    if !audio_samples.is_empty() {
                        if let Some(ref prod) = audio_producer {
                            prod.push_f32_slice(&audio_samples);
                        }
                    }
                }

                // Copy framebuffer and render with WGPU post-processing shader
                if let Some(ref mut px) = pixels {
                    let frame = px.frame_mut();
                    let fb = active_core.framebuffer();

                    if frame.len() == fb.len() {
                        frame.copy_from_slice(fb);
                    }

                    if let Some(ref mut pipeline) = shader_pipeline {
                        let render_res = px.render_with(|encoder, render_target, context| {
                            pipeline.render(
                                encoder,
                                render_target,
                                context,
                                filter_mode,
                                core_width,
                                core_height,
                                window_width,
                                window_height,
                            );
                            if let Some(ref mut overlay) = touch_overlay_renderer {
                                overlay.render(
                                    encoder,
                                    render_target,
                                    context,
                                    &touch_overlay,
                                    window_width,
                                    window_height,
                                );
                            }
                            Ok(())
                        });

                        if let Err(err) = render_res {
                            warn!("Pixels Android render error: {:?}", err);
                        }
                    }
                }
            } else {
                // Thermal sleep loop: Sleep remaining time to prevent thread spinning & thermal throttling
                let remaining = target_frame_duration - elapsed;
                let sleep_margin = Duration::from_micros(500);
                if remaining > sleep_margin {
                    std::thread::sleep(remaining - sleep_margin);
                }
            }
        }
    }

    info!("=== PixelDrive Android NativeActivity terminated safely ===");
}
