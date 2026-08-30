use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

pub const SAVES_DIR: &str = "saves";

/// Helper function performing atomic file writes using a temporary staging file and rename.
fn atomic_write_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("file");
    let temp_name = format!(".{}.tmp.{}", file_name, std::process::id());
    let temp_path = path.with_file_name(temp_name);

    // Write data to temporary file
    fs::write(&temp_path, data)?;

    // Atomically rename temporary file over target file
    if let Err(err) = fs::rename(&temp_path, path) {
        // Fallback if target exists and platform restricts atomic replacement
        if path.exists() {
            let _ = fs::remove_file(path);
            fs::rename(&temp_path, path)?;
        } else {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
    }

    Ok(())
}

/// Unified Save Manager for persisting and loading battery-backed `.sav` RAM files and `.state{slot}` real-time snapshots.
pub struct SaveManager;

impl SaveManager {
    /// Ensures the dedicated `./saves/` directory exists and returns its path.
    pub fn ensure_save_directory() -> std::io::Result<PathBuf> {
        let dir = PathBuf::from(SAVES_DIR);
        if !dir.exists() {
            fs::create_dir_all(&dir)?;
            info!("Created saves directory at {:?}", dir);
        }
        Ok(dir)
    }

    /// Sanitizes any incoming ROM stem or filename to prevent directory traversal attacks.
    pub fn sanitize_stem(raw: &str) -> String {
        let base = Path::new(raw)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(raw);

        let sanitized: String = base
            .chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
                c if c.is_control() => '_',
                c => c,
            })
            .collect();

        let clean = sanitized.trim().trim_matches('.');
        if clean.is_empty() {
            "game".to_string()
        } else {
            clean.to_string()
        }
    }

    /// Derives the canonical `.sav` file path from a ROM file path: `./saves/<rom_stem>.sav`.
    pub fn get_save_path(rom_path: &Path) -> PathBuf {
        let stem = rom_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("game");

        let clean_stem = Self::sanitize_stem(stem);
        let mut save_path = PathBuf::from(SAVES_DIR);
        save_path.push(format!("{}.sav", clean_stem));
        save_path
    }

    /// Reads existing `.sav` file from disk if present.
    pub fn load_save_file(path: &Path) -> Option<Vec<u8>> {
        if !path.exists() {
            return None;
        }

        match fs::read(path) {
            Ok(bytes) => {
                info!(
                    "Loaded persistent save file: {:?} ({} bytes)",
                    path,
                    bytes.len()
                );
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read save file {:?}: {}", path, err);
                None
            }
        }
    }

    /// Flushes battery RAM data to disk atomically, ensuring parent directory exists.
    pub fn write_save_file(path: &Path, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        atomic_write_file(path, data)?;
        info!(
            "Flushed save data ({} bytes) atomically to {:?}",
            data.len(),
            path
        );
        Ok(())
    }

    /// Derives the canonical save state path: `./saves/<rom_stem>.state<slot>`.
    #[allow(dead_code)]
    pub fn get_state_path(rom_path: &Path, slot: usize) -> PathBuf {
        let stem = rom_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("game");

        Self::get_state_path_from_stem(stem, slot)
    }

    /// Derives save state path from ROM stem: `./saves/{rom_stem}.state{slot}`.
    pub fn get_state_path_from_stem(rom_stem: &str, slot: usize) -> PathBuf {
        let clean_stem = Self::sanitize_stem(rom_stem);
        let mut state_path = PathBuf::from(SAVES_DIR);
        state_path.push(format!("{}.state{}", clean_stem, slot));
        state_path
    }

    /// Writes real-time save state snapshot bytes to disk under `./saves/{rom_stem}.state{slot}` atomically.
    pub fn save_state_to_disk(rom_stem: &str, slot: usize, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        let path = Self::get_state_path_from_stem(rom_stem, slot);
        atomic_write_file(&path, data)?;
        info!(
            "Saved state snapshot ({} bytes) atomically to disk -> {:?}",
            data.len(),
            path
        );
        Ok(())
    }

    /// Reads real-time save state snapshot bytes from disk from `./saves/{rom_stem}.state{slot}`.
    pub fn load_state_from_disk(rom_stem: &str, slot: usize) -> Option<Vec<u8>> {
        let path = Self::get_state_path_from_stem(rom_stem, slot);
        if !path.exists() {
            warn!("Save state file does not exist on disk: {:?}", path);
            return None;
        }

        match fs::read(&path) {
            Ok(bytes) => {
                info!(
                    "Loaded save state snapshot from disk: {:?} ({} bytes)",
                    path,
                    bytes.len()
                );
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read save state file {:?}: {}", path, err);
                None
            }
        }
    }

    /// Checks if `./saves/{rom_stem}.state{slot}` exists on disk.
    pub fn state_exists_on_disk(rom_stem: &str, slot: usize) -> bool {
        let path = Self::get_state_path_from_stem(rom_stem, slot);
        path.exists()
    }

    /// Writes real-time save state snapshot bytes to disk atomically.
    #[allow(dead_code)]
    pub fn write_save_state(path: &Path, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        atomic_write_file(path, data)?;
        info!(
            "Saved state snapshot ({} bytes) atomically to {:?}",
            data.len(),
            path
        );
        Ok(())
    }

    /// Reads real-time save state snapshot bytes from disk if present.
    #[allow(dead_code)]
    pub fn read_save_state(path: &Path) -> Option<Vec<u8>> {
        if !path.exists() {
            warn!("Save state file does not exist: {:?}", path);
            return None;
        }

        match fs::read(path) {
            Ok(bytes) => {
                info!(
                    "Loaded save state snapshot: {:?} ({} bytes)",
                    path,
                    bytes.len()
                );
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read save state file {:?}: {}", path, err);
                None
            }
        }
    }

    /// Canonical directory path for a game's dedicated save states: `./saves/states/<clean_title>/`.
    pub fn get_game_states_dir(game_title: &str) -> PathBuf {
        let clean = Self::sanitize_stem(game_title);
        PathBuf::from(SAVES_DIR).join("states").join(clean)
    }

    /// Canonical path for a state slot file: `<states_dir>/<clean_title>/slot_<slot>.state`.
    pub fn get_slot_state_path(game_title: &str, slot: u8) -> PathBuf {
        Self::get_game_states_dir(game_title).join(format!("slot_{}.state", slot))
    }

    /// Canonical path for a state slot metadata file: `<states_dir>/<clean_title>/slot_<slot>.meta`.
    pub fn get_slot_meta_path(game_title: &str, slot: u8) -> PathBuf {
        Self::get_game_states_dir(game_title).join(format!("slot_{}.meta", slot))
    }

    /// Saves snapshot data to a designated slot (1..=5) and writes metadata.
    pub fn save_to_slot(game_title: &str, slot: u8, data: &[u8]) -> std::io::Result<SlotMetadata> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = Self::get_slot_state_path(game_title, clamped_slot);
        let meta_path = Self::get_slot_meta_path(game_title, clamped_slot);

        atomic_write_file(&state_path, data)?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let meta = SlotMetadata {
            slot_index: clamped_slot,
            timestamp: ts,
            formatted_time: format_unix_timestamp(ts),
            is_empty: false,
        };

        if let Ok(meta_bytes) = bincode::serialize(&meta) {
            let _ = atomic_write_file(&meta_path, &meta_bytes);
        }

        info!(
            "Saved state slot {} for '{}' ({} bytes, timestamp {})",
            clamped_slot, game_title, data.len(), meta.formatted_time
        );
        Ok(meta)
    }

    /// Loads snapshot data from a designated slot (1..=5).
    pub fn load_from_slot(game_title: &str, slot: u8) -> Result<Vec<u8>, std::io::Error> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = Self::get_slot_state_path(game_title, clamped_slot);
        if !state_path.exists() {
            // Also check legacy fallback `<saves_dir>/{clean}.state{slot}`
            let legacy_path = Self::get_state_path_from_stem(game_title, clamped_slot as usize);
            if legacy_path.exists() {
                return fs::read(&legacy_path);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Slot {} for '{}' not found", clamped_slot, game_title),
            ));
        }
        fs::read(&state_path)
    }

    /// Queries metadata for all 5 slots of a given game.
    pub fn get_slots_info(game_title: &str) -> [SlotMetadata; 5] {
        let mut slots = [
            SlotMetadata::empty(1),
            SlotMetadata::empty(2),
            SlotMetadata::empty(3),
            SlotMetadata::empty(4),
            SlotMetadata::empty(5),
        ];

        for i in 1..=5 {
            let state_path = Self::get_slot_state_path(game_title, i);
            let meta_path = Self::get_slot_meta_path(game_title, i);
            let legacy_path = Self::get_state_path_from_stem(game_title, i as usize);

            if state_path.exists() || legacy_path.exists() {
                if let Ok(meta_bytes) = fs::read(&meta_path) {
                    if let Ok(meta) = bincode::deserialize::<SlotMetadata>(&meta_bytes) {
                        slots[(i - 1) as usize] = meta;
                        continue;
                    }
                }
                let target = if state_path.exists() { state_path } else { legacy_path };
                let ts = target
                    .metadata()
                    .and_then(|m| m.modified())
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    })
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                slots[(i - 1) as usize] = SlotMetadata {
                    slot_index: i,
                    timestamp: ts,
                    formatted_time: format_unix_timestamp(ts),
                    is_empty: false,
                };
            }
        }
        slots
    }

    /// Deletes a save state slot and its metadata.
    pub fn delete_slot(game_title: &str, slot: u8) -> std::io::Result<()> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = Self::get_slot_state_path(game_title, clamped_slot);
        let meta_path = Self::get_slot_meta_path(game_title, clamped_slot);
        if state_path.exists() {
            let _ = fs::remove_file(state_path);
        }
        if meta_path.exists() {
            let _ = fs::remove_file(meta_path);
        }
        Ok(())
    }
}

