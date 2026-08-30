//! Platform-specific abstractions, storage interfaces, and lifecycle entrypoints for PixelDrive.

use std::path::PathBuf;
use crate::audio::AudioProducer;
use crate::save::SaveManager;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(target_os = "android")]
pub use android::{AndroidAudioPlayer, AndroidHaptics, AndroidStorage};

/// Unified platform abstraction for tactile haptic vibration feedback.
pub trait PlatformHaptics {
    /// Dispatches standard tactile click vibration impulse (~20ms).
    fn vibrate_click(&self);

    /// Dispatches custom vibration with duration (ms) and amplitude (0-255).
    fn vibrate_custom(&self, duration_ms: u64, amplitude: u8);

    /// Sets whether tactile haptic feedback is enabled.
    fn set_enabled(&self, enabled: bool);

    /// Returns whether tactile haptic feedback is enabled.
    fn is_enabled(&self) -> bool;
}

/// No-op desktop haptics implementation for non-mobile platforms.
#[derive(Debug, Clone, Default)]
pub struct DesktopHaptics {
    enabled: bool,
}

impl DesktopHaptics {
    pub fn new() -> Self {
        Self { enabled: false }
    }
}

impl PlatformHaptics for DesktopHaptics {
    fn vibrate_click(&self) {}
    fn vibrate_custom(&self, _duration_ms: u64, _amplitude: u8) {}
    fn set_enabled(&self, _enabled: bool) {}
    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Unified platform abstraction for host audio streams (Desktop cpal vs. Android AAudio/Oboe).
pub trait PlatformAudio {
    /// Returns the thread-safe audio sample producer for pushing samples from emulator cores.
    fn producer(&self) -> AudioProducer;

    /// Returns the active output audio sample rate in Hz (e.g. 48000 or 44100).
    fn sample_rate(&self) -> u32;

    /// Pauses audio playback / shuts down hardware stream to save power.
    fn pause(&mut self);

    /// Resumes audio playback stream with fresh ring buffer state.
    fn resume(&mut self);

    /// Sets whether audio stream is muted.
    fn set_muted(&self, muted: bool);

    /// Returns true if audio stream is currently muted.
    fn is_muted(&self) -> bool;

    /// Sets master volume (0.0 to 1.0).
    fn set_volume(&self, volume: f32);

    /// Returns master volume (0.0 to 1.0).
    fn volume(&self) -> f32;

    /// Sets fast-forward state (drops audio frames during fast-forward).
    fn set_fast_forward(&self, enabled: bool);
}

/// Unified platform abstraction for storage operations, save file derivation, and atomic I/O.
pub trait PlatformStorage {
    /// Resolves canonical cartridge SRAM save path (`<storage_dir>/saves/<game_title>.sav`).
    fn get_save_path(&self, rom_identifier: &str) -> PathBuf;

    /// Resolves canonical save state path (`<storage_dir>/states/<game_title>.slot{slot}.state`).
    fn get_state_path(&self, rom_identifier: &str, slot: usize) -> PathBuf;

    /// Reads battery-backed SRAM save data for the given game.
    fn load_save(&self, rom_identifier: &str) -> Option<Vec<u8>>;

    /// Atomically flushes battery-backed SRAM save data to disk.
    fn write_save(&self, rom_identifier: &str, data: &[u8]) -> std::io::Result<()>;

    /// Reads a real-time save state snapshot from disk.
    fn load_state(&self, rom_identifier: &str, slot: usize) -> Option<Vec<u8>>;

    /// Atomically writes a real-time save state snapshot to disk.
    fn write_state(&self, rom_identifier: &str, slot: usize, data: &[u8]) -> std::io::Result<()>;

    /// Checks if a save state exists for the given slot.
    fn state_exists(&self, rom_identifier: &str, slot: usize) -> bool;

