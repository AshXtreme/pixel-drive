#![allow(dead_code)]

use log::{debug, error, info, warn};
use std::ffi::{c_char, c_uint, c_void, CStr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

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

// Memory IDs
pub const RETRO_MEMORY_SAVE_RAM: c_uint = 0;
pub const RETRO_MEMORY_RTC: c_uint = 1;
pub const RETRO_MEMORY_SYSTEM_RAM: c_uint = 2;
pub const RETRO_MEMORY_VIDEO_RAM: c_uint = 3;

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
#[derive(Debug, Clone, Copy, Default)]
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
pub type RetroGetMemoryDataFn = unsafe extern "C" fn(id: c_uint) -> *mut c_void;
pub type RetroGetMemorySizeFn = unsafe extern "C" fn(id: c_uint) -> usize;
pub type RetroSerializeSizeFn = unsafe extern "C" fn() -> usize;
pub type RetroSerializeFn = unsafe extern "C" fn(data: *mut c_void, size: usize) -> bool;
pub type RetroUnserializeFn = unsafe extern "C" fn(data: *const c_void, size: usize) -> bool;

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
static LIBRETRO_LOCK: parking_lot::ReentrantMutex<()> = parking_lot::ReentrantMutex::new(());

/// Acquire process-wide Libretro execution lock
pub fn lock() -> parking_lot::ReentrantMutexGuard<'static, ()> {
    LIBRETRO_LOCK.lock()
}

// ============================================================================
// Pixel Format Conversion Routines
// ============================================================================

