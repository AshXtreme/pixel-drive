//! Android NativeActivity platform bindings, low-latency audio, SAF storage, haptics, and lifecycle management.

pub mod activity;
pub mod audio;
pub mod haptics;
pub mod storage;

pub use audio::*;
pub use haptics::*;
pub use storage::*;
