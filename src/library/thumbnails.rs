//! Snapshot Thumbnail Capture & Compression Engine
//!
//! Captures active emulation framebuffers upon game exit, applies crisp integer
//! nearest-neighbor scaling (2x for GBA 480x320, 3x for GBC 480x432), encodes to
//! lossless PNG via the `png` crate (with legacy JPEG fallback support), and
//! decodes cached thumbnails via `decode_image`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use log::info;

/// Target thumbnail pixel width (3:2 standard GBA aspect ratio).
pub const THUMBNAIL_WIDTH: u32 = 480;
/// Target thumbnail pixel height (3:2 standard GBA aspect ratio).
pub const THUMBNAIL_HEIGHT: u32 = 320;
/// Default JPEG compression quality (0..=100) used when encoding JPEG fallback.
pub const DEFAULT_JPEG_QUALITY: u8 = 95;

/// Upscales source RGBA8 buffer using exact integer nearest-neighbor point sampling,
/// preserving 100% crisp retro pixel-art edges without blur, smudging, or color blending.
pub fn scale_nearest_neighbor(
    src: &[u8],
    src_w: u32,
    src_h: u32,
    scale: u32,
) -> (Vec<u8>, u32, u32) {
    if src_w == 0 || src_h == 0 || scale == 0 {
        return (Vec::new(), 0, 0);
    }
    let dst_w = src_w * scale;
    let dst_h = src_h * scale;
    let mut dst = vec![0u8; (dst_w * dst_h * 4) as usize];

    for sy in 0..src_h {
        let src_row = (sy * src_w * 4) as usize;
        for dy in 0..scale {
            let dst_row = (((sy * scale + dy) * dst_w) * 4) as usize;
            for sx in 0..src_w {
                let spx = &src[src_row + (sx * 4) as usize..src_row + (sx * 4 + 4) as usize];
                for dx in 0..scale {
                    let dpx = dst_row + ((sx * scale + dx) * 4) as usize;
                    dst[dpx..dpx + 4].copy_from_slice(spx);
                }
            }
        }
    }

    (dst, dst_w, dst_h)
}

/// Bilinear downscaler converting source RGBA8 buffer to destination dimensions (retained for compatibility).
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

/// Compresses an RGBA8 buffer to PNG bytes using lossless pure-Rust `png` crate.
pub fn encode_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    if rgba.len() != (width * height * 4) as usize {
        return Err(format!(
            "Buffer size mismatch: expected {} bytes, got {}",
            width * height * 4,
            rgba.len()
        ));
    }

    let mut output = Vec::with_capacity(32 * 1024);
    {
        let mut encoder = png::Encoder::new(&mut output, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_compression(png::Compression::Fast);
        let mut writer = encoder
            .write_header()
            .map_err(|e| format!("PNG header write failed: {:?}", e))?;
        writer
            .write_image_data(rgba)
            .map_err(|e| format!("PNG image data write failed: {:?}", e))?;
    }

    Ok(output)
}

/// Decodes PNG bytes into an RGBA8 buffer using `png` crate.
pub fn decode_png(png_bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes));
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("PNG read info failed: {:?}", e))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("PNG decode frame failed: {:?}", e))?;

    let width = info.width;
    let height = info.height;

    let rgba = match info.color_type {
        png::ColorType::Rgba => {
            buf.truncate((width * height * 4) as usize);
            buf
        }
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in buf.chunks_exact(3) {
                out.push(chunk[0]);
                out.push(chunk[1]);
                out.push(chunk[2]);
                out.push(255);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for &g in &buf[..((width * height) as usize)] {
                out.push(g);
                out.push(g);
                out.push(g);
                out.push(255);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity((width * height * 4) as usize);
            for chunk in buf.chunks_exact(2) {
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[0]);
                out.push(chunk[1]);
            }
            out
        }
        _ => {
            buf.truncate((width * height * 4) as usize);
            buf
        }
    };

    Ok((rgba, width, height))
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

/// Unified image decoder: detects PNG (`\x89PNG`) vs JPEG (`\xFF\xD8`) magic headers
/// and decodes into an RGBA8 buffer.
pub fn decode_image(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    if bytes.len() >= 8 && bytes[0..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        decode_png(bytes)
    } else if bytes.len() >= 3 && bytes[0..3] == [0xFF, 0xD8, 0xFF] {
        decode_jpeg(bytes)
    } else {
        decode_png(bytes).or_else(|_| decode_jpeg(bytes))
    }
}

