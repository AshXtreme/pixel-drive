//! ROM Library & Thumbnail Management Subsystem
//!
//! Provides JSON persistence for recently played ROMs (`recent_roms.json`),
//! high-fidelity snapshot thumbnail capture, crisp integer scaling, lossless PNG
//! encoding/decoding, and legacy JPEG compatibility.

pub mod db;
pub mod thumbnails;

pub use db::{LibraryManager, RomEntry, MAX_RECENT_ROMS};
pub use thumbnails::{
    capture_and_save_thumbnail, decode_image, decode_jpeg, decode_png, downscale_rgba,
    encode_jpeg, encode_png, scale_nearest_neighbor, DEFAULT_JPEG_QUALITY, THUMBNAIL_HEIGHT,
    THUMBNAIL_WIDTH,
};
