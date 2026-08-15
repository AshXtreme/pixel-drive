#![allow(dead_code)]

use log::{debug, error, info, warn};
use std::ffi::{c_char, c_uint, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ============================================================================
// Libretro Constants
// ============================================================================

pub const RETRO_API_VERSION: c_uint = 1;

// Pixel formats
pub const RETRO_PIXEL_FORMAT_0RGB1555: c_uint = 0;
pub const RETRO_PIXEL_FORMAT_XRGB8888: c_uint = 1;
pub const RETRO_PIXEL_FORMAT_RGB565: c_uint = 2;

// Joypad button IDs (RetroPad)
pub const RETRO_DEVICE_ID_JOYPAD_B: c_uint = 0;
pub const RETRO_DEVICE_ID_JOYPAD_Y: c_uint = 1;
pub const RETRO_DEVICE_ID_JOYPAD_SELECT: c_uint = 2;
pub const RETRO_DEVICE_ID_JOYPAD_START: c_uint = 3;
pub const RETRO_DEVICE_ID_JOYPAD_UP: c_uint = 4;
pub const RETRO_DEVICE_ID_JOYPAD_DOWN: c_uint = 5;
pub const RETRO_DEVICE_ID_JOYPAD_LEFT: c_uint = 6;
pub const RETRO_DEVICE_ID_JOYPAD_RIGHT: c_uint = 7;
pub const RETRO_DEVICE_ID_JOYPAD_A: c_uint = 8;
pub const RETRO_DEVICE_ID_JOYPAD_X: c_uint = 9;
pub const RETRO_DEVICE_ID_JOYPAD_L: c_uint = 10;
pub const RETRO_DEVICE_ID_JOYPAD_R: c_uint = 11;
pub const RETRO_DEVICE_ID_JOYPAD_L2: c_uint = 12;
pub const RETRO_DEVICE_ID_JOYPAD_R2: c_uint = 13;
pub const RETRO_DEVICE_ID_JOYPAD_L3: c_uint = 14;
pub const RETRO_DEVICE_ID_JOYPAD_R3: c_uint = 15;

pub const RETRO_DEVICE_JOYPAD: c_uint = 1;
pub const RETRO_DEVICE_MASK: c_uint = 0xff;

// Environment Commands
pub const RETRO_ENVIRONMENT_SET_ROTATION: c_uint = 1;
pub const RETRO_ENVIRONMENT_GET_OVERSCAN: c_uint = 2;
pub const RETRO_ENVIRONMENT_GET_CAN_DUPE: c_uint = 3;
pub const RETRO_ENVIRONMENT_SET_MESSAGE: c_uint = 6;
pub const RETRO_ENVIRONMENT_SHUTDOWN: c_uint = 7;
pub const RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL: c_uint = 8;
pub const RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY: c_uint = 9;
pub const RETRO_ENVIRONMENT_SET_PIXEL_FORMAT: c_uint = 10;
pub const RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS: c_uint = 11;
pub const RETRO_ENVIRONMENT_SET_KEYBOARD_CALLBACK: c_uint = 12;
pub const RETRO_ENVIRONMENT_GET_VARIABLE: c_uint = 15;
pub const RETRO_ENVIRONMENT_SET_VARIABLES: c_uint = 16;
pub const RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE: c_uint = 17;
pub const RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME: c_uint = 18;
pub const RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY: c_uint = 31;
pub const RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION: c_uint = 52;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS: c_uint = 53;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL: c_uint = 54;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY: c_uint = 55;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2: c_uint = 67;
pub const RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL: c_uint = 68;

// ============================================================================
// Libretro C Structures
// ============================================================================

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RetroGameInfo {
    pub path: *const c_char,
    pub data: *const c_void,
    pub size: usize,
    pub meta: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RetroSystemInfo {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RetroGameGeometry {
    pub base_width: c_uint,
    pub base_height: c_uint,
    pub max_width: c_uint,
    pub max_height: c_uint,
    pub aspect_ratio: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RetroSystemTiming {
    pub fps: f64,
    pub sample_rate: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct RetroSystemAvInfo {
    pub geometry: RetroGameGeometry,
    pub timing: RetroSystemTiming,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RetroVariable {
    pub key: *const c_char,
    pub value: *const c_char,
}

// ============================================================================
// Libretro Callback Types (C-ABI)
// ============================================================================

pub type RetroEnvironmentFn = unsafe extern "C" fn(cmd: c_uint, data: *mut c_void) -> bool;
pub type RetroVideoRefreshFn =
    unsafe extern "C" fn(data: *const c_void, width: c_uint, height: c_uint, pitch: usize);
pub type RetroAudioSampleFn = unsafe extern "C" fn(left: i16, right: i16);
pub type RetroAudioSampleBatchFn = unsafe extern "C" fn(data: *const i16, frames: usize) -> usize;
pub type RetroInputPollFn = unsafe extern "C" fn();
pub type RetroInputStateFn =
    unsafe extern "C" fn(port: c_uint, device: c_uint, index: c_uint, id: c_uint) -> i16;

// ============================================================================
// Libretro Core Export Function Types
// ============================================================================

pub type RetroInitFn = unsafe extern "C" fn();
pub type RetroDeinitFn = unsafe extern "C" fn();
pub type RetroApiVersionFn = unsafe extern "C" fn() -> c_uint;
pub type RetroGetSystemInfoFn = unsafe extern "C" fn(info: *mut RetroSystemInfo);
pub type RetroGetSystemAvInfoFn = unsafe extern "C" fn(info: *mut RetroSystemAvInfo);
pub type RetroSetEnvironmentFn = unsafe extern "C" fn(cb: RetroEnvironmentFn);
pub type RetroSetVideoRefreshFn = unsafe extern "C" fn(cb: RetroVideoRefreshFn);
pub type RetroSetAudioSampleFn = unsafe extern "C" fn(cb: RetroAudioSampleFn);
pub type RetroSetAudioSampleBatchFn = unsafe extern "C" fn(cb: RetroAudioSampleBatchFn);
pub type RetroSetInputPollFn = unsafe extern "C" fn(cb: RetroInputPollFn);
pub type RetroSetInputStateFn = unsafe extern "C" fn(cb: RetroInputStateFn);
pub type RetroLoadGameFn = unsafe extern "C" fn(game: *const RetroGameInfo) -> bool;
pub type RetroUnloadGameFn = unsafe extern "C" fn();
pub type RetroRunFn = unsafe extern "C" fn();
pub type RetroResetFn = unsafe extern "C" fn();

// ============================================================================
// Global Thread-Safe Bridge State
// ============================================================================

#[derive(Debug)]
pub struct BridgeState {
    pub pixel_format: c_uint,
    pub framebuffer: Vec<u8>, // RGBA 32-bit (width * height * 4)
    pub width: u32,
    pub height: u32,
    pub key_states: [bool; 16],
    pub audio_samples: Vec<f32>,
    pub audio_producer: Option<crate::audio::AudioProducer>,
    pub system_dir: Option<std::ffi::CString>,
    pub save_dir: Option<std::ffi::CString>,
}

static BRIDGE_STATE: Mutex<Option<BridgeState>> = Mutex::new(None);

// ============================================================================
// Pixel Format Conversion Routines
// ============================================================================

pub fn convert_rgb565_to_rgba(src: &[u8], width: usize, height: usize, pitch: usize, dst: &mut [u8]) {
    for y in 0..height {
        let row_offset = y * pitch;
        let dst_row_offset = y * width * 4;
        for x in 0..width {
            let src_idx = row_offset + x * 2;
            if src_idx + 1 >= src.len() {
                break;
            }
            let pixel = (src[src_idx] as u16) | ((src[src_idx + 1] as u16) << 8);

            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;

            let r = (r5 << 3) | (r5 >> 2);
            let g = (g6 << 2) | (g6 >> 4);
            let b = (b5 << 3) | (b5 >> 2);

            let dst_idx = dst_row_offset + x * 4;
            if dst_idx + 3 < dst.len() {
                dst[dst_idx] = r;
                dst[dst_idx + 1] = g;
                dst[dst_idx + 2] = b;
                dst[dst_idx + 3] = 255;
            }
        }
    }
}

pub fn convert_xrgb8888_to_rgba(src: &[u8], width: usize, height: usize, pitch: usize, dst: &mut [u8]) {
    for y in 0..height {
        let row_offset = y * pitch;
        let dst_row_offset = y * width * 4;
        for x in 0..width {
            let src_idx = row_offset + x * 4;
            if src_idx + 3 >= src.len() {
                break;
            }
            // Little-endian XRGB8888: Byte 0=B, Byte 1=G, Byte 2=R, Byte 3=X
            let b = src[src_idx];
            let g = src[src_idx + 1];
            let r = src[src_idx + 2];

            let dst_idx = dst_row_offset + x * 4;
            if dst_idx + 3 < dst.len() {
                dst[dst_idx] = r;
                dst[dst_idx + 1] = g;
                dst[dst_idx + 2] = b;
                dst[dst_idx + 3] = 255;
            }
        }
    }
}

pub fn convert_0rgb1555_to_rgba(src: &[u8], width: usize, height: usize, pitch: usize, dst: &mut [u8]) {
    for y in 0..height {
        let row_offset = y * pitch;
        let dst_row_offset = y * width * 4;
        for x in 0..width {
            let src_idx = row_offset + x * 2;
            if src_idx + 1 >= src.len() {
                break;
            }
            let pixel = (src[src_idx] as u16) | ((src[src_idx + 1] as u16) << 8);

            let r5 = ((pixel >> 10) & 0x1F) as u8;
            let g5 = ((pixel >> 5) & 0x1F) as u8;
            let b5 = (pixel & 0x1F) as u8;

            let r = (r5 << 3) | (r5 >> 2);
            let g = (g5 << 3) | (g5 >> 2);
            let b = (b5 << 3) | (b5 >> 2);

            let dst_idx = dst_row_offset + x * 4;
            if dst_idx + 3 < dst.len() {
                dst[dst_idx] = r;
                dst[dst_idx + 1] = g;
                dst[dst_idx + 2] = b;
                dst[dst_idx + 3] = 255;
            }
        }
    }
}

// ============================================================================
// Libretro C Callbacks
// ============================================================================

unsafe extern "C" fn retro_environment_cb(cmd: c_uint, data: *mut c_void) -> bool {
    match cmd {
        RETRO_ENVIRONMENT_SET_PIXEL_FORMAT => {
            if data.is_null() {
                return false;
            }
            let format = *(data as *const c_uint);
            info!("Libretro Environment: SET_PIXEL_FORMAT = {}", format);
            if format == RETRO_PIXEL_FORMAT_XRGB8888
                || format == RETRO_PIXEL_FORMAT_RGB565
                || format == RETRO_PIXEL_FORMAT_0RGB1555
            {
                if let Ok(mut lock) = BRIDGE_STATE.lock() {
                    if let Some(ref mut state) = *lock {
                        state.pixel_format = format;
                    }
                }
                true
            } else {
                warn!("Libretro Environment: Unsupported pixel format requested: {}", format);
                false
            }
        }

        RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            if !data.is_null() {
                *(data as *mut bool) = true;
                true
            } else {
                false
            }
        }

        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY | RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY => {
            if !data.is_null() {
                if let Ok(lock) = BRIDGE_STATE.lock() {
                    if let Some(ref state) = *lock {
                        let dir_ptr = if cmd == RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY {
                            state.system_dir.as_ref().map(|s| s.as_ptr())
                        } else {
                            state.save_dir.as_ref().map(|s| s.as_ptr())
                        };

                        if let Some(ptr) = dir_ptr {
                            *(data as *mut *const c_char) = ptr;
                            return true;
                        }
                    }
                }
            }
            false
        }

        RETRO_ENVIRONMENT_GET_VARIABLE => {
            if !data.is_null() {
                let var = data as *mut RetroVariable;
                if !(*var).key.is_null() {
                    let key = CStr::from_ptr((*var).key).to_string_lossy();
                    debug!("Libretro Environment: GET_VARIABLE query for '{}'", key);
                }
                (*var).value = std::ptr::null();
            }
            false
        }

        RETRO_ENVIRONMENT_SET_VARIABLES
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_DISPLAY
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL => true,

        RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            if !data.is_null() {
                *(data as *mut c_uint) = 2;
                true
            } else {
                false
            }
        }

        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS | RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME => true,

        _ => {
            debug!("Libretro Environment: Unhandled cmd {}", cmd);
            false
        }
    }
}

unsafe extern "C" fn retro_video_refresh_cb(
    data: *const c_void,
    width: c_uint,
    height: c_uint,
    pitch: usize,
) {
    if data.is_null() || width == 0 || height == 0 {
        return;
    }

    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            state.width = width;
            state.height = height;
            let required_size = (width as usize) * (height as usize) * 4;
            if state.framebuffer.len() != required_size {
                state.framebuffer.resize(required_size, 0);
            }

            let src_slice_len = pitch * (height as usize);
            let src_slice = std::slice::from_raw_parts(data as *const u8, src_slice_len);

            match state.pixel_format {
                RETRO_PIXEL_FORMAT_RGB565 => {
                    convert_rgb565_to_rgba(
                        src_slice,
                        width as usize,
                        height as usize,
                        pitch,
                        &mut state.framebuffer,
                    );
                }
                RETRO_PIXEL_FORMAT_XRGB8888 => {
                    convert_xrgb8888_to_rgba(
                        src_slice,
                        width as usize,
                        height as usize,
                        pitch,
                        &mut state.framebuffer,
                    );
                }
                RETRO_PIXEL_FORMAT_0RGB1555 => {
                    convert_0rgb1555_to_rgba(
                        src_slice,
                        width as usize,
                        height as usize,
                        pitch,
                        &mut state.framebuffer,
                    );
                }
                other => {
                    warn!("Unknown pixel format during video refresh: {}", other);
                }
            }
        }
    }
}

unsafe extern "C" fn retro_audio_sample_cb(left: i16, right: i16) {
    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            if let Some(ref prod) = state.audio_producer {
                prod.push_i16_pair(left, right);
            } else {
                state.audio_samples.push(left as f32 / 32768.0);
                state.audio_samples.push(right as f32 / 32768.0);
            }
        }
    }
}

