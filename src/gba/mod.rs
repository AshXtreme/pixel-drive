#![allow(dead_code)]

pub mod mmu;

use crate::core::{Button, EmulatorCore};
use log::info;
pub use mmu::GbaMemoryBus;
use std::path::Path;

pub const GBA_WIDTH: u32 = 240;
pub const GBA_HEIGHT: u32 = 160;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbaHeader {
    pub title: String,
    pub game_code: String,
    pub maker_code: String,
    pub version: u8,
    pub is_valid: bool,
}

impl GbaHeader {
    /// Parse and validate standard 32-bit GBA ROM header structure (192+ bytes)
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 0xC0 {
            return None;
        }

        // Nintendo GBA Header magic byte at offset 0xB2 must be 0x96
        if bytes[0x0B2] != 0x96 {
            return None;
        }

        let title = String::from_utf8_lossy(&bytes[0xA0..0xAC])
            .trim_matches('\0')
            .trim()
            .to_string();

        let game_code = String::from_utf8_lossy(&bytes[0xAC..0xB0])
            .trim_matches('\0')
            .trim()
            .to_string();

        let maker_code = String::from_utf8_lossy(&bytes[0xB0..0xB2])
            .trim_matches('\0')
            .trim()
            .to_string();

        let version = bytes[0xBC];

        Some(Self {
            title,
            game_code,
            maker_code,
            version,
            is_valid: true,
        })
    }
}

pub struct GbaCore {
    pub mmu: GbaMemoryBus,
    framebuffer: Vec<u8>,
    pub header: Option<GbaHeader>,
}

impl Default for GbaCore {
    fn default() -> Self {
        Self::new()
    }
}

impl GbaCore {
    pub fn new() -> Self {
        let size = (GBA_WIDTH * GBA_HEIGHT * 4) as usize;
        Self {
            mmu: GbaMemoryBus::new(),
            framebuffer: vec![0; size],
            header: None,
        }
    }

    /// Load raw ROM byte buffer into GBA MMU memory space.
    pub fn load_rom(&mut self, rom_bytes: &[u8]) {
        self.header = GbaHeader::parse(rom_bytes);
        self.mmu.load_rom(rom_bytes);
        if let Some(ref header) = self.header {
            info!(
                "Loaded GBA ROM into MMU: '{}' [GameCode: {}, Maker: {}] ({} bytes)",
                header.title,
                header.game_code,
                header.maker_code,
                rom_bytes.len()
            );
        } else {
            info!("Loaded raw GBA ROM into MMU ({} bytes)", rom_bytes.len());
        }
    }

    /// Load a `.gba` or compressed `.zip` GBA ROM file from disk into memory, verifying header signature.
    pub fn load_rom_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<GbaHeader> {
        let path_ref = path.as_ref();
        info!("Loading GBA ROM file: {}", path_ref.display());

        let bytes = if path_ref
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("zip"))
            .unwrap_or(false)
        {
            info!("Extracting GBA ROM from ZIP archive: {}", path_ref.display());
            let file = std::fs::File::open(path_ref)?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut rom_bytes = None;

            for i in 0..archive.len() {
                let mut file_entry = archive.by_index(i)?;
                let name = file_entry.name().to_lowercase();
                if name.ends_with(".gba") {
                    info!("Found GBA ROM entry in ZIP: {}", file_entry.name());
                    let mut buffer = Vec::new();
                    std::io::Read::read_to_end(&mut file_entry, &mut buffer)?;
                    rom_bytes = Some(buffer);
                    break;
                }
            }

            match rom_bytes {
                Some(b) => b,
                None => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "No valid .gba ROM file found inside ZIP archive",
                    ));
                }
            }
        } else {
            std::fs::read(path_ref)?
        };

        let header = GbaHeader::parse(&bytes).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid GBA header signature (missing magic byte 0x96 or file too small)",
            )
        })?;

        self.load_rom(&bytes);
        Ok(header)
    }
}

impl EmulatorCore for GbaCore {
    fn step_frame(&mut self) {
        // Placeholder step logic for GBA Core
    }

    fn framebuffer(&self) -> &[u8] {
        &self.framebuffer
    }

    fn display_dimensions(&self) -> (u32, u32) {
        (GBA_WIDTH, GBA_HEIGHT)
    }

    fn handle_input(&mut self, button: Button, pressed: bool) {
        info!("GBA Input: {:?} -> {}", button, if pressed { "Pressed" } else { "Released" });
    }

    fn audio_buffer(&mut self) -> Vec<f32> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn create_dummy_gba_rom(title: &str, game_code: &str) -> Vec<u8> {
        let mut rom = vec![0u8; 0x1000]; // 4KB dummy ROM
        // 0x000 - ARM jump instruction dummy
        rom[0..4].copy_from_slice(&[0x2E, 0x00, 0x00, 0xEA]);
        // 0xB2 - GBA Magic byte
        rom[0x0B2] = 0x96;

        // Title at 0xA0..0xAC
        let title_bytes = title.as_bytes();
        let t_len = title_bytes.len().min(12);
        rom[0xA0..0xA0 + t_len].copy_from_slice(&title_bytes[..t_len]);

        // Game code at 0xAC..0xB0
        let code_bytes = game_code.as_bytes();
        let c_len = code_bytes.len().min(4);
        rom[0xAC..0xAC + c_len].copy_from_slice(&code_bytes[..c_len]);

        rom
    }

    #[test]
    fn test_gba_header_parse() {
        let rom = create_dummy_gba_rom("POKEMON FIRE", "BPRE");
        let header = GbaHeader::parse(&rom).expect("Header should parse");
        assert_eq!(header.title, "POKEMON FIRE");
        assert_eq!(header.game_code, "BPRE");
        assert!(header.is_valid);
    }

    #[test]
    fn test_invalid_gba_header() {
        let rom = vec![0u8; 0x1000]; // Magic byte missing
        assert!(GbaHeader::parse(&rom).is_none());
    }

    #[test]
    fn test_load_rom_file() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let rom_path = temp_dir.join("test_game.gba");
        let rom_bytes = create_dummy_gba_rom("ZELDA MINISH", "BZME");
        std::fs::write(&rom_path, &rom_bytes)?;

        let mut core = GbaCore::new();
        let header = core.load_rom_file(&rom_path)?;
        assert_eq!(header.title, "ZELDA MINISH");
        assert_eq!(header.game_code, "BZME");
        assert_eq!(core.mmu.read_u32(0x08000000), 0xEA00002E);

        let _ = std::fs::remove_file(rom_path);
        Ok(())
    }

    #[test]
    fn test_load_zip_gba() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = std::env::temp_dir();
        let zip_path = temp_dir.join("test_gba.zip");
        let rom_bytes = create_dummy_gba_rom("METROID FUS", "AMFE");

        {
            let file = std::fs::File::create(&zip_path)?;
            let mut zip = zip::ZipWriter::new(file);
            zip.start_file("game.gba", SimpleFileOptions::default())?;
            zip.write_all(&rom_bytes)?;
            zip.finish()?;
        }

        let mut core = GbaCore::new();
        let header = core.load_rom_file(&zip_path)?;
        assert_eq!(header.title, "METROID FUS");
        assert_eq!(header.game_code, "AMFE");

        let _ = std::fs::remove_file(zip_path);
        Ok(())
    }
}
