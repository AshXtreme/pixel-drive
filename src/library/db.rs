//! Persistent ROM Library Database
//!
//! Tracks recently launched ROMs, display names, CRC32 hashes, timestamps,
//! and snapshot thumbnail image paths stored in `<files_dir>/config/recent_roms.json`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use log::{info, warn};
use serde::{Deserialize, Serialize};

/// Maximum number of recent games retained in the persistent library.
pub const MAX_RECENT_ROMS: usize = 30;

/// Metadata entry representing a previously loaded ROM in the user's library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomEntry {
    /// File path on filesystem or Android Content URI (`content://...`).
    pub path: String,
    /// Canonical display name combining title and cartridge game code.
    pub display_name: String,
    /// Uppercase 8-character hexadecimal representation of ROM CRC32.
    pub crc32: String,
    /// Unix epoch timestamp (seconds) when this ROM was last booted or exited.
    pub last_played: u64,
    /// Optional absolute path to generated JPEG snapshot thumbnail.
    #[serde(default)]
    pub thumbnail_path: Option<String>,
    /// Indicates if a quick-resume auto-save state snapshot exists on disk.
    #[serde(default)]
    pub has_auto_save: bool,
}

impl RomEntry {
    /// Constructs a new `RomEntry` with current system timestamp.
    pub fn new(path: String, display_name: String, crc32: String, thumbnail_path: Option<String>) -> Self {
        let last_played = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            path,
            display_name,
            crc32,
            last_played,
            thumbnail_path,
            has_auto_save: false,
        }
    }

    /// Sets whether this entry has an associated auto-save snapshot on disk.
    pub fn with_auto_save(mut self, has_auto_save: bool) -> Self {
        self.has_auto_save = has_auto_save;
        self
    }
}

/// Library database manager handling JSON I/O and recent games ordering.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryManager {
    /// List of registered ROM entries.
    pub entries: Vec<RomEntry>,
    /// Target path on disk where `recent_roms.json` is persisted.
    #[serde(skip)]
    pub config_path: PathBuf,
}

impl LibraryManager {
    /// Loads library database from `config_path`, or initializes empty manager if absent.
    pub fn load_or_create(config_path: &Path) -> Self {
        if config_path.exists() {
            match fs::read_to_string(config_path) {
                Ok(content) => match serde_json::from_str::<Vec<RomEntry>>(&content) {
                    Ok(mut entries) => {
                        entries.sort_by(|a, b| b.last_played.cmp(&a.last_played));
                        info!(
                            "LibraryManager: loaded {} recent ROM entries from {:?}",
                            entries.len(),
                            config_path
                        );
                        return Self {
                            entries,
                            config_path: config_path.to_path_buf(),
                        };
                    }
                    Err(err) => {
                        warn!("LibraryManager: failed to parse {:?}: {}", config_path, err);
                    }
                },
                Err(err) => {
                    warn!("LibraryManager: failed to read {:?}: {}", config_path, err);
                }
            }
        }

        Self {
            entries: Vec::new(),
            config_path: config_path.to_path_buf(),
        }
    }

