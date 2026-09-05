//! Code Decoders & Injection Engine for GBA and GBC.
//!
//! Provides parsing, validation, decryption, and direct memory poking for
//! GameShark, Action Replay, and Raw memory patches on GBA (Libretro + MMU)
//! and GBC (native core).

use super::{CheatType, GameCheats};
use crate::rom::{identify_rom, RomIdentifier};
use log::{debug, info, warn};
use std::path::{Path, PathBuf};

/// Decoded GBC memory patch (target 16-bit address and 8-bit value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GbcPatch {
    pub address: u16,
    pub value: u8,
}

/// Decoded GBA memory patch (target 32-bit address, value, and byte-width: 1, 2, or 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GbaPatch {
    pub address: u32,
    pub value: u32,
    pub width: u8, // 1 (u8), 2 (u16), 4 (u32)
}

/// Verification result for user cheat code input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    Valid {
        detected_type: CheatType,
        lines_count: usize,
    },
    Invalid(String),
}

/// Per-game cheat execution engine handling active memory injection and persistence.
#[derive(Debug, Clone, Default)]
pub struct CheatEngine {
    pub cheats: GameCheats,
    pub rom_identifier: Option<RomIdentifier>,
    pub file_path: Option<PathBuf>,
    pub active_gbc_patches: Vec<GbcPatch>,
    pub active_gba_patches: Vec<GbaPatch>,
    pub is_gba: bool,
    pub libretro_initialized: bool,
}

impl CheatEngine {
    /// Creates a new uninitialized `CheatEngine`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads cheats for a given ROM buffer from the storage directory.
    pub fn load_for_rom(&mut self, rom_bytes: &[u8], storage_dir: &Path) {
        let identifier = identify_rom(rom_bytes);
        let is_gba = rom_bytes.len() >= 0xC0 && rom_bytes[0xB2] == 0x96;

        let cheats_dir = storage_dir.join("cheats");
        let _ = std::fs::create_dir_all(&cheats_dir);
        let file_path = cheats_dir.join(format!("{}.cht", identifier.crc32_hex()));

        info!(
            "CheatEngine: Initializing for '{}' [CRC32: {:08X}, File: {}]",
            identifier.title,
            identifier.crc32,
            file_path.display()
        );

        let cheats = match GameCheats::load_from_path(&file_path, identifier.crc32, &identifier.title) {
            Ok(c) => c,
            Err(err) => {
                warn!("CheatEngine: Could not read {:?}: {}", file_path, err);
                GameCheats::new(identifier.crc32, identifier.title.clone())
            }
        };

        self.cheats = cheats;
        self.rom_identifier = Some(identifier);
        self.file_path = Some(file_path);
        self.is_gba = is_gba;
        self.libretro_initialized = false;
        self.recompile_patches();
    }