unsafe extern "C" fn retro_audio_sample_batch_cb(data: *const i16, frames: usize) -> usize {
    if data.is_null() || frames == 0 {
        return 0;
    }
    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            let slice = std::slice::from_raw_parts(data, frames * 2);
            if let Some(ref prod) = state.audio_producer {
                prod.push_i16_slice(slice);
            } else {
                for &s in slice {
                    state.audio_samples.push(s as f32 / 32768.0);
                }
            }
        }
    }
    frames
}

unsafe extern "C" fn retro_input_poll_cb() {
    // PixelDrive captures input via winit events in the main event loop
}

unsafe extern "C" fn retro_input_state_cb(
    port: c_uint,
    device: c_uint,
    _index: c_uint,
    id: c_uint,
) -> i16 {
    if port == 0 && (device & RETRO_DEVICE_MASK) == RETRO_DEVICE_JOYPAD {
        if let Ok(lock) = BRIDGE_STATE.lock() {
            if let Some(ref state) = *lock {
                if (id as usize) < state.key_states.len() {
                    return if state.key_states[id as usize] { 1 } else { 0 };
                }
            }
        }
    }
    0
}

// ============================================================================
// LibretroCore Implementation
// ============================================================================

pub struct LibretroCore {
    _lib: libloading::Library,
    retro_init: RetroInitFn,
    retro_deinit: RetroDeinitFn,
    retro_api_version: RetroApiVersionFn,
    retro_get_system_info: RetroGetSystemInfoFn,
    retro_get_system_av_info: RetroGetSystemAvInfoFn,
    retro_set_environment: RetroSetEnvironmentFn,
    retro_set_video_refresh: RetroSetVideoRefreshFn,
    retro_set_audio_sample: RetroSetAudioSampleFn,
    retro_set_audio_sample_batch: RetroSetAudioSampleBatchFn,
    retro_set_input_poll: RetroSetInputPollFn,
    retro_set_input_state: RetroSetInputStateFn,
    retro_load_game: RetroLoadGameFn,
    retro_unload_game: RetroUnloadGameFn,
    retro_run: RetroRunFn,
    retro_reset: RetroResetFn,

