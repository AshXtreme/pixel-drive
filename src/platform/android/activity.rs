//! Android NativeActivity lifecycle handler, WGPU/Pixels surface management,
//! touch event processing, and frame pacing loop for PixelDrive.

use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, MotionAction};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use log::{debug, error, info, warn};
use pixels::{Pixels, SurfaceTexture};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HasRawDisplayHandle, HasRawWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::audio::AudioProducer;
use crate::core::EmulatorCore;
use crate::gba::GbaCore;
use crate::input::{InputManager, TouchOverlay};
use crate::render::{FilterMode, ShaderPipeline, TouchOverlayRenderer};
use crate::save::SaveManager;

/// Safe wrapper around Android `NativeWindow` implementing `HasRawWindowHandle` and `HasRawDisplayHandle` (rwh 0.5).
struct AndroidWindowHandle {
    ptr: *mut std::ffi::c_void,
}

unsafe impl Send for AndroidWindowHandle {}
unsafe impl Sync for AndroidWindowHandle {}

unsafe impl HasRawWindowHandle for AndroidWindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        let mut handle = AndroidNdkWindowHandle::empty();
        handle.a_native_window = self.ptr;
        RawWindowHandle::AndroidNdk(handle)
    }
}

unsafe impl HasRawDisplayHandle for AndroidWindowHandle {
    fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::Android(AndroidDisplayHandle::empty())
    }
}

/// Flushes battery-backed SRAM save data from the active core to scoped storage.
fn flush_core_save(core: &dyn EmulatorCore) {
    if let Some(save_path) = core.save_path() {
        if let Some(save_data) = core.get_save_data() {
            if !save_data.is_empty() {
                if let Err(err) = SaveManager::write_save_file(&save_path, save_data) {
                    warn!("Failed to flush save file {:?}: {}", save_path, err);
                } else {
                    debug!("Flushed battery save data ({} bytes)", save_data.len());
                }
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

    info!("=== Starting PixelDrive v1.2 (Android NativeActivity) ===");

    // Initialize Save directory in Android internal/external storage
    if let Some(internal_path) = app.internal_data_path() {
        info!("Android Internal Data Path: {}", internal_path.display());
    }
    if let Some(external_path) = app.external_data_path() {
        info!("Android External Data Path: {}", external_path.display());
    }
    let _ = SaveManager::ensure_save_directory();

    // Default core initialization (GBA 240x160)
    let mut active_core: Box<dyn EmulatorCore> = Box::new(GbaCore::new());
    let (mut core_width, mut core_height) = active_core.display_dimensions();

    // Audio Producer placeholder for Android AAudio/Oboe backend
    let (audio_producer, mut _audio_consumer) = AudioProducer::new_pair(4096 * 4);
    active_core.load_save_data(&[]);

    // Graphics and pipeline state
    let mut pixels: Option<Pixels> = None;
    let mut shader_pipeline: Option<ShaderPipeline> = None;
    let mut touch_overlay_renderer: Option<TouchOverlayRenderer> = None;
    let mut filter_mode = FilterMode::Nearest;
    let mut window_width: u32 = 0;
    let mut window_height: u32 = 0;

    // Input management and virtual touch overlay
    let mut touch_overlay = TouchOverlay::new();
    let mut input_manager = InputManager::new();

    // Lifecycle and state flags
    let mut is_paused = false;
    let mut fast_forward = false;
    let mut running = true;

    // Timing and frame pacing
    let mut last_frame_time = Instant::now();
    let mut last_save_time = Instant::now();
    let auto_save_interval = Duration::from_secs(5);

    while running {
        // 1. Process Android NativeActivity lifecycle and window events
        let timeout = if is_paused || pixels.is_none() {
            None // Block until next event arrives when paused or without a surface
        } else {
            Some(Duration::from_millis(1)) // Non-blocking polling during active gameplay
        };

        app.poll_events(timeout, |poll_event| {
            match poll_event {
                PollEvent::Main(main_event) => {
                    match main_event {
                        MainEvent::InitWindow { .. } => {
                            info!("Android MainEvent: InitWindow");
                            if let Some(native_window) = app.native_window() {
                                window_width = native_window.width() as u32;
                                window_height = native_window.height() as u32;
                                info!(
                                    "Native window acquired: {}x{}",
                                    window_width, window_height
                                );

                                let window_handle = AndroidWindowHandle {
                                    ptr: native_window.ptr().as_ptr() as *mut std::ffi::c_void,
                                };

                                let surface_texture = SurfaceTexture::new(
                                    window_width,
                                    window_height,
                                    window_handle,
                                );

                                match Pixels::new(core_width, core_height, surface_texture) {
                                    Ok(px) => {
                                        let pipeline = ShaderPipeline::new(
                                            px.device(),
                                            pixels::wgpu::TextureFormat::Rgba8UnormSrgb,
                                        );
                                        let overlay = TouchOverlayRenderer::new(
                                            px.device(),
                                            pixels::wgpu::TextureFormat::Rgba8UnormSrgb,
                                        );
                                        shader_pipeline = Some(pipeline);
                                        touch_overlay_renderer = Some(overlay);
                                        pixels = Some(px);
                                        info!("WGPU Vulkan/GLES surface, ShaderPipeline, and TouchOverlayRenderer successfully initialized!");
                                    }
                                    Err(err) => {
                                        error!("Failed to create Pixels WGPU surface: {:?}", err);
                                    }
                                }
                            }
                        }

                        MainEvent::TermWindow { .. } => {
                            info!("Android MainEvent: TermWindow — Releasing WGPU surface");
                            // Release surface swapchain when backgrounded to prevent GPU driver deadlocks
                            shader_pipeline = None;
                            touch_overlay_renderer = None;
                            pixels = None;
                        }

                        MainEvent::WindowResized { .. } => {
                            if let Some(native_window) = app.native_window() {
                                window_width = native_window.width() as u32;
                                window_height = native_window.height() as u32;
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

                        MainEvent::RedrawRequested { .. } => {
                            // Handled in main frame render loop below
                        }

                        MainEvent::Pause => {
                            info!("Android MainEvent: Pause — Auto-pausing emulation and saving SRAM");
                            is_paused = true;
                            flush_core_save(active_core.as_ref());
                        }

                        MainEvent::Resume => {
                            info!("Android MainEvent: Resume — Resuming emulation");
                            is_paused = false;
                        }

                        MainEvent::InsetsChanged { .. } => {
                            debug!("Android MainEvent: InsetsChanged (display cutout / navigation margins)");
                        }

                        MainEvent::Destroy => {
                            info!("Android MainEvent: Destroy — Exiting PixelDrive");
                            flush_core_save(active_core.as_ref());
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
                                        window_width as f32,
                                        window_height as f32,
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
                                        window_width as f32,
                                        window_height as f32,
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
                        for (btn, pressed) in changes {
                            active_core.handle_input(btn, pressed);
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

        // 3. Emulation Stepping & Frame Pacing
        if !is_paused && pixels.is_some() {
            let now = Instant::now();

            // Periodic auto-save flush
            if now.duration_since(last_save_time) >= auto_save_interval {
                last_save_time = now;
                flush_core_save(active_core.as_ref());
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
                        audio_producer.push_f32_slice(&audio_samples);
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
            }
        }
    }

    info!("=== PixelDrive Android NativeActivity terminated safely ===");
}