/// Captures an emulation frame, applies crisp integer nearest-neighbor scaling
/// (2x for GBA 480x320, 3x for GBC 480x432), encodes to lossless PNG (or high-quality JPEG if requested),
/// and writes to disk.
pub fn capture_and_save_thumbnail(
    frame_rgba: &[u8],
    src_w: u32,
    src_h: u32,
    out_path: &Path,
) -> io::Result<PathBuf> {
    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let scale_factor = if src_w <= 160 {
        3 // 160x144 GBC -> 480x432 (exact 10:9)
    } else if src_w <= 240 {
        2 // 240x160 GBA -> 480x320 (exact 3:2)
    } else {
        1
    };

    let (scaled_rgba, dst_w, dst_h) = scale_nearest_neighbor(frame_rgba, src_w, src_h, scale_factor);

    let is_jpg = out_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false);

    let bytes = if is_jpg {
        encode_jpeg(&scaled_rgba, dst_w, dst_h, DEFAULT_JPEG_QUALITY)
            .map_err(io::Error::other)?
    } else {
        encode_png(&scaled_rgba, dst_w, dst_h)
            .map_err(io::Error::other)?
    };

    let file_name = out_path.file_name().and_then(|s| s.to_str()).unwrap_or("thumb");
    let temp_name = format!(".{}.tmp.{}", file_name, std::process::id());
    let temp_path = out_path.with_file_name(temp_name);

    fs::write(&temp_path, &bytes)?;

    if let Err(err) = fs::rename(&temp_path, out_path) {
        if out_path.exists() {
            let _ = fs::remove_file(out_path);
            fs::rename(&temp_path, out_path)?;
        } else {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
    }

    info!(
        "High-fidelity snapshot thumbnail successfully written: {:?} ({}x{}, {} bytes)",
        out_path,
        dst_w,
        dst_h,
        bytes.len()
    );
    Ok(out_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nearest_neighbor_scaling_and_png_roundtrip() {
        let src_w = 240u32;
        let src_h = 160u32;
        let mut src = vec![0u8; (src_w * src_h * 4) as usize];

        // Fill with distinct test pattern
        for y in 0..src_h {
            for x in 0..src_w {
                let idx = ((y * src_w + x) * 4) as usize;
                src[idx] = (x % 256) as u8;
                src[idx + 1] = (y % 256) as u8;
                src[idx + 2] = ((x + y) % 256) as u8;
                src[idx + 3] = 255;
            }
        }

        // Test 2x scaling (GBA 240x160 -> 480x320)
        let (scaled, dst_w, dst_h) = scale_nearest_neighbor(&src, src_w, src_h, 2);
        assert_eq!(dst_w, 480);
        assert_eq!(dst_h, 320);
        assert_eq!(scaled.len(), (480 * 320 * 4) as usize);

        // Verify nearest-neighbor pixel fidelity: (0,0) and (1,1) in scaled must match (0,0) in src
        assert_eq!(&scaled[0..4], &src[0..4]);
        let p11_idx = ((1 * dst_w + 1) * 4) as usize;
        assert_eq!(&scaled[p11_idx..p11_idx + 4], &src[0..4]);

        // Encode to lossless PNG
        let png_bytes = encode_png(&scaled, dst_w, dst_h).unwrap();
        assert!(!png_bytes.is_empty());
        assert_eq!(&png_bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

        // Decode with unified decode_image
        let (decoded_rgba, w, h) = decode_image(&png_bytes).unwrap();
        assert_eq!(w, dst_w);
        assert_eq!(h, dst_h);
        assert_eq!(decoded_rgba, scaled); // Lossless exact match!

        // Test file I/O with capture_and_save_thumbnail
        let temp_file = std::env::temp_dir().join("pixeldrive_thumb_test.png");
        let saved_path = capture_and_save_thumbnail(&src, src_w, src_h, &temp_file).unwrap();
        assert!(saved_path.exists());
        let read_bytes = fs::read(&saved_path).unwrap();
        assert_eq!(&read_bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        let _ = fs::remove_file(temp_file);
    }

    #[test]
    fn test_legacy_jpeg_backward_compatibility() {
        let src_w = 160u32;
        let src_h = 144u32;
        let src = vec![128u8; (src_w * src_h * 4) as usize];

        let jpeg_bytes = encode_jpeg(&src, src_w, src_h, 95).unwrap();
        assert_eq!(&jpeg_bytes[0..3], &[0xFF, 0xD8, 0xFF]);

        let (decoded_rgba, w, h) = decode_image(&jpeg_bytes).unwrap();
        assert_eq!(w, src_w);
        assert_eq!(h, src_h);
        assert_eq!(decoded_rgba.len(), (src_w * src_h * 4) as usize);
    }
}