    pub library_name: String,
    pub library_version: String,
    pub av_info: RetroSystemAvInfo,
    pub is_game_loaded: bool,
    framebuffer_cache: Vec<u8>,
    width: u32,
    height: u32,
}

impl LibretroCore {
    /// Load a dynamic Libretro core library (.dylib, .so, .dll) from disk and initialize it.
    pub fn load<P: AsRef<Path>>(core_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path_ref = core_path.as_ref();
        info!("Loading Libretro core library: {}", path_ref.display());

        let lib = unsafe { libloading::Library::new(path_ref)? };

        let retro_init: RetroInitFn = unsafe { *lib.get(b"retro_init")? };
        let retro_deinit: RetroDeinitFn = unsafe { *lib.get(b"retro_deinit")? };
        let retro_api_version: RetroApiVersionFn = unsafe { *lib.get(b"retro_api_version")? };
        let retro_get_system_info: RetroGetSystemInfoFn =
            unsafe { *lib.get(b"retro_get_system_info")? };
        let retro_get_system_av_info: RetroGetSystemAvInfoFn =
            unsafe { *lib.get(b"retro_get_system_av_info")? };
        let retro_set_environment: RetroSetEnvironmentFn =
            unsafe { *lib.get(b"retro_set_environment")? };
        let retro_set_video_refresh: RetroSetVideoRefreshFn =
            unsafe { *lib.get(b"retro_set_video_refresh")? };
        let retro_set_audio_sample: RetroSetAudioSampleFn =
            unsafe { *lib.get(b"retro_set_audio_sample")? };
        let retro_set_audio_sample_batch: RetroSetAudioSampleBatchFn =
            unsafe { *lib.get(b"retro_set_audio_sample_batch")? };
        let retro_set_input_poll: RetroSetInputPollFn =
            unsafe { *lib.get(b"retro_set_input_poll")? };
        let retro_set_input_state: RetroSetInputStateFn =
            unsafe { *lib.get(b"retro_set_input_state")? };
        let retro_load_game: RetroLoadGameFn = unsafe { *lib.get(b"retro_load_game")? };
        let retro_unload_game: RetroUnloadGameFn = unsafe { *lib.get(b"retro_unload_game")? };
        let retro_run: RetroRunFn = unsafe { *lib.get(b"retro_run")? };
        let retro_reset: RetroResetFn = unsafe { *lib.get(b"retro_reset")? };

        let api_ver = unsafe { (retro_api_version)() };
        info!("Libretro Core API Version: {}", api_ver);
        if api_ver != RETRO_API_VERSION {
            warn!(
                "Core API version mismatch (got {}, expected {}). Attempting to proceed.",
                api_ver, RETRO_API_VERSION
            );
        }

        // Initialize global bridge state
        {
            let mut state_lock = BRIDGE_STATE.lock().map_err(|e| e.to_string())?;
            let system_dir = std::env::current_dir()
                .ok()
                .and_then(|p| p.to_str().map(|s| std::ffi::CString::new(s).ok()).flatten());
            let save_dir = system_dir.clone();

            *state_lock = Some(BridgeState {
                pixel_format: RETRO_PIXEL_FORMAT_RGB565, // Default expectation
                framebuffer: vec![0; 240 * 160 * 4],
                width: 240,
                height: 160,
                key_states: [false; 16],
                audio_samples: Vec::new(),
                audio_producer: None,
                system_dir,
                save_dir,
            });
        }

        // Connect C callbacks
        unsafe {
            (retro_set_environment)(retro_environment_cb);
            (retro_set_video_refresh)(retro_video_refresh_cb);
            (retro_set_audio_sample)(retro_audio_sample_cb);
            (retro_set_audio_sample_batch)(retro_audio_sample_batch_cb);
            (retro_set_input_poll)(retro_input_poll_cb);
            (retro_set_input_state)(retro_input_state_cb);
            (retro_init)();
        }

        let mut sys_info = RetroSystemInfo {
            library_name: std::ptr::null(),
            library_version: std::ptr::null(),
            valid_extensions: std::ptr::null(),
            need_fullpath: false,
            block_extract: false,
        };
        unsafe {
            (retro_get_system_info)(&mut sys_info);
        }

        let library_name = if !sys_info.library_name.is_null() {
            unsafe { CStr::from_ptr(sys_info.library_name).to_string_lossy().into_owned() }
        } else {
            "Unknown Libretro Core".to_string()
        };

        let library_version = if !sys_info.library_version.is_null() {
            unsafe { CStr::from_ptr(sys_info.library_version).to_string_lossy().into_owned() }
        } else {
            "0.0.0".to_string()
        };

        info!(
            "Successfully initialized Libretro Core: '{}' (v{})",
            library_name, library_version
        );

        Ok(Self {
            _lib: lib,
            retro_init,
            retro_deinit,
            retro_api_version,
            retro_get_system_info,
            retro_get_system_av_info,
            retro_set_environment,
            retro_set_video_refresh,
            retro_set_audio_sample,
            retro_set_audio_sample_batch,
            retro_set_input_poll,
            retro_set_input_state,
            retro_load_game,
            retro_unload_game,
            retro_run,
            retro_reset,

            library_name,
            library_version,
            av_info: RetroSystemAvInfo::default(),
            is_game_loaded: false,
            framebuffer_cache: vec![0; 240 * 160 * 4],
            width: 240,
            height: 160,
        })
    }

