//! Android NativeActivity lifecycle handler, WGPU/Pixels surface management,
//! low-latency audio lifecycle, tactile haptics, SAF scoped storage, and thermal frame pacing.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, KeyAction, MotionAction};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};
use log::{debug, error, info, warn};
use pixels::{Pixels, PixelsBuilder, SurfaceTexture};
use raw_window_handle::{
    AndroidDisplayHandle, AndroidNdkWindowHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawDisplayHandle, RawWindowHandle,
};

use crate::audio::AudioProducer;
use crate::core::EmulatorCore;
use crate::gba::{GbaCore, GbaHeader};
use crate::gbc::GbcCore;
use crate::input::{InputSource, JoypadState, TouchAction, TouchOverlay};
use crate::platform::android::audio::AndroidAudioPlayer;
use crate::platform::android::haptics::AndroidHaptics;
use crate::platform::android::storage::jni_bridge;
use crate::platform::android::storage::AndroidStorage;
use crate::platform::PlatformStorage;
use crate::render::{FilterMode, ShaderPipeline, TouchOverlayRenderer};
use crate::save::SaveManager;

/// Thread-safe queue storing pending Content URIs selected via Android SAF ROM picker.
static PENDING_ROM_URI: Mutex<Option<String>> = Mutex::new(None);

/// JNI entrypoint invoked by `MainActivity.nativeOnRomSelected(String uriString)`.
#[no_mangle]
pub extern "system" fn Java_com_pixeldrive_emulator_MainActivity_nativeOnRomSelected(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    uri_jstring: jni::objects::JString,
) {
    if let Ok(uri) = env.get_string(&uri_jstring) {
        let uri_str: String = uri.into();
        info!("JNI nativeOnRomSelected received Content URI: {}", uri_str);
        if let Ok(mut lock) = PENDING_ROM_URI.lock() {
            *lock = Some(uri_str);
        }
    }
}

/// Polls for a recently selected SAF Content URI.
pub fn poll_pending_rom_uri() -> Option<String> {
    let mut lock = PENDING_ROM_URI.lock().ok()?;
    lock.take()
}

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

