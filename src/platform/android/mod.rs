#[cfg(target_os = "android")]
pub mod activity;
#[cfg(target_os = "android")]
pub mod audio;
#[cfg(target_os = "android")]
pub mod haptics;
pub mod storage;

#[cfg(target_os = "android")]
pub use audio::*;
#[cfg(target_os = "android")]
pub use haptics::*;
pub use storage::*;