    /// Persists the active entries list to disk as pretty JSON.
    pub fn save_to_disk(&self) -> io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let json_data = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(&self.config_path, json_data.as_bytes())?;
        info!(
            "LibraryManager: successfully saved {} entries to {:?}",
            self.entries.len(),
            self.config_path
        );
        Ok(())
    }

    /// Registers a newly loaded ROM or updates an existing entry's timestamp & path.
    pub fn add_or_update(
        &mut self,
        path: &str,
        display_name: &str,
        crc32: &str,
        thumbnail_path: Option<String>,
    ) -> &RomEntry {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // Check if matching CRC32 already exists
        if let Some(idx) = self.entries.iter().position(|e| e.crc32.eq_ignore_ascii_case(crc32)) {
            self.entries[idx].path = path.to_string();
            self.entries[idx].display_name = display_name.to_string();
            self.entries[idx].last_played = now;
            if thumbnail_path.is_some() {
                self.entries[idx].thumbnail_path = thumbnail_path;
            }
        } else {
            self.entries.push(RomEntry {
                path: path.to_string(),
                display_name: display_name.to_string(),
                crc32: crc32.to_uppercase(),
                last_played: now,
                thumbnail_path,
                has_auto_save: false,
            });
        }

        // Sort descending by last_played
        self.entries.sort_by(|a, b| b.last_played.cmp(&a.last_played));
        self.entries.truncate(MAX_RECENT_ROMS);

        let _ = self.save_to_disk();
        &self.entries[0]
    }

    /// Updates the thumbnail path for a specific ROM identified by CRC32.
    pub fn update_thumbnail(&mut self, crc32: &str, thumb_path: &str) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.crc32.eq_ignore_ascii_case(crc32)) {
            entry.thumbnail_path = Some(thumb_path.to_string());
            let _ = self.save_to_disk();
            true
        } else {
            false
        }
    }

    /// Marks whether a fast-resume auto-save state snapshot exists on disk for this ROM.
    pub fn mark_auto_save(&mut self, crc32: &str, has_auto_save: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.crc32.eq_ignore_ascii_case(crc32)) {
            entry.has_auto_save = has_auto_save;
            let _ = self.save_to_disk();
            true
        } else {
            false
        }
    }

    /// Returns sorted slice of recent entries (newest first).
    pub fn recent_entries(&self) -> &[RomEntry] {
        &self.entries
    }

    /// Finds a ROM entry by CRC32 hex string.
    pub fn find_by_crc32(&self, crc32: &str) -> Option<&RomEntry> {
        self.entries.iter().find(|e| e.crc32.eq_ignore_ascii_case(crc32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_library_manager_add_and_sort() {
        let temp_dir = std::env::temp_dir().join("pixeldrive_test_lib");
        let config_path = temp_dir.join("recent_roms.json");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut manager = LibraryManager::load_or_create(&config_path);
        assert!(manager.recent_entries().is_empty());

        manager.add_or_update("path/game1.gba", "Game 1", "11111111", None);
        std::thread::sleep(std::time::Duration::from_millis(15));
        manager.add_or_update("path/game2.gbc", "Game 2", "22222222", Some("thumb2.jpg".to_string()));

        let recents = manager.recent_entries();
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0].crc32, "22222222");
        assert_eq!(recents[1].crc32, "11111111");

        // Re-adding game1 should bump it to front
        std::thread::sleep(std::time::Duration::from_millis(15));
        manager.add_or_update("path/game1.gba", "Game 1 [NEW]", "11111111", Some("thumb1.jpg".to_string()));
        let recents = manager.recent_entries();
        assert_eq!(recents.len(), 2);
        assert_eq!(recents[0].crc32, "11111111");
        assert_eq!(recents[0].display_name, "Game 1 [NEW]");
        assert_eq!(recents[0].thumbnail_path.as_deref(), Some("thumb1.jpg"));

        // Test persistence reload
        let reloaded = LibraryManager::load_or_create(&config_path);
        assert_eq!(reloaded.recent_entries().len(), 2);
        assert_eq!(reloaded.recent_entries()[0].crc32, "11111111");

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_library_manager_auto_save() {
        let temp_dir = std::env::temp_dir().join("pixeldrive_test_lib_autosave");
        let config_path = temp_dir.join("recent_roms.json");
        let _ = fs::remove_dir_all(&temp_dir);

        let mut manager = LibraryManager::load_or_create(&config_path);
        manager.add_or_update("path/pokemon.gba", "Pokemon FireRed", "84EE4776", None);
        assert!(!manager.recent_entries()[0].has_auto_save);

        assert!(manager.mark_auto_save("84EE4776", true));
        assert!(manager.recent_entries()[0].has_auto_save);

        // Verify persistence
        let reloaded = LibraryManager::load_or_create(&config_path);
        assert!(reloaded.recent_entries()[0].has_auto_save);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