/// Helper to create Pixels WGPU surface with dynamic surface capability querying and format negotiation.
fn create_pixels_surface(
    core_width: u32,
    core_height: u32,
    window_width: u32,
    window_height: u32,
    native_window_ptr: *mut std::ffi::c_void,
) -> Result<Pixels<'static>, pixels::Error> {
    let window_handle = AndroidWindowWrapper { ptr: native_window_ptr };

    // 1. Initialize WGPU Instance with GL backend to probe supported surface formats
    let instance = pixels::wgpu::Instance::new(pixels::wgpu::InstanceDescriptor {
        backends: pixels::wgpu::Backends::GL,
        ..Default::default()
    });

    let (chosen_format, chosen_present_mode) = match instance.create_surface(&window_handle) {
        Ok(surface) => {
            let adapter = pollster::block_on(instance.request_adapter(&pixels::wgpu::RequestAdapterOptions {
                power_preference: pixels::wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }));

            if let Some(ref adapter) = adapter {
                let caps = surface.get_capabilities(adapter);
                info!(
                    "Android GLES Surface Capabilities: formats={:?}, present_modes={:?}, alpha_modes={:?}",
                    caps.formats, caps.present_modes, caps.alpha_modes
                );

                let format = caps
                    .formats
                    .iter()
                    .copied()
                    .find(|f| f.is_srgb())
                    .or_else(|| caps.formats.first().copied())
                    .unwrap_or(pixels::wgpu::TextureFormat::Rgba8UnormSrgb);

                let present_mode = if caps.present_modes.contains(&pixels::wgpu::PresentMode::Fifo) {
                    pixels::wgpu::PresentMode::Fifo
                } else {
                    caps.present_modes.first().copied().unwrap_or(pixels::wgpu::PresentMode::Fifo)
                };

                (format, present_mode)
            } else {
                (pixels::wgpu::TextureFormat::Rgba8UnormSrgb, pixels::wgpu::PresentMode::Fifo)
            }
        }
        Err(err) => {
            warn!("Failed to create probe surface for capabilities: {:?}", err);
            (pixels::wgpu::TextureFormat::Rgba8UnormSrgb, pixels::wgpu::PresentMode::Fifo)
        }
    };

    info!(
        "Configuring Pixels surface: format={:?}, present_mode={:?}",
        chosen_format, chosen_present_mode
    );

    let window_handle = AndroidWindowWrapper { ptr: native_window_ptr };
    let surface_texture = SurfaceTexture::new(window_width.max(1), window_height.max(1), window_handle);

    // Primary: OpenGL ES with queried supported format & present mode
    PixelsBuilder::new(core_width, core_height, surface_texture)
        .wgpu_backend(pixels::wgpu::Backends::GL)
        .surface_texture_format(chosen_format)
        .render_texture_format(chosen_format)
        .present_mode(chosen_present_mode)
        .build()
        .or_else(|err| {
            warn!("Pixels GL backend initialization failed ({:?}), retrying with Vulkan fallback...", err);
            let window_handle = AndroidWindowWrapper { ptr: native_window_ptr };
            let surface_texture = SurfaceTexture::new(window_width.max(1), window_height.max(1), window_handle);
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

/// Launches the Android SAF ROM file picker via JNI call to `MainActivity.openRomPicker()`.
fn launch_saf_picker(jvm_ptr: *mut std::ffi::c_void, activity_ptr: *mut std::ffi::c_void) {
    if jvm_ptr.is_null() || activity_ptr.is_null() {
        return;
    }
    if let Ok(vm) = unsafe { jni::JavaVM::from_raw(jvm_ptr.cast()) } {
        if let Ok(mut env) = vm.attach_current_thread() {
            let act_obj = unsafe { jni::objects::JObject::from_raw(activity_ptr as _) };
            if let Err(err) = env.call_method(&act_obj, "openRomPicker", "()V", &[]) {
                warn!("Failed to invoke MainActivity.openRomPicker(): {:?}", err);
            } else {
                info!("Successfully launched SAF ROM Document Picker");
            }
        }
    }
}

/// Hot-loads raw ROM bytes into the appropriate emulator core (GBA or GBC), restoring battery saves.
fn load_rom_bytes_into_core(
    rom_bytes: &[u8],
    filename_hint: &str,
    active_core: &mut Box<dyn EmulatorCore>,
    storage: &AndroidStorage,
    current_game_title: &mut String,
    core_width: &mut u32,
    core_height: &mut u32,
    audio_producer: &Option<AudioProducer>,
) -> bool {
    if rom_bytes.is_empty() {
        warn!("Cannot load empty ROM byte buffer");
        return false;
    }

    // 1. Flush active game's SRAM save data before switching
    flush_core_save(active_core.as_ref(), storage, current_game_title);

    // 2. If it's a zip archive, extract the first valid ROM file
    let mut decompressed_bytes = None;
    let mut resolved_hint = filename_hint.to_string();

    if rom_bytes.starts_with(b"PK\x03\x04") {
        info!("Detected ZIP archive, inspecting compressed entries...");
        let cursor = std::io::Cursor::new(rom_bytes);
        if let Ok(mut archive) = zip::ZipArchive::new(cursor) {
            for i in 0..archive.len() {
                if let Ok(mut file) = archive.by_index(i) {
                    let name = file.name().to_lowercase();
                    if name.ends_with(".gba") || name.ends_with(".gbc") || name.ends_with(".gb") {
                        let mut buf = Vec::with_capacity(file.size() as usize);
                        if std::io::copy(&mut file, &mut buf).is_ok() {
                            resolved_hint = file.name().to_string();
                            info!("Extracted ROM '{}' ({} bytes) from ZIP", resolved_hint, buf.len());
                            decompressed_bytes = Some(buf);
                            break;
                        }
                    }
                }
            }
        }
    }

    let bytes = decompressed_bytes.as_deref().unwrap_or(rom_bytes);
    let ext = std::path::Path::new(&resolved_hint)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Drop any existing core first so previous Libretro native instance unloads and deinits cleanly
    *active_core = Box::new(GbcCore::new());

    // 3. Try GBA Core first if extension matches or GBA header parses
    if ext == "gba" || GbaHeader::parse(bytes).is_some() {
        info!("Hot-loading GBA ROM ({} bytes, hint: '{}')", bytes.len(), resolved_hint);
        let mut gba = GbaCore::new();
        gba.set_audio_producer(audio_producer.clone());
        gba.load_rom_with_hint(bytes, &resolved_hint);

        let title = if let Some(header) = GbaHeader::parse(bytes) {
            if !header.title.trim().is_empty() {
                header.title.trim().to_string()
            } else {
                resolved_hint.clone()
            }
        } else {
            resolved_hint.clone()
        };

        let clean_title = SaveManager::sanitize_stem(&title);
        *current_game_title = clean_title.clone();

        if let Some(save_data) = storage.load_save(&clean_title) {
            gba.load_save_data(&save_data);
            info!("Restored GBA battery save for '{}'", clean_title);
        }

        *active_core = Box::new(gba);
        let (w, h) = active_core.display_dimensions();
        *core_width = w;
        *core_height = h;
        info!("Successfully initialized GBA core for '{}' ({}x{})", clean_title, w, h);
        return true;
    }

    // 4. Try GBC Core
    info!("Hot-loading Game Boy / GBC ROM ({} bytes, hint: '{}')", bytes.len(), resolved_hint);
    let mut gbc = GbcCore::new();
    gbc.set_audio_producer(audio_producer.clone());
    gbc.load_rom(bytes);

    let clean_title = SaveManager::sanitize_stem(&resolved_hint);
    *current_game_title = clean_title.clone();

    if let Some(save_data) = storage.load_save(&clean_title) {
        gbc.load_save_data(&save_data);
        info!("Restored GBC battery save for '{}'", clean_title);
    }

    *active_core = Box::new(gbc);
    let (w, h) = active_core.display_dimensions();
    *core_width = w;
    *core_height = h;
    info!("Successfully initialized GBC core for '{}' ({}x{})", clean_title, w, h);
    true
}

/// Main entrypoint invoked by Android `NativeActivity`.
#[no_mangle]
fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_tag("PixelDriveNative"),
    );

    std::panic::set_hook(Box::new(|panic_info| {
        log::error!("CRITICAL RUST PANIC: {:?}", panic_info);
    }));

    info!("=== Starting PixelDrive v1.2.1 (Android NativeActivity) ===");

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_android_app(app);
    }));

    if let Err(err) = result {
        error!("PixelDrive runtime terminated with panic: {:?}", err);
    }
}

