//! ROM Library & Thumbnail Management Subsystem
//!
//! Provides JSON persistence for recently played ROMs (`recent_roms.json`),
//! snapshot thumbnail capture, bilinear downscaling, and JPEG encoding/decoding.

pub mod db;
pub mod thumbnails;

pub use db::{LibraryManager, RomEntry, MAX_RECENT_ROMS};
pub use thumbnails::{
    capture_and_save_thumbnail, decode_jpeg, downscale_rgba, encode_jpeg,
    DEFAULT_JPEG_QUALITY, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH,
};