    /// Load a ROM byte buffer into the Libretro core.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) -> bool {
        if self.is_game_loaded {
            unsafe { (self.retro_unload_game)(); }
            self.is_game_loaded = false;
        }

        let game_info = RetroGameInfo {
            path: std::ptr::null(),
            data: rom_bytes.as_ptr() as *const c_void,
            size: rom_bytes.len(),
            meta: std::ptr::null(),
        };

        let ok = unsafe { (self.retro_load_game)(&game_info) };
        if ok {
            self.is_game_loaded = true;
            let mut av_info = RetroSystemAvInfo::default();
            unsafe {
                (self.retro_get_system_av_info)(&mut av_info);
            }
            self.av_info = av_info;

            self.width = if av_info.geometry.base_width > 0 {
                av_info.geometry.base_width
            } else {
                240
            };
            self.height = if av_info.geometry.base_height > 0 {
                av_info.geometry.base_height
            } else {
                160
            };

            let fb_size = (self.width * self.height * 4) as usize;
            self.framebuffer_cache.resize(fb_size, 0);

            info!(
                "Libretro Core loaded ROM successfully! Geometry: {}x{} (Aspect: {:.2}), Timing: {:.2} FPS, {:.0} Hz",
                self.width,
                self.height,
                self.av_info.geometry.aspect_ratio,
                self.av_info.timing.fps,
                self.av_info.timing.sample_rate
            );

            if self.av_info.timing.sample_rate > 0.0 {
                if let Ok(lock) = BRIDGE_STATE.lock() {
                    if let Some(ref state) = *lock {
                        if let Some(ref prod) = state.audio_producer {
                            prod.set_input_sample_rate(self.av_info.timing.sample_rate);
                        }
                    }
                }
            }
        } else {
            error!("Libretro Core failed to load ROM!");
        }