pub fn convert_rgb565_to_rgba(
    src: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    dst: &mut [u8],
) {
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

pub fn convert_xrgb8888_to_rgba(
    src: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    dst: &mut [u8],
) {
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

pub fn convert_0rgb1555_to_rgba(
    src: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    dst: &mut [u8],
) {
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

static SYSTEM_DIR_CSTRING: Mutex<Option<std::ffi::CString>> = Mutex::new(None);
static SAVE_DIR_CSTRING: Mutex<Option<std::ffi::CString>> = Mutex::new(None);
static ACTIVE_ROM_PATH: Mutex<Option<std::ffi::CString>> = Mutex::new(None);
static LOADED_ROM_DATA: Mutex<Vec<u8>> = Mutex::new(Vec::new());

// Static fallback C-strings for core options
static COLOR_CORRECTION_DEFAULT: &[u8] = b"OFF\0";
static INTERFRAME_DEFAULT: &[u8] = b"OFF\0";
static SOLAR_SENSOR_DEFAULT: &[u8] = b"0\0";
static FRAMESKIP_DEFAULT: &[u8] = b"0\0";
static BIOS_DEFAULT: &[u8] = b"OFF\0";
static SKIP_BIOS_DEFAULT: &[u8] = b"OFF\0";
static IDLE_LOOP_DEFAULT: &[u8] = b"OFF\0";
static ALLOW_OPPOSING_DEFAULT: &[u8] = b"OFF\0";

/// Set or update the global system and save directories for Libretro cores.
pub fn set_directories<P: AsRef<Path>>(system_path: P, save_path: P) {
    let sys_p = system_path.as_ref();
    let save_p = save_path.as_ref();
    let _ = std::fs::create_dir_all(sys_p);
    let _ = std::fs::create_dir_all(save_p);

    let sys_c = sys_p.to_str().and_then(|s| std::ffi::CString::new(s).ok());
    let save_c = save_p.to_str().and_then(|s| std::ffi::CString::new(s).ok());

    if let Ok(mut lock) = SYSTEM_DIR_CSTRING.lock() {
        *lock = sys_c.clone();
    }
    if let Ok(mut lock) = SAVE_DIR_CSTRING.lock() {
        *lock = save_c.clone();
    }

    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            state.system_dir = sys_c;
            state.save_dir = save_c;
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
                warn!(
                    "Libretro Environment: Unsupported pixel format requested: {}",
                    format
                );
                false
            }
        }

        // GET_CAN_DUPE (CMD 3)
        RETRO_ENVIRONMENT_GET_CAN_DUPE => {
            if !data.is_null() {
                *(data as *mut bool) = true;
                true
            } else {
                false
            }
        }

        // GET_SYSTEM_DIRECTORY (9), GET_SAVE_DIRECTORY (31), GET_CORE_ASSETS_DIRECTORY (30), GET_LIBRETRO_PATH (19)
        RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY
        | RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY
        | 30
        | 19 => {
            if !data.is_null() {
                let out_ptr = data as *mut *const c_char;
                if let Ok(lock) = BRIDGE_STATE.lock() {
                    if let Some(ref state) = *lock {
                        let dir_ptr = if cmd == RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY {
                            state.save_dir.as_ref().map(|s| s.as_ptr())
                        } else {
                            state.system_dir.as_ref().map(|s| s.as_ptr())
                        };

                        if let Some(ptr) = dir_ptr {
                            *out_ptr = ptr;
                            return true;
                        }
                    }
                }

                // Fallback from static cached directories
                let static_lock = if cmd == RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY {
                    SAVE_DIR_CSTRING.lock().ok()
                } else {
                    SYSTEM_DIR_CSTRING.lock().ok()
                };

                if let Some(Some(ref cstr)) = static_lock.as_deref() {
                    *out_ptr = cstr.as_ptr();
                    return true;
                }

                *out_ptr = std::ptr::null();
            }
            false
        }

        // GET_LANGUAGE (CMD 39)
        39 => {
            if !data.is_null() {
                *(data as *mut c_uint) = 0; // RETRO_LANGUAGE_ENGLISH
                true
            } else {
                false
            }
        }

        // GET_AUDIO_VIDEO_ENABLE (CMD 35)
        35 => {
            if !data.is_null() {
                *(data as *mut std::ffi::c_int) = 1 | 2; // bit 0 = video enabled, bit 1 = audio enabled
                true
            } else {
                false
            }
        }

        RETRO_ENVIRONMENT_GET_VARIABLE => {
            if data.is_null() {
                return false;
            }
            let var = unsafe { &mut *(data as *mut RetroVariable) };
            if var.key.is_null() {
                var.value = std::ptr::null();
                return false;
            }

            let key = unsafe { CStr::from_ptr(var.key) }.to_string_lossy();
            debug!("Libretro Environment: GET_VARIABLE query for '{}'", key);

            match key.as_ref() {
                "mgba_color_correction" => {
                    var.value = COLOR_CORRECTION_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_interframe_blending" => {
                    var.value = INTERFRAME_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_solar_sensor" => {
                    var.value = SOLAR_SENSOR_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_frameskip" => {
                    var.value = FRAMESKIP_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_use_bios" => {
                    var.value = BIOS_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_skip_bios" => {
                    var.value = SKIP_BIOS_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_idle_loop_remove" => {
                    var.value = IDLE_LOOP_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                "mgba_allow_opposing_directions" => {
                    var.value = ALLOW_OPPOSING_DEFAULT.as_ptr() as *const c_char;
                    true
                }
                _ => {
                    var.value = std::ptr::null();
                    false
                }
            }
        }

        // GET_VARIABLE_UPDATE (17)
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE => {
            if !data.is_null() {
                unsafe {
                    *(data as *mut bool) = false;
                }
            }
            true
        }

        RETRO_ENVIRONMENT_SET_VARIABLES
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS
        | RETRO_ENVIRONMENT_SET_CORE_OPTIONS_INTL => true,

        RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION => {
            if !data.is_null() {
                *(data as *mut c_uint) = 0;
                true
            } else {
                false
            }
        }

        // SET_INPUT_DESCRIPTORS (11), SET_SUPPORT_NO_GAME (18), SET_PERFORMANCE_LEVEL (8), SET_MESSAGE (6)
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS
        | RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME
        | RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL
        | RETRO_ENVIRONMENT_SET_MESSAGE => true,

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
    // Under Libretro specification, data can be NULL or RETRO_HW_FRAME_BUFFER_VALID ((void*)-1)
    // for hardware contexts or frame dupes. Do not dereference if null or sentinel.
    if data.is_null()
        || data as usize == usize::MAX
        || data as usize == (u32::MAX as usize)
        || width == 0
        || height == 0
        || pitch == 0
    {
        return;
    }

    let src_slice_len = match pitch.checked_mul(height as usize) {
        Some(len) if len > 0 => len,
        _ => return,
    };

    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            state.width = width;
            state.height = height;
            let required_size = match (width as usize)
                .checked_mul(height as usize)
                .and_then(|wh| wh.checked_mul(4))
            {
                Some(sz) => sz,
                None => return,
            };

            if state.framebuffer.len() != required_size {
                state.framebuffer.resize(required_size, 0);
            }

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
    if data.is_null()
        || frames == 0
        || data as usize == usize::MAX
        || data as usize == (u32::MAX as usize)
        || !(data as usize).is_multiple_of(std::mem::align_of::<i16>())
    {
        return 0;
    }

    let sample_count = match frames.checked_mul(2) {
        Some(cnt) => cnt,
        None => return 0,
    };

    if let Ok(mut lock) = BRIDGE_STATE.lock() {
        if let Some(ref mut state) = *lock {
            let slice = std::slice::from_raw_parts(data, sample_count);
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

static LOADED_LIBS: Mutex<Option<std::collections::HashMap<PathBuf, Arc<libloading::Library>>>> =
    Mutex::new(None);

pub struct LibretroCore {
    _lib: Arc<libloading::Library>,
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
    retro_get_memory_data: RetroGetMemoryDataFn,
    retro_get_memory_size: RetroGetMemorySizeFn,
    retro_serialize_size: RetroSerializeSizeFn,
    retro_serialize: RetroSerializeFn,
    retro_unserialize: RetroUnserializeFn,

    pub library_name: String,
    pub library_version: String,
    pub av_info: RetroSystemAvInfo,
    pub is_game_loaded: bool,
    framebuffer_cache: Vec<u8>,
    width: u32,
    height: u32,
    pending_save_data: Option<Vec<u8>>,
    _game_path_cstr: Option<std::ffi::CString>,
    _pinned_rom_data: Option<Vec<u8>>,
}

impl LibretroCore {
    /// Load a dynamic Libretro core library (.dylib, .so, .dll) from disk and initialize it.
    pub fn load<P: AsRef<Path>>(core_path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let _lock = LIBRETRO_LOCK.lock();
        let path_ref = core_path.as_ref();
        info!("Loading Libretro core library: {}", path_ref.display());

        let canonical_path = path_ref
            .canonicalize()
            .unwrap_or_else(|_| path_ref.to_path_buf());
        let lib = {
            let mut map_lock = LOADED_LIBS.lock().unwrap_or_else(|e| e.into_inner());
            let map = map_lock.get_or_insert_with(std::collections::HashMap::new);
            if let Some(existing) = map.get(&canonical_path) {
                existing.clone()
            } else {
                let loaded = Arc::new(unsafe { libloading::Library::new(&canonical_path)? });
                map.insert(canonical_path, loaded.clone());
                loaded
            }
        };

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
        let retro_get_memory_data: RetroGetMemoryDataFn =
            unsafe { *lib.get(b"retro_get_memory_data")? };
        let retro_get_memory_size: RetroGetMemorySizeFn =
            unsafe { *lib.get(b"retro_get_memory_size")? };
        let retro_serialize_size: RetroSerializeSizeFn =
            unsafe { *lib.get(b"retro_serialize_size")? };
        let retro_serialize: RetroSerializeFn = unsafe { *lib.get(b"retro_serialize")? };
        let retro_unserialize: RetroUnserializeFn = unsafe { *lib.get(b"retro_unserialize")? };

        // Initialize bridge state with default system & save directory paths from storage
        let (sys_dir_c, save_dir_c) = {
            let sys_guard = SYSTEM_DIR_CSTRING.lock().unwrap_or_else(|e| e.into_inner());
            let save_guard = SAVE_DIR_CSTRING.lock().unwrap_or_else(|e| e.into_inner());
            (sys_guard.clone(), save_guard.clone())
        };

        if let Ok(mut lock) = BRIDGE_STATE.lock() {
            *lock = Some(BridgeState {
                width: 240,
                height: 160,
                framebuffer: vec![0; 240 * 160 * 4],
                pixel_format: RETRO_PIXEL_FORMAT_RGB565,
                audio_producer: None,
                audio_samples: Vec::new(),
                key_states: [false; 16],
                system_dir: sys_dir_c,
                save_dir: save_dir_c,
            });
        }

        unsafe {
            (retro_set_environment)(retro_environment_cb);
            (retro_set_video_refresh)(retro_video_refresh_cb);
            (retro_set_audio_sample)(retro_audio_sample_cb);
            (retro_set_audio_sample_batch)(retro_audio_sample_batch_cb);
            (retro_set_input_poll)(retro_input_poll_cb);
            (retro_set_input_state)(retro_input_state_cb);
            (retro_init)();
        }

        let mut sys_info = RetroSystemInfo::default();
        unsafe {
            (retro_get_system_info)(&mut sys_info);
        }

        let library_name = if !sys_info.library_name.is_null() {
            unsafe { CStr::from_ptr(sys_info.library_name) }
                .to_string_lossy()
                .into_owned()
        } else {
            "Unknown Libretro Core".to_string()
        };

        let library_version = if !sys_info.library_version.is_null() {
            unsafe { CStr::from_ptr(sys_info.library_version) }
                .to_string_lossy()
                .into_owned()
        } else {
            "0.0.0".to_string()
        };

        info!(
            "Initialized Libretro core: {} (v{})",
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
            retro_get_memory_data,
            retro_get_memory_size,
            retro_serialize_size,
            retro_serialize,
            retro_unserialize,

            library_name,
            library_version,
            av_info: RetroSystemAvInfo::default(),
            is_game_loaded: false,
            framebuffer_cache: vec![0; 240 * 160 * 4],
            width: 240,
            height: 160,
            pending_save_data: None,
            _game_path_cstr: None,
            _pinned_rom_data: None,
        })
    }

    /// Load a ROM byte buffer into the Libretro core with an optional file path hint.
    pub fn load_rom_with_path(&mut self, rom_bytes: &[u8], path_hint: Option<&str>) -> bool {
        let _lock = LIBRETRO_LOCK.lock();
        if self.is_game_loaded {
            unsafe {
                (self.retro_unload_game)();
            }
            self.is_game_loaded = false;
        }

        let mut rom_guard = LOADED_ROM_DATA.lock().unwrap_or_else(|e| e.into_inner());
        *rom_guard = rom_bytes.to_vec();

        let c_path = path_hint
            .and_then(|p| std::ffi::CString::new(p).ok())
            .unwrap_or_else(|| std::ffi::CString::new("game.gba").unwrap());

        let path_ptr = {
            let mut lock = ACTIVE_ROM_PATH.lock().unwrap_or_else(|e| e.into_inner());
            *lock = Some(c_path.clone());
            lock.as_ref().unwrap().as_ptr()
        };
        self._game_path_cstr = Some(c_path);
        self._pinned_rom_data = Some(rom_bytes.to_vec());

        let game_info = RetroGameInfo {
            path: path_ptr,
            data: rom_guard.as_ptr() as *const c_void,
            size: rom_guard.len(),
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

            if let Ok(mut lock) = BRIDGE_STATE.lock() {
                if let Some(ref mut state) = *lock {
                    state.width = self.width;
                    state.height = self.height;
                    state.framebuffer.resize(fb_size, 0);
                }
            }

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

    /// Load a ROM byte buffer into the Libretro core.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) -> bool {
        self.load_rom_with_path(rom_bytes, None)
    }

    /// Advance the Libretro core simulation by 1 frame.
    pub fn step_frame(&mut self) {
        if !self.is_game_loaded {
            return;
        }

        let _lock = LIBRETRO_LOCK.lock();

        unsafe {
            (self.retro_run)();
        }

        // Deferred SRAM injection: If save data was queued before SRAM table allocation, inject after retro_run
        if let Some(ref pending) = self.pending_save_data {
            let size = unsafe { (self.retro_get_memory_size)(RETRO_MEMORY_SAVE_RAM) };
            let ptr = unsafe { (self.retro_get_memory_data)(RETRO_MEMORY_SAVE_RAM) };
            if size > 0
                && !ptr.is_null()
                && ptr as usize != usize::MAX
                && ptr as usize != (u32::MAX as usize)
            {
                let copy_len = size.min(pending.len());
                unsafe {
                    std::ptr::copy_nonoverlapping(pending.as_ptr(), ptr as *mut u8, copy_len);
                }
                info!(
                    "Deferred loaded {} bytes into Libretro Save RAM after retro_run tick",
                    copy_len
                );
                self.pending_save_data = None;
            }
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
        let _lock = LIBRETRO_LOCK.lock();
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

    /// Extract pointer and size for specified Libretro memory ID.
    pub fn get_memory(&self, id: c_uint) -> Option<&[u8]> {
        if !self.is_game_loaded {
            return None;
        }
        let _lock = LIBRETRO_LOCK.lock();
        let size = unsafe { (self.retro_get_memory_size)(id) };
        let ptr = unsafe { (self.retro_get_memory_data)(id) };
        if size > 0
            && !ptr.is_null()
            && ptr as usize != usize::MAX
            && ptr as usize != (u32::MAX as usize)
        {
            Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, size) })
        } else {
            None
        }
    }

    /// Returns a slice of the save RAM if available.
    pub fn get_save_data(&self) -> Option<&[u8]> {
        self.get_memory(RETRO_MEMORY_SAVE_RAM)
    }

    /// Load saved data into the core's save RAM.
    pub fn load_save_data(&mut self, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }

        let _lock = LIBRETRO_LOCK.lock();

        if !self.is_game_loaded {
            info!(
                "Game not yet loaded in LibretroCore, caching {} bytes save data for post-load injection",
                data.len()
            );
            self.pending_save_data = Some(data.to_vec());
            return true;
        }

        let size = unsafe { (self.retro_get_memory_size)(RETRO_MEMORY_SAVE_RAM) };
        let ptr = unsafe { (self.retro_get_memory_data)(RETRO_MEMORY_SAVE_RAM) };

        if size > 0
            && !ptr.is_null()
            && ptr as usize != usize::MAX
            && ptr as usize != (u32::MAX as usize)
        {
            let copy_len = size.min(data.len());
            unsafe {
                std::ptr::copy_nonoverlapping(data.as_ptr(), ptr as *mut u8, copy_len);
            }
            info!(
                "Loaded {} bytes into Libretro Save RAM (total size: {})",
                copy_len, size
            );
            self.pending_save_data = None;
            true
        } else {
            info!(
                "Libretro Save RAM not yet available (ptr: {:?}, size: {}). Deferring {} bytes save data.",
                ptr, size, data.len()
            );
            self.pending_save_data = Some(data.to_vec());
            true
        }
    }

    /// Serializes full real-time core emulation state into a byte buffer.
    pub fn save_state(&self) -> Option<Vec<u8>> {
        if !self.is_game_loaded {
            return None;
        }

        let _lock = LIBRETRO_LOCK.lock();
        let size = unsafe { (self.retro_serialize_size)() };
        if size == 0 {
            warn!("Libretro core reported 0 serialization size");
            return None;
        }

        let mut buffer = vec![0u8; size];
        let ok = unsafe { (self.retro_serialize)(buffer.as_mut_ptr() as *mut c_void, size) };
        if ok {
            info!("Libretro core serialized real-time state ({} bytes)", size);
            Some(buffer)
        } else {
            warn!("Libretro core failed to serialize state");
            None
        }
    }

    /// Unserializes full real-time core emulation state from a byte buffer.
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if !self.is_game_loaded || data.is_empty() {
            return false;
        }

        let _lock = LIBRETRO_LOCK.lock();
        let ok = unsafe { (self.retro_unserialize)(data.as_ptr() as *const c_void, data.len()) };
        if ok {
            info!(
                "Libretro core successfully restored state from {} bytes",
                data.len()
            );
            true
        } else {
            warn!(
                "Libretro core failed to restore state ({} bytes)",
                data.len()
            );
            false
        }
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
        let _lock = LIBRETRO_LOCK.lock();
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

    // 0. Android internal library and core directories
    #[cfg(target_os = "android")]
    {
        for name in &[
            "libmgba_core.so",
            "libmgba_libretro_android.so",
            "mgba_libretro_android.so",
            "libvbanext_libretro_android.so",
        ] {
            // Direct native library name (resolved by Android dynamic linker)
            candidate_paths.push(PathBuf::from(name));
            // Standard Android native library installation directories
            candidate_paths.push(PathBuf::from(format!(
                "/data/data/com.pixeldrive.emulator/lib/{}",
                name
            )));
            candidate_paths.push(PathBuf::from(format!(
                "/data/user/0/com.pixeldrive.emulator/lib/{}",
                name
            )));
            // Scoped internal/external files cores directories
            candidate_paths.push(PathBuf::from(format!(
                "/data/data/com.pixeldrive.emulator/files/cores/{}",
                name
            )));
            candidate_paths.push(PathBuf::from(format!(
                "/data/user/0/com.pixeldrive.emulator/files/cores/{}",
                name
            )));
        }
    }

    // 1. Search ./cores/ directory relative to current working directory
    let cwd_cores = PathBuf::from("cores");
    for name in &[
        "mgba_libretro",
        "gba_libretro",
        "vbam_libretro",
        "libmgba_core",
        "mgba_libretro_android",
    ] {
        candidate_paths.push(cwd_cores.join(format!("{}.dylib", name)));
        candidate_paths.push(cwd_cores.join(format!("{}.so", name)));
        candidate_paths.push(cwd_cores.join(format!("{}.dll", name)));
    }

    // 2. Search root working directory
    for name in &[
        "mgba_libretro",
        "gba_libretro",
        "vbam_libretro",
        "libmgba_core",
        "mgba_libretro_android",
    ] {
        candidate_paths.push(PathBuf::from(format!("{}.dylib", name)));
        candidate_paths.push(PathBuf::from(format!("{}.so", name)));
        candidate_paths.push(PathBuf::from(format!("{}.dll", name)));
    }

    // 3. Search directory adjacent to current executable, bundle resources, and workspace ancestors
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // (a) macOS .app Bundle Resources: ../Resources/cores/
            let bundle_resources_cores = exe_dir.join("../Resources/cores");
            for name in &[
                "mgba_libretro",
                "gba_libretro",
                "vbam_libretro",
                "libmgba_core",
                "mgba_libretro_android",
            ] {
                candidate_paths.push(bundle_resources_cores.join(format!("{}.dylib", name)));
                candidate_paths.push(bundle_resources_cores.join(format!("{}.so", name)));
                candidate_paths.push(bundle_resources_cores.join(format!("{}.dll", name)));
            }

            // (b) Adjacent cores/ directory: ./cores/ next to executable
            let exe_cores = exe_dir.join("cores");
            for name in &[
                "mgba_libretro",
                "gba_libretro",
                "vbam_libretro",
                "libmgba_core",
                "mgba_libretro_android",
            ] {
                candidate_paths.push(exe_cores.join(format!("{}.dylib", name)));
                candidate_paths.push(exe_cores.join(format!("{}.so", name)));
                candidate_paths.push(exe_cores.join(format!("{}.dll", name)));
            }

            // (c) Direct executable directory
            for name in &[
                "mgba_libretro",
                "gba_libretro",
                "vbam_libretro",
                "libmgba_core",
                "mgba_libretro_android",
            ] {
                candidate_paths.push(exe_dir.join(format!("{}.dylib", name)));
                candidate_paths.push(exe_dir.join(format!("{}.so", name)));
                candidate_paths.push(exe_dir.join(format!("{}.dll", name)));
            }

            // (d) Development workspace root (if running inside target/debug/ or target/release/)
            let workspace_cores = exe_dir.join("../../cores");
            for name in &[
                "mgba_libretro",
                "gba_libretro",
                "vbam_libretro",
                "libmgba_core",
                "mgba_libretro_android",
            ] {
                candidate_paths.push(workspace_cores.join(format!("{}.dylib", name)));
                candidate_paths.push(workspace_cores.join(format!("{}.so", name)));
                candidate_paths.push(workspace_cores.join(format!("{}.dll", name)));
            }
        }
    }

    for path in candidate_paths {
        if path.exists() {
            info!("Found Libretro core candidate on disk: {}", path.display());
            return Some(path);
        }

        // On Android, also check if libloading can load by library name directly (from system/APK linker path)
        #[cfg(target_os = "android")]
        {
            if let Ok(lib) = unsafe { libloading::Library::new(&path) } {
                drop(lib);
                info!(
                    "Found Libretro core dynamically loadable by Android linker: {}",
                    path.display()
                );
                return Some(path);
            }
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
        assert_eq!(rgba[1], 0); // G
        assert_eq!(rgba[2], 0); // B
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
        assert_eq!(rgba[2], 0); // B
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
        assert_eq!(
            std::mem::size_of::<RetroGameInfo>(),
            4 * std::mem::size_of::<usize>()
        );
        assert!(std::mem::size_of::<RetroSystemInfo>() > 0);
        assert!(std::mem::size_of::<RetroSystemAvInfo>() > 0);
    }

    #[test]
    fn test_mgba_libretro_core_loading_and_execution() {
        let _test_lock = lock();
        let core_path = PathBuf::from("cores/mgba_libretro.dylib");
        if !core_path.exists() {
            log::info!("No mgba_libretro.dylib in ./cores/, skipping test");
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
                    log::debug!(
                        "mGBA Frame {}: fb_len={}, non_zero_bytes={}, sample=[{}, {}, {}, {}]",
                        frame_idx,
                        fb.len(),
                        non_zero,
                        fb[0],
                        fb[1],
                        fb[2],
                        fb[3]
                    );
                }
            }

            assert_eq!(core.framebuffer().len(), 240 * 160 * 4);
            // Verify audio samples were pushed into ring buffer
            use ringbuf::traits::Observer;
            assert!(cons.occupied_len() > 0, "mGBA should produce audio samples");

            // Verify Libretro Save RAM interface
            let save_ram = core.get_save_data();
            assert!(save_ram.is_some(), "mGBA should expose save RAM memory");
            let ram_len = save_ram.unwrap().len();
            assert!(
                ram_len >= 0x2000,
                "GBA save RAM should be at least 8KB (Flash/SRAM)"
            );

            let test_save_bytes = vec![0x77u8; 1024];
            let loaded_save = core.load_save_data(&test_save_bytes);
            assert!(loaded_save, "load_save_data should succeed");
            assert_eq!(core.get_save_data().unwrap()[0], 0x77);

            // Verify Libretro Real-Time State Serialization
            let state_snapshot = core.save_state();
            assert!(
                state_snapshot.is_some(),
                "mGBA should support state serialization"
            );
            let state_bytes = state_snapshot.unwrap();
            assert!(
                !state_bytes.is_empty(),
                "State snapshot should have non-zero size"
            );

            let state_restored = core.load_state(&state_bytes);
            assert!(
                state_restored,
                "mGBA should successfully restore state snapshot"
            );
        }
    }
}
