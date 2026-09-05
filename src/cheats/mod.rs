//! Per-Game Cheat Code Engine & Libretro .cht Persistence.
//!
//! Provides data models, parser/serializer for standard Libretro-compatible `.cht` files,
//! format validators, and persistent storage management for GameShark, Action Replay,
//! and raw memory patch cheat codes.

pub mod engine;

pub use engine::CheatEngine;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Supported cheat code formats and decryption handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CheatType {
    #[default]
    Raw,
    GameSharkGba,
    ActionReplayMax,
    GameSharkGbc,
}

impl CheatType {
    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            CheatType::Raw => "Raw Memory Patch",
            CheatType::GameSharkGba => "GameShark Advance (GBA)",
            CheatType::ActionReplayMax => "Action Replay MAX (GBA)",
            CheatType::GameSharkGbc => "GameShark (GB/GBC)",
        }
    }

    /// Short badge identifier for compact UI lists.
    pub fn badge(&self) -> &'static str {
        match self {
            CheatType::Raw => "RAW",
            CheatType::GameSharkGba => "GBA-GS",
            CheatType::ActionReplayMax => "AR-MAX",
            CheatType::GameSharkGbc => "GBC-GS",
        }
    }

    /// Auto-detects cheat type based on raw code string syntax and target system.
    pub fn detect(code: &str, is_gba: bool) -> Self {
        let cleaned: String = code
            .chars()
            .filter(|c| c.is_ascii_hexdigit() || *c == ':' || *c == '=')
            .collect();

        if !is_gba {
            return CheatType::GameSharkGbc;
        }

        if cleaned.contains(':') || cleaned.contains('=') {
            return CheatType::Raw;
        }

        let hex_only: String = cleaned.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex_only.len() == 12 {
            CheatType::GameSharkGba
        } else if hex_only.len() == 16 {
            CheatType::ActionReplayMax
        } else {
            CheatType::GameSharkGba
        }
    }
}

/// A single cheat code entry with description, code payload, and toggle state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheatEntry {
    pub id: String,
    pub desc: String,
    pub code: String, // Multiline or spaced codes
    pub enabled: bool,
    pub cheat_type: CheatType,
}

impl CheatEntry {
    /// Creates a new cheat entry with a generated unique ID.
    pub fn new(desc: String, code: String, enabled: bool, cheat_type: CheatType) -> Self {
        let id = format!(
            "cht_{}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0),
            fastrand::u32(1000..9999)
        );
        Self {
            id,
            desc,
            code,
            enabled,
            cheat_type,
        }
    }

