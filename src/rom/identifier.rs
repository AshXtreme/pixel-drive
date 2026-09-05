//! ROM Identification & Checksum Helper
//!
//! Computes CRC32 checksums and extracts canonical title and game codes
//! from GBA and GB/GBC cartridge headers.

use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

/// Canonical metadata identifying a loaded ROM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomIdentifier {
    pub crc32: u32,
    pub title: String,
    pub game_code: String, // e.g. "BPRE" for Pokémon FireRed
}

impl RomIdentifier {
    /// Formats the CRC32 checksum as an uppercase 8-character hex string.
    pub fn crc32_hex(&self) -> String {
        format!("{:08X}", self.crc32)
    }

    /// Formats a clean display name combining title and game code.
    pub fn display_name(&self) -> String {
        if self.game_code.is_empty() || self.game_code == "NONE" || self.game_code == "RAW" {
            self.title.clone()
        } else {
            format!("{} [{}]", self.title, self.game_code)
        }
    }
}

/// Identifies a ROM buffer by computing its CRC32 and extracting internal header metadata.
pub fn identify_rom(rom_bytes: &[u8]) -> RomIdentifier {
    let mut hasher = Hasher::new();
    hasher.update(rom_bytes);
    let crc32 = hasher.finalize();

    // 1. Inspect GBA Cartridge Header (0x00..0xC0)
    if rom_bytes.len() >= 0xC0 {
        // Nintendo GBA magic byte at offset 0xB2 must be 0x96
        if rom_bytes[0x0B2] == 0x96 {
            let title = clean_ascii(&rom_bytes[0xA0..0xAC]);
            let game_code = clean_ascii(&rom_bytes[0xAC..0xB0]);

            return RomIdentifier {
                crc32,
                title: if title.is_empty() { "UNKNOWN_GBA".to_string() } else { title },
                game_code: if game_code.is_empty() { "GBA".to_string() } else { game_code },
            };
        }
    }

    // 2. Inspect GB / GBC Cartridge Header (0x0100..0x0150)
    if rom_bytes.len() >= 0x0150 {
        let cgb_flag = rom_bytes[0x0143];
        let is_cgb = cgb_flag == 0x80 || cgb_flag == 0xC0;

        let (title_bytes, code_bytes) = if is_cgb {
            // In CGB mode, title is up to 11 chars (0x0134..0x013F), manufacturer/game code is 4 chars (0x013F..0x0143)
            (&rom_bytes[0x0134..0x013F], &rom_bytes[0x013F..0x0143])
        } else {
            // In DMG mode, title is up to 16 chars (0x0134..0x0144)
            (&rom_bytes[0x0134..0x0144], &[][..])
        };

        let title = clean_ascii(title_bytes);
        let game_code = clean_ascii(code_bytes);

        if !title.is_empty() {
            return RomIdentifier {
                crc32,
                title,
                game_code: if game_code.is_empty() {
                    if is_cgb { "GBC".to_string() } else { "DMG".to_string() }
                } else {
                    game_code
                },
            };
        }
    }

    // Fallback for homebrew or raw ROMs
    RomIdentifier {
        crc32,
        title: format!("ROM_{:08X}", crc32),
        game_code: "RAW".to_string(),
    }
}

/// Helper to extract clean printable ASCII string from byte slices.
fn clean_ascii(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len());
    for &b in bytes {
        if b == 0 {
            break;
        }
        if b.is_ascii_graphic() || b == b' ' {
            s.push(b as char);
        }
    }
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identify_gba_rom() {
        let mut rom = vec![0u8; 0x200];
        rom[0x0B2] = 0x96; // GBA magic
        rom[0xA0..0xAC].copy_from_slice(b"POKEMON FIRE");
        rom[0xAC..0xB0].copy_from_slice(b"BPRE");

        let id = identify_rom(&rom);
        assert_eq!(id.title, "POKEMON FIRE");
        assert_eq!(id.game_code, "BPRE");
        assert_eq!(id.crc32_hex().len(), 8);
        assert_eq!(id.display_name(), "POKEMON FIRE [BPRE]");
    }

    #[test]
    fn test_identify_gbc_rom() {
        let mut rom = vec![0u8; 0x200];
        rom[0x0143] = 0x80; // CGB flag
        rom[0x0134..0x013F].copy_from_slice(b"POKEMON_GLD");
        rom[0x013F..0x0143].copy_from_slice(b"AAUE");

        let id = identify_rom(&rom);
        assert_eq!(id.title, "POKEMON_GLD");
        assert_eq!(id.game_code, "AAUE");
        assert_eq!(id.crc32_hex().len(), 8);
    }

    #[test]
    fn test_identify_dmg_rom() {
        let mut rom = vec![0u8; 0x200];
        rom[0x0143] = 0x00; // DMG
        rom[0x0134..0x0144].copy_from_slice(b"TETRIS\0\0\0\0\0\0\0\0\0\0");

        let id = identify_rom(&rom);
        assert_eq!(id.title, "TETRIS");
        assert_eq!(id.game_code, "DMG");
    }
}
