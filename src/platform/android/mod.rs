//! Android NativeActivity platform bindings, low-latency audio, SAF storage, and lifecycle management.

pub mod activity;
pub mod audio;
pub mod storage;

pub use audio::*;
pub use storage::*;