    /// Reads ROM binary content from a filesystem path or URI (`file://`, `content://`, or relative/absolute path).
    fn read_rom_bytes(&self, uri_or_path: &str) -> std::io::Result<Vec<u8>>;
}

/// Desktop filesystem-backed storage implementation using `./saves/` directory.
#[derive(Debug, Clone)]
pub struct DesktopStorage {
    base_dir: PathBuf,
}

impl Default for DesktopStorage {
    fn default() -> Self {
        Self::new(PathBuf::from("saves"))
    }
}

impl DesktopStorage {
    pub fn new(base_dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&base_dir);
        let _ = std::fs::create_dir_all(base_dir.join("states"));
        Self { base_dir }
    }

    pub fn sanitize_stem(raw: &str) -> String {
        let stem = std::path::Path::new(raw)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(raw);
        SaveManager::sanitize_stem(stem)
    }

    pub fn save_to_slot(&self, game_title: &str, slot: u8, data: &[u8]) -> std::io::Result<crate::save::SlotMetadata> {
        SaveManager::save_to_slot(game_title, slot, data)
    }

    pub fn load_from_slot(&self, game_title: &str, slot: u8) -> Result<Vec<u8>, std::io::Error> {
        SaveManager::load_from_slot(game_title, slot)
    }

    pub fn get_slots_info(&self, game_title: &str) -> [crate::save::SlotMetadata; 5] {
        SaveManager::get_slots_info(game_title)
    }
}

impl PlatformStorage for DesktopStorage {
    fn get_save_path(&self, rom_identifier: &str) -> PathBuf {
        let stem = Self::sanitize_stem(rom_identifier);
        self.base_dir.join(format!("{}.sav", stem))
    }

    fn get_state_path(&self, rom_identifier: &str, slot: usize) -> PathBuf {
        let stem = Self::sanitize_stem(rom_identifier);
        self.base_dir.join(format!("{}.state{}", stem, slot))
    }

    fn load_save(&self, rom_identifier: &str) -> Option<Vec<u8>> {
        let path = self.get_save_path(rom_identifier);
        SaveManager::load_save_file(&path)
    }

    fn write_save(&self, rom_identifier: &str, data: &[u8]) -> std::io::Result<()> {
        let path = self.get_save_path(rom_identifier);
        SaveManager::write_save_file(&path, data)
    }

    fn load_state(&self, rom_identifier: &str, slot: usize) -> Option<Vec<u8>> {
        let path = self.get_state_path(rom_identifier, slot);
        SaveManager::read_save_state(&path)
    }

    fn write_state(&self, rom_identifier: &str, slot: usize, data: &[u8]) -> std::io::Result<()> {
        let path = self.get_state_path(rom_identifier, slot);
        SaveManager::write_save_state(&path, data)
    }

    fn state_exists(&self, rom_identifier: &str, slot: usize) -> bool {
        let path = self.get_state_path(rom_identifier, slot);
        path.exists()
    }

    fn read_rom_bytes(&self, uri_or_path: &str) -> std::io::Result<Vec<u8>> {
        let clean_path = uri_or_path.strip_prefix("file://").unwrap_or(uri_or_path);
        std::fs::read(clean_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_storage_paths() {
        let storage = DesktopStorage::new(PathBuf::from("test_saves"));
        let save_path = storage.get_save_path("Pokemon_Emerald.gba");
        assert_eq!(save_path, PathBuf::from("test_saves/Pokemon_Emerald.sav"));

        let state_path = storage.get_state_path("Pokemon_Emerald.gba", 2);
        assert_eq!(state_path, PathBuf::from("test_saves/Pokemon_Emerald.state2"));
    }

    #[test]
    fn test_desktop_storage_roundtrip() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join("pixeldrive_storage_test");
        let storage = DesktopStorage::new(temp_dir.clone());

        let game = "Zelda_MinishCap";
        let save_data = vec![0x11, 0x22, 0x33, 0x44];
        storage.write_save(game, &save_data)?;

        let loaded_save = storage.load_save(game).expect("Save must load");
        assert_eq!(loaded_save, save_data);

        let state_data = vec![0x99, 0x88, 0x77];
        storage.write_state(game, 0, &state_data)?;
        assert!(storage.state_exists(game, 0));
        let loaded_state = storage.load_state(game, 0).expect("State must load");
        assert_eq!(loaded_state, state_data);

        let _ = std::fs::remove_dir_all(temp_dir);
        Ok(())
    }
}
