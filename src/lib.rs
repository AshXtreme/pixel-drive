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
pub use input::{InputManager, InputSource, JoypadState, TouchOverlay};
pub use render::{FilterMode, ShaderPipeline};
pub use save::SaveManager;
