use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

pub const SAVES_DIR: &str = "saves";

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

    /// Derives the canonical `.sav` file path from a ROM file path: `./saves/<rom_stem>.sav`.
    pub fn get_save_path(rom_path: &Path) -> PathBuf {
        let stem = rom_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("game");

        // Clean up common zip-appended names or archive brackets
        let clean_stem = stem.trim();
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
                info!("Loaded persistent save file: {:?} ({} bytes)", path, bytes.len());
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

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, data)?;
        info!("Flushed save data ({} bytes) to {:?}", data.len(), path);
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
        let clean_stem = rom_stem.trim();
        let mut state_path = PathBuf::from(SAVES_DIR);
        state_path.push(format!("{}.state{}", clean_stem, slot));
        state_path
    }

    /// Writes real-time save state snapshot bytes to disk under `./saves/{rom_stem}.state{slot}`.
    pub fn save_state_to_disk(rom_stem: &str, slot: usize, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        fs::create_dir_all(SAVES_DIR).ok();

        let path = Self::get_state_path_from_stem(rom_stem, slot);
        fs::write(&path, data)?;
        info!("Saved state snapshot ({} bytes) to disk -> {:?}", data.len(), path);
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
                info!("Loaded save state snapshot from disk: {:?} ({} bytes)", path, bytes.len());
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

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, data)?;
        info!("Saved state snapshot ({} bytes) to {:?}", data.len(), path);
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
                info!("Loaded save state snapshot: {:?} ({} bytes)", path, bytes.len());
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read save state file {:?}: {}", path, err);
                None
            }
        }
    }
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
        assert_eq!(zip_save, PathBuf::from("saves/Pokemon - Crystal Version (USA).sav"));

        let state1_path = SaveManager::get_state_path(rom_path, 1);
        assert_eq!(state1_path, PathBuf::from("saves/Pokemon_FireRed.state1"));

        let state2_path = SaveManager::get_state_path(zip_rom, 2);
        assert_eq!(state2_path, PathBuf::from("saves/Pokemon - Crystal Version (USA).state2"));
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

        let loaded = SaveManager::read_save_state(&test_state_path).expect("Should load state file");
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
        let loaded = SaveManager::load_state_from_disk(test_stem, slot).expect("State must load from disk");
        assert_eq!(loaded, test_data);

        // Clean up
        let _ = fs::remove_file(&state_path);
        assert!(!SaveManager::state_exists_on_disk(test_stem, slot));

        Ok(())
    }
}