/// Metadata representation for a persistent save state slot (Slots 1..=5).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SlotMetadata {
    pub slot_index: u8,
    pub timestamp: u64,
    pub formatted_time: String,
    pub is_empty: bool,
}

impl SlotMetadata {
    pub fn empty(slot_index: u8) -> Self {
        Self {
            slot_index,
            timestamp: 0,
            formatted_time: "[ Empty Slot ]".to_string(),
            is_empty: true,
        }
    }
}

/// Formats a Unix epoch timestamp (seconds) to civil `YYYY-MM-DD HH:MM`.
pub fn format_unix_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "[ Empty Slot ]".to_string();
    }
    let min = (ts / 60) % 60;
    let hour = (ts / 3600) % 24;
    let mut days = (ts / 86400) as i64;

    // Epoch: 1970-01-01
    days += 719468;
    let era = if days >= 0 { days } else { days - 146096 } / 146097;
    let doe = (days - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, m, d, hour, min)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_path_resolution() {
        let rom_path = Path::new("/path/to/Pokemon_FireRed.gba");
        let save_path = SaveManager::get_save_path(rom_path);
        assert_eq!(save_path, PathBuf::from("saves/Pokemon_FireRed.sav"));

        let zip_rom = Path::new("downloads/Pokemon - Crystal Version (USA).zip");
        let zip_save = SaveManager::get_save_path(zip_rom);
        assert_eq!(
            zip_save,
            PathBuf::from("saves/Pokemon - Crystal Version (USA).sav")
        );

        let state1_path = SaveManager::get_state_path(rom_path, 1);
        assert_eq!(state1_path, PathBuf::from("saves/Pokemon_FireRed.state1"));

        let state2_path = SaveManager::get_state_path(zip_rom, 2);
        assert_eq!(
            state2_path,
            PathBuf::from("saves/Pokemon - Crystal Version (USA).state2")
        );
    }

    #[test]
    fn test_sanitize_stem_path_traversal() {
        let malicious_stem = "../../../../etc/cron.d/payload";
        let sanitized = SaveManager::sanitize_stem(malicious_stem);
        assert_eq!(sanitized, "payload");

        let state_path = SaveManager::get_state_path_from_stem(malicious_stem, 1);
        assert_eq!(state_path, PathBuf::from("saves/payload.state1"));

        let dangerous_chars = "game:with/slash\\and\0null";
        let sanitized_chars = SaveManager::sanitize_stem(dangerous_chars);
        assert!(!sanitized_chars.contains('/'));
        assert!(!sanitized_chars.contains('\\'));
        assert!(!sanitized_chars.contains(':'));
        assert!(!sanitized_chars.contains('\0'));
    }

    #[test]
    fn test_save_read_write_roundtrip() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_save_path = temp_dir.join("test_battery_save.sav");

        let dummy_data = vec![0x42u8; 32768]; // 32KB RAM
        SaveManager::write_save_file(&test_save_path, &dummy_data)?;

        let loaded = SaveManager::load_save_file(&test_save_path).expect("Should load save file");
        assert_eq!(loaded.len(), dummy_data.len());
        assert_eq!(loaded, dummy_data);

        let _ = fs::remove_file(test_save_path);
        Ok(())
    }

    #[test]
    fn test_state_read_write_roundtrip() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir();
        let test_state_path = temp_dir.join("test_snapshot.state1");

        let dummy_state = vec![0xEEu8; 65536]; // 64KB state snapshot
        SaveManager::write_save_state(&test_state_path, &dummy_state)?;

        let loaded =
            SaveManager::read_save_state(&test_state_path).expect("Should load state file");
        assert_eq!(loaded.len(), dummy_state.len());
        assert_eq!(loaded, dummy_state);

        let _ = fs::remove_file(test_state_path);
        Ok(())
    }

    #[test]
    fn test_save_state_disk_methods() -> std::io::Result<()> {
        let test_stem = "TestGame_DiskPersist";
        let slot = 3;
        let test_data = vec![0xAB, 0xCD, 0xEF, 0x01, 0x23];

        // Ensure clean state before test
        let state_path = SaveManager::get_state_path_from_stem(test_stem, slot);
        let _ = fs::remove_file(&state_path);

        assert!(!SaveManager::state_exists_on_disk(test_stem, slot));
        assert!(SaveManager::load_state_from_disk(test_stem, slot).is_none());

        // Write state
        SaveManager::save_state_to_disk(test_stem, slot, &test_data)?;

        // Verify exists and load
        assert!(SaveManager::state_exists_on_disk(test_stem, slot));
        let loaded =
            SaveManager::load_state_from_disk(test_stem, slot).expect("State must load from disk");
        assert_eq!(loaded, test_data);

        // Clean up
        let _ = fs::remove_file(&state_path);
        assert!(!SaveManager::state_exists_on_disk(test_stem, slot));

        Ok(())
    }

    #[test]
    fn test_format_unix_timestamp() {
        assert_eq!(format_unix_timestamp(0), "[ Empty Slot ]");
        // 2026-08-30 07:30 UTC = 1788075000 approx
        let ts = 1788075000;
        let formatted = format_unix_timestamp(ts);
        assert!(formatted.starts_with("2026-08-30"));
    }

    #[test]
    fn test_multi_slot_save_load_and_info() -> std::io::Result<()> {
        let game = "TestPokemonEmerald";

        // Clean up any existing slots
        for i in 1..=5 {
            let _ = SaveManager::delete_slot(game, i);
        }

        let initial_slots = SaveManager::get_slots_info(game);
        assert_eq!(initial_slots.len(), 5);
        for (i, slot) in initial_slots.iter().enumerate() {
            assert_eq!(slot.slot_index, (i + 1) as u8);
            assert!(slot.is_empty);
        }

        // Save into Slot 1 and Slot 3
        let data1 = vec![0x11, 0x22, 0x33, 0x44];
        let data3 = vec![0x99, 0x88, 0x77, 0x66, 0x55];

        let meta1 = SaveManager::save_to_slot(game, 1, &data1)?;
        assert_eq!(meta1.slot_index, 1);
        assert!(!meta1.is_empty);

        let meta3 = SaveManager::save_to_slot(game, 3, &data3)?;
        assert_eq!(meta3.slot_index, 3);
        assert!(!meta3.is_empty);

        // Verify loaded data
        let loaded1 = SaveManager::load_from_slot(game, 1)?;
        assert_eq!(loaded1, data1);

        let loaded3 = SaveManager::load_from_slot(game, 3)?;
        assert_eq!(loaded3, data3);

        // Slot 2 should error with NotFound
        assert!(SaveManager::load_from_slot(game, 2).is_err());

        // Verify get_slots_info
        let updated_slots = SaveManager::get_slots_info(game);
        assert!(!updated_slots[0].is_empty);
        assert!(updated_slots[1].is_empty);
        assert!(!updated_slots[2].is_empty);
        assert!(updated_slots[3].is_empty);
        assert!(updated_slots[4].is_empty);

        // Clean up
        for i in 1..=5 {
            let _ = SaveManager::delete_slot(game, i);
        }
        Ok(())
    }
}