        ok
    }

    /// Advance the Libretro core simulation by 1 frame.
    pub fn step_frame(&mut self) {
        if !self.is_game_loaded {
            return;
        }

        unsafe {
            (self.retro_run)();
        }

        // Copy latest frame from bridge state to core's local cache
        if let Ok(lock) = BRIDGE_STATE.lock() {
            if let Some(ref state) = *lock {
                self.width = state.width;
                self.height = state.height;
                let required_size = (state.width * state.height * 4) as usize;
                if self.framebuffer_cache.len() != required_size {
                    self.framebuffer_cache.resize(required_size, 0);
                }
                if state.framebuffer.len() == self.framebuffer_cache.len() {
                    self.framebuffer_cache.copy_from_slice(&state.framebuffer);
                }
            }
        }
    }

    /// Returns a slice of the 32-bit RGBA framebuffer.
    pub fn framebuffer(&self) -> &[u8] {
        &self.framebuffer_cache
    }

    /// Set button press state for player 1.
    pub fn set_key_state(&mut self, button_id: u32, pressed: bool) {
        if let Ok(mut lock) = BRIDGE_STATE.lock() {
            if let Some(ref mut state) = *lock {
                if (button_id as usize) < state.key_states.len() {
                    state.key_states[button_id as usize] = pressed;
                }
            }
        }
    }

    /// Returns the active display dimensions (width, height).
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Reset the core simulation.
    pub fn reset(&mut self) {
        if self.is_game_loaded {
            unsafe {
                (self.retro_reset)();
            }
        }
    }

    /// Set or update the active audio sample producer.
    pub fn set_audio_producer(&mut self, producer: Option<crate::audio::AudioProducer>) {
        if let Some(ref prod) = producer {
            if self.av_info.timing.sample_rate > 0.0 {
                prod.set_input_sample_rate(self.av_info.timing.sample_rate);
            }
        }
        set_global_audio_producer(producer);
    }

    /// Drain queued audio samples.
    pub fn drain_audio(&mut self) -> Vec<f32> {
        if let Ok(mut lock) = BRIDGE_STATE.lock() {
            if let Some(ref mut state) = *lock {
                return std::mem::take(&mut state.audio_samples);
            }
        }
        Vec::new()
    }
}