    /// Formats the code for clean multi-line display.
    pub fn formatted_code(&self) -> String {
        self.code
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Collection of cheats for a specific ROM identified by its CRC32 checksum.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameCheats {
    pub rom_crc32: u32,
    pub game_title: String,
    pub entries: Vec<CheatEntry>,
}

impl GameCheats {
    /// Creates an empty collection for a game.
    pub fn new(rom_crc32: u32, game_title: String) -> Self {
        Self {
            rom_crc32,
            game_title,
            entries: Vec::new(),
        }
    }

    /// Number of total cheat entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if no cheats are present.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Number of currently enabled cheats.
    pub fn enabled_count(&self) -> usize {
        self.entries.iter().filter(|e| e.enabled).count()
    }

    /// Adds a new cheat code entry.
    pub fn add(&mut self, desc: String, code: String, cheat_type: CheatType) -> &CheatEntry {
        let entry = CheatEntry::new(desc, code, true, cheat_type);
        self.entries.push(entry);
        self.entries.last().unwrap()
    }

    /// Removes a cheat entry by its ID.
    pub fn remove(&mut self, id: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }

    /// Removes a cheat entry by index.
    pub fn remove_at(&mut self, index: usize) -> Option<CheatEntry> {
        if index < self.entries.len() {
            Some(self.entries.remove(index))
        } else {
            None
        }
    }

    /// Toggles enabled state of a cheat by ID.
    pub fn toggle(&mut self, id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.enabled = !entry.enabled;
        }
    }

    /// Toggles enabled state of a cheat by index.
    pub fn toggle_at(&mut self, index: usize) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.enabled = !entry.enabled;
        }
    }

    /// Bulk toggles all cheats to either enabled or disabled.
    pub fn set_all_enabled(&mut self, enabled: bool) {
        for entry in &mut self.entries {
            entry.enabled = enabled;
        }
    }

    /// Removes all cheats.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Serializes cheats into standard Libretro `.cht` syntax.
    pub fn to_cht_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# PixelDrive Cheat File: {}\n", self.game_title));
        out.push_str(&format!("# ROM CRC32: {:08X}\n\n", self.rom_crc32));
        out.push_str(&format!("cheats = {}\n\n", self.entries.len()));

        for (i, entry) in self.entries.iter().enumerate() {
            let single_line_code = entry
                .code
                .lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" + ");

            out.push_str(&format!("cheat{}_desc = \"{}\"\n", i, entry.desc.replace('"', "\\\"")));
            out.push_str(&format!("cheat{}_code = \"{}\"\n", i, single_line_code));
            out.push_str(&format!("cheat{}_enable = {}\n", i, entry.enabled));
            out.push_str(&format!("cheat{}_type = \"{:?}\"\n\n", i, entry.cheat_type));
        }

        out
    }

    /// Parses standard Libretro `.cht` file content into `GameCheats`.
    pub fn parse_cht_str(content: &str, rom_crc32: u32, game_title: &str) -> Self {
        use std::collections::HashMap;

        let mut descs: HashMap<usize, String> = HashMap::new();
        let mut codes: HashMap<usize, String> = HashMap::new();
        let mut enables: HashMap<usize, bool> = HashMap::new();
        let mut types: HashMap<usize, CheatType> = HashMap::new();
        let mut max_idx: Option<usize> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                let unquoted = if (val.starts_with('"') && val.ends_with('"'))
                    || (val.starts_with('\'') && val.ends_with('\''))
                {
                    if val.len() >= 2 {
                        &val[1..val.len() - 1]
                    } else {
                        ""
                    }
                } else {
                    val
                };

                if key == "cheats" {
                    continue;
                }

                if key.starts_with("cheat") {
                    let rest = &key[5..];
                    if let Some(underscore_pos) = rest.find('_') {
                        let idx_str = &rest[..underscore_pos];
                        let field = &rest[underscore_pos + 1..];

                        if let Ok(idx) = idx_str.parse::<usize>() {
                            max_idx = Some(max_idx.map_or(idx, |m: usize| m.max(idx)));
                            match field {
                                "desc" => {
                                    descs.insert(idx, unquoted.replace("\\\"", "\""));
                                }
                                "code" => {
                                    let normalized = unquoted.replace(" + ", "\n");
                                    codes.insert(idx, normalized);
                                }
                                "enable" => {
                                    let enabled = match unquoted.to_lowercase().as_str() {
                                        "true" | "1" | "yes" | "on" => true,
                                        _ => false,
                                    };
                                    enables.insert(idx, enabled);
                                }
                                "type" => {
                                    let ct = match unquoted.to_lowercase().as_str() {
                                        "gamesharkgba" | "gameshark_gba" | "gba" => CheatType::GameSharkGba,
                                        "actionreplaymax" | "action_replay" | "ar" => CheatType::ActionReplayMax,
                                        "gamesharkgbc" | "gameshark_gbc" | "gbc" => CheatType::GameSharkGbc,
                                        "raw" => CheatType::Raw,
                                        _ => CheatType::Raw,
                                    };
                                    types.insert(idx, ct);
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        let mut entries = Vec::new();
        if let Some(max) = max_idx {
            for idx in 0..=max {
                if let Some(code) = codes.remove(&idx) {
                    let desc = descs
                        .remove(&idx)
                        .unwrap_or_else(|| format!("Cheat Code {}", idx + 1));
                    let enabled = enables.remove(&idx).unwrap_or(true);
                    let cheat_type = types.remove(&idx).unwrap_or_else(|| {
                        CheatType::detect(&code, true)
                    });

                    entries.push(CheatEntry {
                        id: format!("cht_{}_{}", idx, fastrand::u32(1000..9999)),
                        desc,
                        code,
                        enabled,
                        cheat_type,
                    });
                }
            }
        }

        Self {
            rom_crc32,
            game_title: game_title.to_string(),
            entries,
        }
    }

    /// Loads cheats from a `.cht` file on disk. If the file does not exist, returns an empty collection.
    pub fn load_from_path<P: AsRef<Path>>(path: P, rom_crc32: u32, game_title: &str) -> std::io::Result<Self> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Ok(Self::new(rom_crc32, game_title.to_string()));
        }

        let content = fs::read_to_string(path_ref)?;
        Ok(Self::parse_cht_str(&content, rom_crc32, game_title))
    }

    /// Atomically saves cheats to a `.cht` file on disk.
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = self.to_cht_string();
        let tmp_path = path_ref.with_extension("cht.tmp");
        fs::write(&tmp_path, &content)?;
        fs::rename(&tmp_path, path_ref)?;
        Ok(())
    }
}

/// Fallback fastrand generator using system timestamps when rand crate is not in dependencies.
mod fastrand {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEED: AtomicU32 = AtomicU32::new(0x12345678);

    pub fn u32(range: std::ops::Range<u32>) -> u32 {
        let mut cur = SEED.load(Ordering::Relaxed);
        if cur == 0 {
            cur = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0xCAFEBABE);
        }
        let next = cur.wrapping_mul(1664525).wrapping_add(1013904223);
        SEED.store(next, Ordering::Relaxed);
        let span = range.end.saturating_sub(range.start).max(1);
        range.start + (next % span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cht_serialization_and_parsing() {
        let mut cheats = GameCheats::new(0x1234ABCD, "Pokemon FireRed".to_string());
        cheats.add(
            "Infinite Rare Candy".to_string(),
            "82025840 002C".to_string(),
            CheatType::GameSharkGba,
        );
        cheats.add(
            "Max Money".to_string(),
            "32003884 00FF\n32003885 00FF".to_string(),
            CheatType::Raw,
        );
        cheats.entries[1].enabled = false;

        let cht_text = cheats.to_cht_string();
        assert!(cht_text.contains("cheats = 2"));
        assert!(cht_text.contains("cheat0_desc = \"Infinite Rare Candy\""));
        assert!(cht_text.contains("cheat0_code = \"82025840 002C\""));
        assert!(cht_text.contains("cheat0_enable = true"));
        assert!(cht_text.contains("cheat1_desc = \"Max Money\""));
        assert!(cht_text.contains("cheat1_enable = false"));

        let parsed = GameCheats::parse_cht_str(&cht_text, 0x1234ABCD, "Pokemon FireRed");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed.entries[0].desc, "Infinite Rare Candy");
        assert_eq!(parsed.entries[0].code, "82025840 002C");
        assert_eq!(parsed.entries[0].enabled, true);
        assert_eq!(parsed.entries[0].cheat_type, CheatType::GameSharkGba);

        assert_eq!(parsed.entries[1].desc, "Max Money");
        assert_eq!(parsed.entries[1].enabled, false);
        assert_eq!(parsed.entries[1].cheat_type, CheatType::Raw);
    }
}