    /// Saves the current cheat list to its designated `.cht` file on disk.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            self.cheats.save_to_path(path)
        } else {
            Ok(())
        }
    }

    /// Recompiles and updates active memory patches from all enabled cheat entries.
    pub fn recompile_patches(&mut self) {
        let mut gbc_patches = Vec::new();
        let mut gba_patches = Vec::new();

        for entry in self.cheats.entries.iter().filter(|e| e.enabled) {
            if self.is_gba {
                let patches = decode_gba_code(&entry.code, entry.cheat_type);
                gba_patches.extend(patches);
            } else {
                let patches = decode_gbc_code(&entry.code);
                gbc_patches.extend(patches);
            }
        }

        self.active_gbc_patches = gbc_patches;
        self.active_gba_patches = gba_patches;
        self.libretro_initialized = false; // Signals Libretro backend to re-sync cheat entries

        debug!(
            "CheatEngine: Recompiled patches (GBC: {}, GBA: {})",
            self.active_gbc_patches.len(),
            self.active_gba_patches.len()
        );
    }

    /// Adds a new cheat code with validation. Returns `Ok(())` or validation error string.
    pub fn add_cheat(
        &mut self,
        desc: String,
        code: String,
        cheat_type: CheatType,
    ) -> Result<(), String> {
        let validation = Self::validate_code(&code, cheat_type, self.is_gba);
        if let ValidationResult::Invalid(err) = validation {
            return Err(err);
        }

        self.cheats.add(desc, code, cheat_type);
        self.recompile_patches();
        let _ = self.save();
        Ok(())
    }

    /// Removes a cheat code at the given index.
    pub fn remove_cheat(&mut self, index: usize) -> bool {
        if self.cheats.remove_at(index).is_some() {
            self.recompile_patches();
            let _ = self.save();
            true
        } else {
            false
        }
    }

    /// Toggles the enabled state of a cheat code at index.
    pub fn toggle_cheat(&mut self, index: usize) {
        self.cheats.toggle_at(index);
        self.recompile_patches();
        let _ = self.save();
    }

    /// Enables or disables all cheat codes.
    pub fn toggle_all(&mut self, enable: bool) {
        self.cheats.set_all_enabled(enable);
        self.recompile_patches();
        let _ = self.save();
    }

    /// Removes all cheat entries.
    pub fn clear_all(&mut self) {
        self.cheats.clear();
        self.recompile_patches();
        let _ = self.save();
    }

    /// Validates code text format for non-hex characters and structure.
    pub fn validate_code(code: &str, cheat_type: CheatType, _is_gba: bool) -> ValidationResult {
        let lines: Vec<&str> = code
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();

        if lines.is_empty() {
            return ValidationResult::Invalid("Cheat code cannot be empty.".to_string());
        }

        for (i, line) in lines.iter().enumerate() {
            // Strip whitespace, pluses, colons, equals, dashes
            let stripped: String = line
                .chars()
                .filter(|c| !c.is_whitespace() && *c != '+' && *c != '-' && *c != ':' && *c != '=')
                .collect();

            if stripped.is_empty() {
                continue;
            }

            // Check that all characters are valid hex
            if !stripped.chars().all(|c| c.is_ascii_hexdigit()) {
                return ValidationResult::Invalid(format!(
                    "Line {} contains invalid non-hexadecimal characters: \"{}\"",
                    i + 1,
                    line
                ));
            }

            match cheat_type {
                CheatType::GameSharkGbc => {
                    if stripped.len() != 8 && stripped.len() != 6 {
                        return ValidationResult::Invalid(format!(
                            "Line {} GameShark GBC code must be 8 hex characters (e.g. 01XXYYZZ), got {} characters.",
                            i + 1,
                            stripped.len()
                        ));
                    }
                }
                CheatType::GameSharkGba | CheatType::ActionReplayMax => {
                    if stripped.len() != 12 && stripped.len() != 16 && stripped.len() != 8 {
                        return ValidationResult::Invalid(format!(
                            "Line {} GBA code must be 8, 12, or 16 hex characters (e.g. 82025840 002C), got {} characters.",
                            i + 1,
                            stripped.len()
                        ));
                    }
                }
                CheatType::Raw => {
                    if stripped.len() < 4 || stripped.len() > 16 {
                        return ValidationResult::Invalid(format!(
                            "Line {} Raw patch has invalid length ({} characters).",
                            i + 1,
                            stripped.len()
                        ));
                    }
                }
            }
        }

        ValidationResult::Valid {
            detected_type: cheat_type,
            lines_count: lines.len(),
        }
    }

    /// Injects active GBC GameShark cheat patches into the GBC memory bus.
    pub fn apply_to_gbc(&self, bus: &mut crate::gbc::mmu::MemoryBus) {
        for patch in &self.active_gbc_patches {
            // Guard: Address must be in safe writable memory ranges (WRAM, HRAM, SRAM)
            // 0xC000..=0xDFFF: WRAM (8 KB DMG / 32 KB banked GBC)
            // 0xFF80..=0xFFFE: HRAM (127 bytes)
            // 0xA000..=0xBFFF: External Cartridge RAM
            let addr = patch.address;
            if (0xC000..=0xDFFF).contains(&addr)
                || (0xFF80..=0xFFFE).contains(&addr)
                || (0xA000..=0xBFFF).contains(&addr)
            {
                bus.write_byte(addr, patch.value);
            }
        }
    }

    /// Injects active GBA cheat patches into GbaMemoryBus (EWRAM & IWRAM).
    pub fn apply_to_gba_mmu(&self, mmu: &mut crate::gba::mmu::GbaMemoryBus) {
        for patch in &self.active_gba_patches {
            let addr = patch.address;
            let val = patch.value;

            // EWRAM: 0x02000000..=0x0203FFFF (256 KB)
            if (0x02000000..=0x0203FFFF).contains(&addr) {
                let offset = (addr - 0x02000000) as usize;
                match patch.width {
                    1 => {
                        if offset < mmu.ewram.len() {
                            mmu.ewram[offset] = val as u8;
                        }
                    }
                    2 => {
                        if offset + 1 < mmu.ewram.len() {
                            mmu.ewram[offset] = val as u8;
                            mmu.ewram[offset + 1] = (val >> 8) as u8;
                        }
                    }
                    4 => {
                        if offset + 3 < mmu.ewram.len() {
                            mmu.ewram[offset] = val as u8;
                            mmu.ewram[offset + 1] = (val >> 8) as u8;
                            mmu.ewram[offset + 2] = (val >> 16) as u8;
                            mmu.ewram[offset + 3] = (val >> 24) as u8;
                        }
                    }
                    _ => {}
                }
            }
            // IWRAM: 0x03000000..=0x03007FFF (32 KB)
            else if (0x03000000..=0x03007FFF).contains(&addr) {
                let offset = (addr - 0x03000000) as usize;
                match patch.width {
                    1 => {
                        if offset < mmu.iwram.len() {
                            mmu.iwram[offset] = val as u8;
                        }
                    }
                    2 => {
                        if offset + 1 < mmu.iwram.len() {
                            mmu.iwram[offset] = val as u8;
                            mmu.iwram[offset + 1] = (val >> 8) as u8;
                        }
                    }
                    4 => {
                        if offset + 3 < mmu.iwram.len() {
                            mmu.iwram[offset] = val as u8;
                            mmu.iwram[offset + 1] = (val >> 8) as u8;
                            mmu.iwram[offset + 2] = (val >> 16) as u8;
                            mmu.iwram[offset + 3] = (val >> 24) as u8;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Injects active GBA cheat codes into a Libretro core backend (mGBA).
    pub fn apply_to_gba_libretro(&mut self, lr: &mut crate::gba::libretro::LibretroCore) {
        // 1. If Libretro core exports retro_cheat_set, register cheats with core
        if !self.libretro_initialized {
            lr.cheat_reset();
            let mut cheat_idx = 0;
            for entry in self.cheats.entries.iter().filter(|e| e.enabled) {
                // Form single-line Libretro code syntax
                let single_line = entry
                    .code
                    .lines()
                    .map(|l| l.trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" + ");

                if lr.cheat_set(cheat_idx, true, &single_line) {
                    cheat_idx += 1;
                }
            }
            self.libretro_initialized = true;
        }

        // 2. Direct RAM fallback for cores without native cheat handler or raw patches
        if let Some(sys_ram) = lr.get_system_ram_mut() {
            let ram_len = sys_ram.len();
            for patch in &self.active_gba_patches {
                let addr = patch.address;
                let val = patch.value;

                // Map EWRAM (0x02000000..0x0203FFFF)
                if (0x02000000..=0x0203FFFF).contains(&addr) {
                    let offset = (addr - 0x02000000) as usize;
                    match patch.width {
                        1 if offset < ram_len => {
                            sys_ram[offset] = val as u8;
                        }
                        2 if offset + 1 < ram_len => {
                            sys_ram[offset] = val as u8;
                            sys_ram[offset + 1] = (val >> 8) as u8;
                        }
                        4 if offset + 3 < ram_len => {
                            sys_ram[offset] = val as u8;
                            sys_ram[offset + 1] = (val >> 8) as u8;
                            sys_ram[offset + 2] = (val >> 16) as u8;
                            sys_ram[offset + 3] = (val >> 24) as u8;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

/// Decodes GBC GameShark code strings into memory patches.
///
/// GameShark GBC format: `01XXYYZZ`
/// - Value = `XX`
/// - Address = `(ZZ << 8) | YY`
pub fn decode_gbc_code(code_str: &str) -> Vec<GbcPatch> {
    let mut patches = Vec::new();

    for line in code_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Handle raw ADDR:VAL format (e.g. D016:FF or C000:01)
        if let Some((addr_str, val_str)) = line.split_once(':') {
            let addr_str = addr_str.trim().trim_start_matches("0x");
            let val_str = val_str.trim().trim_start_matches("0x");
            if let (Ok(addr), Ok(val)) = (
                u16::from_str_radix(addr_str, 16),
                u8::from_str_radix(val_str, 16),
            ) {
                patches.push(GbcPatch {
                    address: addr,
                    value: val,
                });
                continue;
            }
        }

        // Strip non-hex characters
        let hex: String = line.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex.len() == 8 {
            // Check prefix (01 = standard RAM poke, 91/90 = banked RAM poke)
            let prefix = &hex[0..2];
            if prefix == "01" || prefix == "91" || prefix == "90" {
                if let (Ok(val), Ok(yy), Ok(zz)) = (
                    u8::from_str_radix(&hex[2..4], 16),
                    u16::from_str_radix(&hex[4..6], 16),
                    u16::from_str_radix(&hex[6..8], 16),
                ) {
                    let address = (zz << 8) | yy;
                    patches.push(GbcPatch { address, value: val });
                }
            }
        }
    }

    patches
}

/// Decodes GBA GameShark, Action Replay, CodeBreaker, and Raw codes into memory patches.
pub fn decode_gba_code(code_str: &str, _cheat_type: CheatType) -> Vec<GbaPatch> {
    let mut patches = Vec::new();

    for line in code_str.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Handle ADDR:VAL format (e.g. 02025840:002C or 02000000:FF)
        if let Some((addr_str, val_str)) = line.split_once(':') {
            let addr_str = addr_str.trim().trim_start_matches("0x");
            let val_str = val_str.trim().trim_start_matches("0x");
            if let (Ok(addr), Ok(val)) = (
                u32::from_str_radix(addr_str, 16),
                u32::from_str_radix(val_str, 16),
            ) {
                let width = match val_str.len() {
                    1..=2 => 1,
                    3..=4 => 2,
                    _ => 4,
                };
                patches.push(GbaPatch {
                    address: addr,
                    value: val,
                    width,
                });
                continue;
            }
        }

        let hex: String = line.chars().filter(|c| c.is_ascii_hexdigit()).collect();

        // 12-Digit Format: "8AAAAAAA VVVV" (16-bit) or "3AAAAAAA 00VV" (8-bit)
        if hex.len() == 12 {
            let prefix = &hex[0..1];
            let addr_part = &hex[1..8];
            let val_part = &hex[8..12];

            if let (Ok(raw_addr), Ok(val)) = (
                u32::from_str_radix(addr_part, 16),
                u32::from_str_radix(val_part, 16),
            ) {
                // Ensure canonical GBA RAM base address:
                let address = if raw_addr < 0x02000000 {
                    0x02000000 | (raw_addr & 0x00FFFFFF)
                } else {
                    raw_addr
                };

                match prefix {
                    "8" => {
                        // 16-bit write
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFFFF,
                            width: 2,
                        });
                    }
                    "3" => {
                        // 8-bit write
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFF,
                            width: 1,
                        });
                    }
                    _ => {
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFFFF,
                            width: 2,
                        });
                    }
                }
            }
        }
        // 16-Digit Format: "XXXXXXXX YYYYYYYY" (Action Replay / GameShark Advance)
        else if hex.len() == 16 {
            let code_type = &hex[0..2];
            let addr_part = &hex[2..8];
            let val_part = &hex[8..16];

            if let (Ok(raw_addr), Ok(val)) = (
                u32::from_str_radix(addr_part, 16),
                u32::from_str_radix(val_part, 16),
            ) {
                let address = if raw_addr < 0x02000000 {
                    0x02000000 | (raw_addr & 0x00FFFFFF)
                } else {
                    raw_addr
                };

                match code_type {
                    "04" => {
                        // 32-bit RAM write
                        patches.push(GbaPatch {
                            address,
                            value: val,
                            width: 4,
                        });
                    }
                    "02" => {
                        // 16-bit RAM write
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFFFF,
                            width: 2,
                        });
                    }
                    "00" => {
                        // 8-bit RAM write
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFF,
                            width: 1,
                        });
                    }
                    _ => {
                        // Fallback: Default 16-bit write
                        patches.push(GbaPatch {
                            address,
                            value: val & 0xFFFF,
                            width: 2,
                        });
                    }
                }
            }
        }
    }

    patches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gbc_gameshark_decoding() {
        // 01FF16D0 -> Value: 0xFF, Address: 0xD016
        let patches = decode_gbc_code("01FF16D0");
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].address, 0xD016);
        assert_eq!(patches[0].value, 0xFF);

        // 010370D0 -> Value: 0x03, Address: 0xD070
        let patches2 = decode_gbc_code("010370D0");
        assert_eq!(patches2.len(), 1);
        assert_eq!(patches2[0].address, 0xD070);
        assert_eq!(patches2[0].value, 0x03);
    }

    #[test]
    fn test_gba_gameshark_decoding() {
        // 82025840 002C -> 16-bit write 0x002C at 0x02025840
        let patches = decode_gba_code("82025840 002C", CheatType::GameSharkGba);
        assert_eq!(patches.len(), 1);
        assert_eq!(patches[0].address, 0x02025840);
        assert_eq!(patches[0].value, 0x002C);
        assert_eq!(patches[0].width, 2);

        // 32003884 00FF -> 8-bit write 0xFF at 0x02003884
        let patches2 = decode_gba_code("32003884 00FF", CheatType::GameSharkGba);
        assert_eq!(patches2.len(), 1);
        assert_eq!(patches2[0].address, 0x02003884);
        assert_eq!(patches2[0].value, 0xFF);
        assert_eq!(patches2[0].width, 1);
    }

    #[test]
    fn test_code_validation() {
        let valid = CheatEngine::validate_code("82025840 002C", CheatType::GameSharkGba, true);
        assert!(matches!(valid, ValidationResult::Valid { .. }));

        let invalid_chars = CheatEngine::validate_code("82025840 00ZZ", CheatType::GameSharkGba, true);
        assert!(matches!(invalid_chars, ValidationResult::Invalid(_)));

        let invalid_len = CheatEngine::validate_code("01FF", CheatType::GameSharkGbc, false);
        assert!(matches!(invalid_len, ValidationResult::Invalid(_)));
    }

    #[test]
    fn test_gbc_live_ram_injection_and_bounds() {
        let mut bus = crate::gbc::mmu::MemoryBus::new();
        let mut engine = CheatEngine::new();
        engine.is_gba = false;

        // Valid WRAM write (0xD016)
        engine
            .add_cheat(
                "Infinite Lives".to_string(),
                "016316D0".to_string(), // Value 0x63, Address 0xD016
                CheatType::GameSharkGbc,
            )
            .expect("Valid code");

        // Invalid ROM write (0x0100 - must be ignored to protect ROM integrity)
        engine
            .add_cheat(
                "Invalid ROM Poke".to_string(),
                "01AA0001".to_string(), // Address 0x0100 (ROM range)
                CheatType::GameSharkGbc,
            )
            .expect("Valid format");

        engine.apply_to_gbc(&mut bus);

        // Verify WRAM was patched
        assert_eq!(bus.read_byte(0xD016), 0x63);
    }

    #[test]
    fn test_gba_mmu_ram_injection() {
        let mut mmu = crate::gba::mmu::GbaMemoryBus::new();
        let mut engine = CheatEngine::new();
        engine.is_gba = true;

        // EWRAM 16-bit patch: 0x02025840 -> 0x002C (Rare Candy)
        engine
            .add_cheat(
                "Rare Candy".to_string(),
                "82025840 002C".to_string(),
                CheatType::GameSharkGba,
            )
            .expect("Valid code");

        // IWRAM 8-bit patch: 0x03001234 -> 0xFF
        engine
            .add_cheat(
                "Max Stat".to_string(),
                "33001234 00FF".to_string(),
                CheatType::GameSharkGba,
            )
            .expect("Valid code");

        engine.apply_to_gba_mmu(&mut mmu);

        let ewram_val = mmu.read_u16(0x02025840);
        assert_eq!(ewram_val, 0x002C);

        let iwram_val = mmu.read_u8(0x03001234);
        assert_eq!(iwram_val, 0xFF);
    }

    #[test]
    fn test_cheat_engine_file_persistence_roundtrip() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join("pixeldrive_cheat_test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let mut dummy_rom = vec![0u8; 0x200];
        dummy_rom[0x0B2] = 0x96;
        dummy_rom[0xA0..0xAC].copy_from_slice(b"POKEMON EMER");
        dummy_rom[0xAC..0xB0].copy_from_slice(b"BPEE");

        let mut engine = CheatEngine::new();
        engine.load_for_rom(&dummy_rom, &temp_dir);

        engine
            .add_cheat(
                "Infinite Master Balls".to_string(),
                "82025840 0001".to_string(),
                CheatType::GameSharkGba,
            )
            .unwrap();

        // Verify .cht was saved to disk
        let cheat_file = engine.file_path.clone().expect("File path must exist");
        assert!(cheat_file.exists());

        // Reload fresh engine from disk
        let mut loaded_engine = CheatEngine::new();
        loaded_engine.load_for_rom(&dummy_rom, &temp_dir);

        assert_eq!(loaded_engine.cheats.len(), 1);
        assert_eq!(loaded_engine.cheats.entries[0].desc, "Infinite Master Balls");
        assert_eq!(loaded_engine.cheats.entries[0].code, "82025840 0001");
        assert_eq!(loaded_engine.active_gba_patches.len(), 1);

        let _ = std::fs::remove_dir_all(temp_dir);
        Ok(())
    }
}