/// Set global AudioProducer on active BridgeState.
pub fn set_global_audio_producer(producer: Option<crate::audio::AudioProducer>) {
    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            state.audio_producer = producer;
        }
    }
}

impl Drop for LibretroCore {
    fn drop(&mut self) {
        info!("Unloading Libretro Core: {}", self.library_name);
        if self.is_game_loaded {
            unsafe {
                (self.retro_unload_game)();
            }
            self.is_game_loaded = false;
        }
        unsafe {
            (self.retro_deinit)();
        }
    }
}

// ============================================================================
// Core Discovery Helpers
// ============================================================================

/// Search for available Libretro core dynamic libraries in standard directories.
pub fn find_available_core() -> Option<PathBuf> {
    let mut candidate_paths = Vec::new();

    // 1. Search ./cores/ directory relative to current working directory
    let cwd_cores = PathBuf::from("cores");
    candidate_paths.push(cwd_cores.join("mgba_libretro.dylib"));
    candidate_paths.push(cwd_cores.join("mgba_libretro.so"));
    candidate_paths.push(cwd_cores.join("mgba_libretro.dll"));
    candidate_paths.push(cwd_cores.join("gba_libretro.dylib"));
    candidate_paths.push(cwd_cores.join("gba_libretro.so"));
    candidate_paths.push(cwd_cores.join("gba_libretro.dll"));
    candidate_paths.push(cwd_cores.join("vbam_libretro.dylib"));
    candidate_paths.push(cwd_cores.join("vbam_libretro.so"));
    candidate_paths.push(cwd_cores.join("vbam_libretro.dll"));

    // 2. Search root working directory
    candidate_paths.push(PathBuf::from("mgba_libretro.dylib"));
    candidate_paths.push(PathBuf::from("mgba_libretro.so"));
    candidate_paths.push(PathBuf::from("mgba_libretro.dll"));

    // 3. Search directory adjacent to current executable and workspace ancestors
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let exe_cores = exe_dir.join("cores");
            candidate_paths.push(exe_cores.join("mgba_libretro.dylib"));
            candidate_paths.push(exe_cores.join("mgba_libretro.so"));
            candidate_paths.push(exe_cores.join("mgba_libretro.dll"));
            candidate_paths.push(exe_dir.join("mgba_libretro.dylib"));
            candidate_paths.push(exe_dir.join("mgba_libretro.so"));
            candidate_paths.push(exe_dir.join("mgba_libretro.dll"));

            // If running inside target/debug/ or target/release/
            candidate_paths.push(exe_dir.join("../../cores/mgba_libretro.dylib"));
            candidate_paths.push(exe_dir.join("../../cores/mgba_libretro.so"));
            candidate_paths.push(exe_dir.join("../../cores/mgba_libretro.dll"));
        }
    }

    for path in candidate_paths {
        if path.exists() {
            info!("Found Libretro core candidate: {}", path.display());
            return Some(path);
        }
    }

    None
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_format_conversion_rgb565() {
        // Red pixel in RGB565: 0xF800 (11111 000000 00000)
        let red_565: [u8; 2] = [0x00, 0xF8];
        let mut rgba = [0u8; 4];
        convert_rgb565_to_rgba(&red_565, 1, 1, 2, &mut rgba);
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 0);   // G
        assert_eq!(rgba[2], 0);   // B
        assert_eq!(rgba[3], 255); // A

        // Green pixel in RGB565: 0x07E0 (00000 111111 00000)
        let green_565: [u8; 2] = [0xE0, 0x07];
        convert_rgb565_to_rgba(&green_565, 1, 1, 2, &mut rgba);
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[1], 255);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);

        // Blue pixel in RGB565: 0x001F (00000 000000 11111)
        let blue_565: [u8; 2] = [0x1F, 0x00];
        convert_rgb565_to_rgba(&blue_565, 1, 1, 2, &mut rgba);
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 255);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_pixel_format_conversion_xrgb8888() {
        // XRGB8888 Little-Endian in memory: [B, G, R, X]
        let yellow_xrgb: [u8; 4] = [0x00, 0xFF, 0xFF, 0x00]; // B=0, G=255, R=255
        let mut rgba = [0u8; 4];
        convert_xrgb8888_to_rgba(&yellow_xrgb, 1, 1, 4, &mut rgba);
        assert_eq!(rgba[0], 255); // R
        assert_eq!(rgba[1], 255); // G
        assert_eq!(rgba[2], 0);   // B
        assert_eq!(rgba[3], 255); // A
    }

    #[test]
    fn test_pixel_format_conversion_0rgb1555() {
        // Red pixel in 0RGB1555: 0x7C00 (0 11111 00000 00000)
        let red_1555: [u8; 2] = [0x00, 0x7C];
        let mut rgba = [0u8; 4];
        convert_0rgb1555_to_rgba(&red_1555, 1, 1, 2, &mut rgba);
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_struct_sizes() {
        assert_eq!(std::mem::size_of::<RetroGameInfo>(), 4 * std::mem::size_of::<usize>());
        assert!(std::mem::size_of::<RetroSystemInfo>() > 0);
        assert!(std::mem::size_of::<RetroSystemAvInfo>() > 0);
    }

    #[test]
    fn test_mgba_libretro_core_loading_and_execution() {
        let core_path = PathBuf::from("cores/mgba_libretro.dylib");
        if !core_path.exists() {
            println!("No mgba_libretro.dylib in ./cores/, skipping test");
            return;
        }

        let mut core = LibretroCore::load(&core_path).expect("Failed to load mgba_libretro.dylib");
        assert_eq!(core.library_name, "mGBA");

        // Connect AudioProducer ring buffer
        let (producer, cons) = crate::audio::AudioProducer::new_pair(4096 * 2);
        core.set_audio_producer(Some(producer));

        let rom_path = "/Users/ashutoshsamal/Downloads/Pokemon_Fire_Red_1[romsretro.com]/Pokemon - FireRed Version (USA, Europe).gba";
        if Path::new(rom_path).exists() {
            let rom_bytes = std::fs::read(rom_path).expect("Failed to read test ROM");
            let loaded = core.load_rom(&rom_bytes);
            assert!(loaded, "ROM should load in mGBA core");
            assert_eq!(core.dimensions(), (240, 160));

            for frame_idx in 0..180 {
                core.step_frame();
                if frame_idx % 30 == 0 {
                    let fb = core.framebuffer();
                    let non_zero = fb.iter().filter(|&&b| b != 0 && b != 255).count();
                    println!(
                        "mGBA Frame {}: fb_len={}, non_zero_bytes={}, sample=[{}, {}, {}, {}]",
                        frame_idx, fb.len(), non_zero, fb[0], fb[1], fb[2], fb[3]
                    );
                }
            }

            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
            // Verify audio samples were pushed into ring buffer
            use ringbuf::traits::Observer;
            assert!(cons.occupied_len() > 0, "mGBA should produce audio samples");
        }
    }
}

