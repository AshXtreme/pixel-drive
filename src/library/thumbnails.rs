//! Snapshot Thumbnail Capture & Compression Engine
//!
//! Captures active emulation framebuffers upon game exit, performs high-quality
//! bilinear downscaling to standard 320x214 aspect ratio, compresses to JPEG via
//! `jpeg-encoder`, and decodes cached thumbnails via `jpeg-decoder`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use log::info;

/// Target thumbnail pixel width (3:2 standard GBA aspect ratio).
pub const THUMBNAIL_WIDTH: u32 = 320;
/// Target thumbnail pixel height (3:2 standard GBA aspect ratio).
pub const THUMBNAIL_HEIGHT: u32 = 214;
/// Default JPEG compression quality (0..=100).
pub const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Bilinear downscaler converting source RGBA8 buffer to destination dimensions.
pub fn downscale_rgba(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> Vec<u8> {
    if src_w == 0 || src_h == 0 || dst_w == 0 || dst_h == 0 {
        return vec![0u8; (dst_w * dst_h * 4) as usize];
    }

    let mut out = vec![0u8; (dst_w * dst_h * 4) as usize];
    let x_ratio = (src_w as f32) / (dst_w as f32);
    let y_ratio = (src_h as f32) / (dst_h as f32);

    for dy in 0..dst_h {
        let src_y_f = (dy as f32 + 0.5) * y_ratio - 0.5;
        let y0 = (src_y_f.floor() as i32).clamp(0, (src_h - 1) as i32) as u32;
        let y1 = (y0 + 1).min(src_h - 1);
        let y_weight = (src_y_f - y0 as f32).clamp(0.0, 1.0);

        for dx in 0..dst_w {
            let src_x_f = (dx as f32 + 0.5) * x_ratio - 0.5;
            let x0 = (src_x_f.floor() as i32).clamp(0, (src_w - 1) as i32) as u32;
            let x1 = (x0 + 1).min(src_w - 1);
            let x_weight = (src_x_f - x0 as f32).clamp(0.0, 1.0);

            let idx00 = ((y0 * src_w + x0) * 4) as usize;
            let idx01 = ((y0 * src_w + x1) * 4) as usize;
            let idx10 = ((y1 * src_w + x0) * 4) as usize;
            let idx11 = ((y1 * src_w + x1) * 4) as usize;

            let dst_idx = ((dy * dst_w + dx) * 4) as usize;

            for c in 0..4 {
                let p00 = src.get(idx00 + c).copied().unwrap_or(0) as f32;
                let p01 = src.get(idx01 + c).copied().unwrap_or(0) as f32;
                let p10 = src.get(idx10 + c).copied().unwrap_or(0) as f32;
                let p11 = src.get(idx11 + c).copied().unwrap_or(0) as f32;

                let top = p00 * (1.0 - x_weight) + p01 * x_weight;
                let btm = p10 * (1.0 - x_weight) + p11 * x_weight;
                let val = (top * (1.0 - y_weight) + btm * y_weight).round();

                out[dst_idx + c] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }

    out
}

/// Compresses an RGBA buffer to JPEG bytes using `jpeg-encoder`.
pub fn encode_jpeg(rgba: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, String> {
    if rgba.len() != (width * height * 4) as usize {
        return Err(format!(
            "Buffer size mismatch: expected {} bytes, got {}",
            width * height * 4,
            rgba.len()
        ));
    }

    let mut output = Vec::with_capacity(32 * 1024);
    let encoder = jpeg_encoder::Encoder::new(&mut output, quality.clamp(1, 100));
    encoder
        .encode(rgba, width as u16, height as u16, jpeg_encoder::ColorType::Rgba)
        .map_err(|e| format!("JPEG encoding failed: {:?}", e))?;

    Ok(output)
}

/// Decodes JPEG bytes into an RGBA8 buffer using `jpeg-decoder`.
pub fn decode_jpeg(jpeg_bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(jpeg_bytes));
    let pixels = decoder.decode().map_err(|e| format!("JPEG decode failed: {:?}", e))?;
    let info = decoder.info().ok_or_else(|| "Missing JPEG info".to_string())?;

    let width = info.width as u32;
    let height = info.height as u32;

    // Convert pixel formats to RGBA8 if necessary
    let rgba = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => {
            let mut buf = Vec::with_capacity((width * height * 4) as usize);
            for chunk in pixels.chunks_exact(3) {
                buf.push(chunk[0]);
                buf.push(chunk[1]);
                buf.push(chunk[2]);
                buf.push(255);
            }
            buf
        }
        jpeg_decoder::PixelFormat::L8 => {
            let mut buf = Vec::with_capacity((width * height * 4) as usize);
            for &lum in &pixels {
                buf.push(lum);
                buf.push(lum);
                buf.push(lum);
                buf.push(255);
            }
            buf
        }
        _ => pixels,
    };

    Ok((rgba, width, height))
}

/// Captures an emulation frame, downscales to 320x214, encodes to JPEG, and writes to disk.
pub fn capture_and_save_thumbnail(
    frame_rgba: &[u8],
    src_w: u32,
    src_h: u32,
    out_path: &Path,
) -> io::Result<PathBuf> {
    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let downscaled = downscale_rgba(frame_rgba, src_w, src_h, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
    let jpeg_bytes = encode_jpeg(&downscaled, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, DEFAULT_JPEG_QUALITY)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    fs::write(out_path, &jpeg_bytes)?;
    info!(
        "Snapshot thumbnail successfully written: {:?} ({} bytes)",
        out_path,
        jpeg_bytes.len()
    );
    Ok(out_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downscale_and_jpeg_roundtrip() {
        let src_w = 240u32;
        let src_h = 160u32;
        let mut src = vec![0u8; (src_w * src_h * 4) as usize];

        // Fill with gradient test pattern
        for y in 0..src_h {
            for x in 0..src_w {
                let idx = ((y * src_w + x) * 4) as usize;
                src[idx] = (x % 256) as u8;
                src[idx + 1] = (y % 256) as u8;
                src[idx + 2] = ((x + y) % 256) as u8;
                src[idx + 3] = 255;
            }
        }

        let downscaled = downscale_rgba(&src, src_w, src_h, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT);
        assert_eq!(downscaled.len(), (THUMBNAIL_WIDTH * THUMBNAIL_HEIGHT * 4) as usize);

        // Encode to JPEG
        let jpeg_bytes = encode_jpeg(&downscaled, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT, 85).unwrap();
        assert!(!jpeg_bytes.is_empty());
        assert_eq!(&jpeg_bytes[0..2], &[0xFF, 0xD8]); // JPEG magic SOI

        // Decode back
        let (decoded_rgba, w, h) = decode_jpeg(&jpeg_bytes).unwrap();
        assert_eq!(w, THUMBNAIL_WIDTH);
        assert_eq!(h, THUMBNAIL_HEIGHT);
        assert_eq!(decoded_rgba.len(), (w * h * 4) as usize);

        // File I/O test
        let temp_file = std::env::temp_dir().join("pixeldrive_thumb_test.jpg");
        let saved_path = capture_and_save_thumbnail(&src, src_w, src_h, &temp_file).unwrap();
        assert!(saved_path.exists());
        let read_bytes = fs::read(&saved_path).unwrap();
        assert_eq!(read_bytes.len(), jpeg_bytes.len());
        let _ = fs::remove_file(temp_file);
    }
}
