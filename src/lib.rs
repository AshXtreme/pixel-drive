//! PixelDrive Core Library
//!
//! A unified, high-performance Game Boy (GB/GBC) and Game Boy Advance (GBA)
//! emulator in Rust targeting macOS, Windows, and Android.

pub mod audio;
pub mod core;
pub mod error;
pub mod gba;
pub mod gbc;
pub mod input;
pub mod platform;
pub mod render;
pub mod save;
pub mod ui;

pub use audio::{AudioPlayer, AudioProducer};
pub use core::{Button, EmulatorCore};
pub use error::PixelDriveError;
pub use gba::GbaCore;
pub use gbc::GbcCore;
pub use input::{
    touch_bits, ButtonShape, ChordHitbox, InputManager, InputSource, JoypadState, TouchAction,
    TouchInputManager, TouchOverlay, TouchOverlayPreset, TouchPhase, TouchPoint, TouchRect,
    VirtualButton, VirtualButtonId, VirtualDPad,
};
pub use platform::{DesktopStorage, PlatformAudio, PlatformStorage};
#[cfg(target_os = "android")]
pub use platform::{AndroidAudioPlayer, AndroidStorage};
pub use render::{FilterMode, ShaderPipeline, TouchOverlayRenderer, TouchOverlayUniforms};
pub use save::SaveManager;