/// Core application event and rendering loop.
fn run_android_app(app: AndroidApp) {
    // 1. Initialize Android Scoped Storage directory & Libretro system/save paths
    let base_storage_dir = app
        .internal_data_path()
        .or_else(|| app.external_data_path())
        .unwrap_or_else(|| PathBuf::from("/data/local/tmp/pixeldrive"));

    let system_dir = base_storage_dir.join("system");
    let saves_dir = base_storage_dir.join("saves");
    let _ = std::fs::create_dir_all(&system_dir);
    let _ = std::fs::create_dir_all(&saves_dir);
    crate::gba::libretro::set_directories(&system_dir, &saves_dir);

    let storage = AndroidStorage::new(base_storage_dir);
    let mut current_game_title = "PixelDrive_GBA_Default".to_string();

    // 2. Initialize Native Android Tactile Haptics Engine & JavaVM handle via JNI
    let jvm_ptr = app.vm_as_ptr();
    let activity_ptr = app.activity_as_ptr();

    let haptics = if !jvm_ptr.is_null() && !activity_ptr.is_null() {
        if let Ok(vm) = unsafe { jni::JavaVM::from_raw(jvm_ptr.cast()) } {
            AndroidHaptics::new(vm, activity_ptr)
        } else {
            AndroidHaptics::dummy()
        }
    } else {
        AndroidHaptics::dummy()
    };

    // 3. Initialize Android Low-Latency Audio Stream (AAudio/Oboe <= 30ms latency)
    let mut audio_player: Option<AndroidAudioPlayer> = match AndroidAudioPlayer::new() {
        Ok(player) => {
            info!("Low-latency Android AAudio stream initialized successfully (44,100 Hz, Stereo)");
            Some(player)
        }
        Err(err) => {
            warn!("Failed to initialize Android AAudio stream: {:?}", err);
            None
        }
    };

    let audio_producer = audio_player.as_ref().map(|p| p.producer());

    // 4. Default core initialization (Lightweight idle GBC core before ROM is selected)
    let mut active_core: Box<dyn EmulatorCore> = {
        let mut core = GbcCore::new();
        core.set_audio_producer(audio_producer.clone());
        Box::new(core)
    };
    let (mut core_width, mut core_height) = (240, 160); // Default to standard 240x160 viewport

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
    let mut prev_joypad_state = JoypadState::default();

    // 7. Lifecycle and state flags
    let mut is_paused = false;
    let mut fast_forward = false;
    let mut running = true;

    // 8. Frame pacing and thermal management
    let mut last_frame_time = Instant::now();
    let mut last_save_time = Instant::now();
    let auto_save_interval = Duration::from_secs(5);

    while running {
        // 1. Check for newly selected SAF Content URI
        if let Some(uri_str) = poll_pending_rom_uri() {
            info!("Processing incoming Content URI: {}", uri_str);
            if !jvm_ptr.is_null() && !activity_ptr.is_null() {
                if let Ok(vm) = unsafe { jni::JavaVM::from_raw(jvm_ptr.cast()) } {
                    let act_obj = unsafe { jni::objects::JObject::from_raw(activity_ptr as _) };
                    match jni_bridge::read_bytes_from_content_uri(&vm, &act_obj, &uri_str) {
                        Ok(rom_bytes) => {
                            let filename_hint = uri_str
                                .split('/')
                                .last()
                                .unwrap_or("game.rom")
                                .replace("%20", " ");

                            if load_rom_bytes_into_core(
                                &rom_bytes,
                                &filename_hint,
                                &mut active_core,
                                &storage,
                                &mut current_game_title,
                                &mut core_width,
                                &mut core_height,
                                &audio_producer,
                            ) {
                                if let Some(ref mut px) = pixels {
                                    if let Err(err) = px.resize_buffer(core_width, core_height) {
                                        warn!("Failed to resize Pixels buffer after ROM load: {:?}", err);
                                    }
                                }
                                is_paused = false;
                                if let Some(ref mut player) = audio_player {
                                    player.resume_audio_stream();
                                }
                            }
                        }
                        Err(err) => {
                            error!("Failed to stream ROM bytes from Content URI: {}", err);
                        }
                    }
                }
            }
        }

        // 2. Process Android NativeActivity lifecycle and window events
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
                                let w = native_window.width() as u32;
                                let h = native_window.height() as u32;
                                info!("Native window acquired: {}x{}", w, h);

                                if w > 0 && h > 0 {
                                    window_width = w;
                                    window_height = h;

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
                                } else {
                                    info!("Native window dimensions are 0x0; deferring surface creation until WindowResized");
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
                                let w = native_window.width() as u32;
                                let h = native_window.height() as u32;
                                debug!("Android WindowResized: {}x{}", w, h);

                                if w > 0 && h > 0 {
                                    window_width = w;
                                    window_height = h;

                                    if let Some(ref mut px) = pixels {
                                        if let Err(err) = px.resize_surface(window_width, window_height) {
                                            warn!("Failed to resize surface: {:?}", err);
                                        }
                                    } else {
                                        // Deferral resolution: Create surface now that dimensions are non-zero
                                        match create_pixels_surface(
                                            core_width,
                                            core_height,
                                            window_width,
                                            window_height,
                                            native_window.ptr().as_ptr() as *mut std::ffi::c_void,
                                        ) {
                                            Ok(px) => {
                                                let surface_format = px.render_texture_format();
                                                info!("Configuring deferred shader pipeline with surface format: {:?}", surface_format);
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
                                                info!("WGPU surface successfully initialized on WindowResized!");
                                            }
                                            Err(err) => {
                                                error!("Failed to create deferred Pixels surface: {:?}", err);
                                            }
                                        }
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

        // 3. Process Android Touch and Hardware Input Events
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
                        let changes = touch_state.diff(prev_joypad_state);
                        prev_joypad_state = touch_state;
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

                        // Process non-joypad HUD actions (Fast-Forward, Menu / Load ROM, Quick Save, Quick Load)
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
                                    info!("Fast-Forward toggled: {}", fast_forward);
                                }
                                TouchAction::OpenMenu => {
                                    info!("Menu / Load ROM tapped: launching SAF Document Picker...");
                                    launch_saf_picker(jvm_ptr, activity_ptr);
                                }
                                TouchAction::QuickSave => {
                                    info!("QuickSave triggered for '{}'", current_game_title);
                                    if let Some(state_data) = active_core.save_state() {
                                        if let Err(err) = storage.write_state(&current_game_title, 0, &state_data) {
                                            warn!("QuickSave failed: {}", err);
                                        } else {
                                            info!("QuickSave successfully written slot 0 ({} bytes)", state_data.len());
                                        }
                                    }
                                }
                                TouchAction::QuickLoad => {
                                    info!("QuickLoad triggered for '{}'", current_game_title);
                                    if let Some(state_data) = storage.load_state(&current_game_title, 0) {
                                        if !active_core.load_state(&state_data) {
                                            warn!("QuickLoad failed to restore state snapshot");
                                        } else {
                                            info!("QuickLoad successfully restored state from slot 0");
                                        }
                                    } else {
                                        warn!("No QuickSave state found in slot 0 for '{}'", current_game_title);
                                    }
                                }
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

        // 4. Emulation Stepping & Frame Pacing with Thermal Management
        let surface_ready = pixels.is_some() && shader_pipeline.is_some() && app.native_window().is_some();
        if !is_paused && surface_ready {
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
                        let render_res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            px.render_with(|encoder, render_target, context| {
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
                            })
                        }));

                        match render_res {
                            Ok(Ok(())) => {}
                            Ok(Err(err)) => {
                                warn!("Pixels Android render error: {:?}", err);
                            }
                            Err(panic_err) => {
                                error!("Panic during Pixels render_with: {:?}", panic_err);
                            }
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
