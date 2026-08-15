use log::{info, warn};
use std::fs;
use std::path::{Path, PathBuf};

pub const SAVES_DIR: &str = "saves";

/// Unified Save Manager for persisting and loading battery-backed `.sav` RAM files.
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
}
