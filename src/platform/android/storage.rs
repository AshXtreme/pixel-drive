//! Android Storage Access Framework (SAF) & Scoped Storage Pipeline.
//!
//! Provides JNI bridges for Android native document picking (`ACTION_OPEN_DOCUMENT` / `ACTION_GET_CONTENT`),
//! Content URI stream resolution (`content://...`), scoped storage path management,
//! and atomic SRAM/state flushing routines.

use std::fs;
use std::path::{Path, PathBuf};
use log::{info, warn};

use crate::platform::PlatformStorage;
use crate::save::SaveManager;

/// Default directory names under Android scoped storage.
pub const ANDROID_SAVES_SUBDIR: &str = "saves";
pub const ANDROID_STATES_SUBDIR: &str = "states";
pub const ANDROID_CHEATS_SUBDIR: &str = "cheats";

/// Request code used when launching the SAF document picker intent.
pub const SAF_ROM_PICKER_REQUEST_CODE: i32 = 0x524F; // "RO"

/// Android Scoped Storage Manager managing saves, states, and Content URIs.
#[derive(Debug, Clone)]
pub struct AndroidStorage {
    base_dir: PathBuf,
    saves_dir: PathBuf,
    states_dir: PathBuf,
    cheats_dir: PathBuf,
}

impl AndroidStorage {
    /// Creates a new `AndroidStorage` manager rooted at `base_dir`.
    pub fn new(base_dir: PathBuf) -> Self {
        let saves_dir = base_dir.join(ANDROID_SAVES_SUBDIR);
        let states_dir = base_dir.join(ANDROID_STATES_SUBDIR);
        let cheats_dir = base_dir.join(ANDROID_CHEATS_SUBDIR);

        if let Err(err) = fs::create_dir_all(&saves_dir) {
            warn!("Failed to create Android saves directory {:?}: {}", saves_dir, err);
        }
        if let Err(err) = fs::create_dir_all(&states_dir) {
            warn!("Failed to create Android states directory {:?}: {}", states_dir, err);
        }
        if let Err(err) = fs::create_dir_all(&cheats_dir) {
            warn!("Failed to create Android cheats directory {:?}: {}", cheats_dir, err);
        }

        info!(
            "Android Scoped Storage initialized:\n  Base: {}\n  Saves: {}\n  States: {}\n  Cheats: {}",
            base_dir.display(),
            saves_dir.display(),
            states_dir.display(),
            cheats_dir.display()
        );

        Self {
            base_dir,
            saves_dir,
            states_dir,
            cheats_dir,
        }
    }

    /// Returns the base scoped storage directory path.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the dedicated saves directory path.
    pub fn saves_dir(&self) -> &Path {
        &self.saves_dir
    }

    /// Returns the dedicated states directory path.
    pub fn states_dir(&self) -> &Path {
        &self.states_dir
    }

    /// Returns the dedicated cheats directory path.
    pub fn cheats_dir(&self) -> &Path {
        &self.cheats_dir
    }

    /// Derives canonical per-game cheat file path: `<storage_dir>/cheats/<crc32_hex>.cht`.
    pub fn get_cheat_path(&self, rom_crc32: u32) -> PathBuf {
        self.cheats_dir.join(format!("{:08X}.cht", rom_crc32))
    }

    /// Derives canonical cartridge SRAM save path: `<storage_dir>/saves/<game_title>.sav`.
    pub fn get_save_path(&self, rom_title: &str) -> PathBuf {
        let stem = Path::new(rom_title)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rom_title);
        let clean_stem = SaveManager::sanitize_stem(stem);
        self.saves_dir.join(format!("{}.sav", clean_stem))
    }

    /// Derives canonical save state path: `<storage_dir>/states/<game_title>.slot{slot}.state`.
    pub fn get_state_path(&self, rom_title: &str, slot: usize) -> PathBuf {
        let stem = Path::new(rom_title)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(rom_title);
        let clean_stem = SaveManager::sanitize_stem(stem);
        self.states_dir.join(format!("{}.slot{}.state", clean_stem, slot))
    }

    /// Derives slot state path: `<storage_dir>/states/<game_title>/slot_<slot>.state`.
    pub fn get_slot_state_path(&self, game_title: &str, slot: u8) -> PathBuf {
        let clean = SaveManager::sanitize_stem(game_title);
        self.states_dir.join(&clean).join(format!("slot_{}.state", slot))
    }

    /// Derives slot metadata path: `<storage_dir>/states/<game_title>/slot_<slot>.meta`.
    pub fn get_slot_meta_path(&self, game_title: &str, slot: u8) -> PathBuf {
        let clean = SaveManager::sanitize_stem(game_title);
        self.states_dir.join(&clean).join(format!("slot_{}.meta", slot))
    }

    /// Atomically flushes buffer bytes to disk using a staging temporary file.
    pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
        if data.is_empty() {
            return Ok(());
        }

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("save_file");
        let temp_name = format!(".{}.tmp.{}", file_name, std::process::id());
        let temp_path = path.with_file_name(temp_name);

        fs::write(&temp_path, data)?;

        if let Err(err) = fs::rename(&temp_path, path) {
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

    /// Flushes battery-backed SRAM save data on-pause or periodically.
    pub fn flush_sram(&self, rom_title: &str, sram_data: &[u8]) -> std::io::Result<()> {
        if sram_data.is_empty() {
            return Ok(());
        }
        let save_path = self.get_save_path(rom_title);
        Self::write_atomic(&save_path, sram_data)?;
        info!("Flushed SRAM save ({} bytes) -> {:?}", sram_data.len(), save_path);
        Ok(())
    }

    /// Saves snapshot data to a designated slot (1..=5) under scoped storage.
    pub fn save_to_slot(&self, game_title: &str, slot: u8, data: &[u8]) -> std::io::Result<crate::save::SlotMetadata> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = self.get_slot_state_path(game_title, clamped_slot);
        let meta_path = self.get_slot_meta_path(game_title, clamped_slot);

        Self::write_atomic(&state_path, data)?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let meta = crate::save::SlotMetadata {
            slot_index: clamped_slot,
            timestamp: ts,
            formatted_time: crate::save::format_unix_timestamp(ts),
            is_empty: false,
        };

        if let Ok(meta_bytes) = bincode::serialize(&meta) {
            let _ = Self::write_atomic(&meta_path, &meta_bytes);
        }

        info!(
            "Saved Android state slot {} for '{}' ({} bytes, timestamp {})",
            clamped_slot, game_title, data.len(), meta.formatted_time
        );
        Ok(meta)
    }

    /// Loads snapshot data from a designated slot (1..=5) under scoped storage.
    pub fn load_from_slot(&self, game_title: &str, slot: u8) -> Result<Vec<u8>, std::io::Error> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = self.get_slot_state_path(game_title, clamped_slot);
        if !state_path.exists() {
            let legacy_path = self.get_state_path(game_title, clamped_slot as usize);
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

    /// Queries metadata for all 5 slots of a given game under scoped storage.
    pub fn get_slots_info(&self, game_title: &str) -> [crate::save::SlotMetadata; 5] {
        let mut slots = [
            crate::save::SlotMetadata::empty(1),
            crate::save::SlotMetadata::empty(2),
            crate::save::SlotMetadata::empty(3),
            crate::save::SlotMetadata::empty(4),
            crate::save::SlotMetadata::empty(5),
        ];

        for i in 1..=5 {
            let state_path = self.get_slot_state_path(game_title, i);
            let meta_path = self.get_slot_meta_path(game_title, i);
            let legacy_path = self.get_state_path(game_title, i as usize);

            if state_path.exists() || legacy_path.exists() {
                if let Ok(meta_bytes) = fs::read(&meta_path) {
                    if let Ok(meta) = bincode::deserialize::<crate::save::SlotMetadata>(&meta_bytes) {
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
                slots[(i - 1) as usize] = crate::save::SlotMetadata {
                    slot_index: i,
                    timestamp: ts,
                    formatted_time: crate::save::format_unix_timestamp(ts),
                    is_empty: false,
                };
            }
        }
        slots
    }

    /// Deletes a save state slot and its associated metadata under scoped storage.
    pub fn delete_slot(&self, game_title: &str, slot: u8) -> std::io::Result<()> {
        let clamped_slot = slot.clamp(1, 5);
        let state_path = self.get_slot_state_path(game_title, clamped_slot);
        let meta_path = self.get_slot_meta_path(game_title, clamped_slot);
        if state_path.exists() {
            let _ = fs::remove_file(state_path);
        }
        if meta_path.exists() {
            let _ = fs::remove_file(meta_path);
        }
        Ok(())
    }
}

impl PlatformStorage for AndroidStorage {
    fn get_save_path(&self, rom_identifier: &str) -> PathBuf {
        self.get_save_path(rom_identifier)
    }

    fn get_state_path(&self, rom_identifier: &str, slot: usize) -> PathBuf {
        self.get_state_path(rom_identifier, slot)
    }

    fn get_cheat_path(&self, rom_crc32: u32) -> PathBuf {
        self.get_cheat_path(rom_crc32)
    }

    fn load_save(&self, rom_identifier: &str) -> Option<Vec<u8>> {
        let path = self.get_save_path(rom_identifier);
        if !path.exists() {
            return None;
        }
        match fs::read(&path) {
            Ok(bytes) => {
                info!("Loaded Android save {:?} ({} bytes)", path, bytes.len());
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read Android save {:?}: {}", path, err);
                None
            }
        }
    }

    fn write_save(&self, rom_identifier: &str, data: &[u8]) -> std::io::Result<()> {
        self.flush_sram(rom_identifier, data)
    }

    fn load_state(&self, rom_identifier: &str, slot: usize) -> Option<Vec<u8>> {
        let path = self.get_state_path(rom_identifier, slot);
        if !path.exists() {
            return None;
        }
        match fs::read(&path) {
            Ok(bytes) => {
                info!("Loaded Android state snapshot {:?} ({} bytes)", path, bytes.len());
                Some(bytes)
            }
            Err(err) => {
                warn!("Failed to read Android state {:?}: {}", path, err);
                None
            }
        }
    }

    fn write_state(&self, rom_identifier: &str, slot: usize, data: &[u8]) -> std::io::Result<()> {
        let path = self.get_state_path(rom_identifier, slot);
        Self::write_atomic(&path, data)?;
        info!("Saved Android state snapshot ({} bytes) -> {:?}", data.len(), path);
        Ok(())
    }

    fn state_exists(&self, rom_identifier: &str, slot: usize) -> bool {
        self.get_state_path(rom_identifier, slot).exists()
    }

    fn read_rom_bytes(&self, uri_or_path: &str) -> std::io::Result<Vec<u8>> {
        if uri_or_path.starts_with("content://") {
            // Content URI resolution requires JNI bridge via Activity ContentResolver
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Content URI '{}' requires Activity JNI ContentResolver bridge", uri_or_path),
            ))
        } else {
            let clean_path = uri_or_path.strip_prefix("file://").unwrap_or(uri_or_path);
            fs::read(clean_path)
        }
    }
}

// ---------------------------------------------------------------------------
// JNI Storage Access Framework (SAF) Bridge
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
pub mod jni_bridge {
    use super::*;
    use jni::objects::{JObject, JString, JValue};
    use jni::JavaVM;

    /// Launches the Android SAF native document picker filtering for `.gb`, `.gbc`, `.gba`, and `.zip`.
    pub fn launch_saf_rom_picker(
        vm: &JavaVM,
        activity: &JObject,
        request_code: i32,
    ) -> Result<(), String> {
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| format!("Failed to attach JNI thread: {:?}", e))?;

        // 1. Create Intent("android.intent.action.OPEN_DOCUMENT")
        let action_str = env
            .new_string("android.intent.action.OPEN_DOCUMENT")
            .map_err(|e| format!("JNI string error: {:?}", e))?;
        let intent_class = env
            .find_class("android/content/Intent")
            .map_err(|e| format!("Intent class not found: {:?}", e))?;
        let intent = env
            .new_object(
                &intent_class,
                "(Ljava/lang/String;)V",
                &[JValue::Object(&action_str)],
            )
            .map_err(|e| format!("Failed to create Intent: {:?}", e))?;

        // 2. intent.addCategory(Intent.CATEGORY_OPENABLE)
        let category_str = env
            .new_string("android.intent.category.OPENABLE")
            .map_err(|e| format!("JNI string error: {:?}", e))?;
        env.call_method(
            &intent,
            "addCategory",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&category_str)],
        )
        .map_err(|e| format!("Failed to addCategory: {:?}", e))?;

        // 3. intent.setType("*/*")
        let mime_type_str = env
            .new_string("*/*")
            .map_err(|e| format!("JNI string error: {:?}", e))?;
        env.call_method(
            &intent,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[JValue::Object(&mime_type_str)],
        )
        .map_err(|e| format!("Failed to setType: {:?}", e))?;

        // 4. intent.putExtra(Intent.EXTRA_MIME_TYPES, ["application/octet-stream", "application/zip", ...])
        let extra_mime_types_key = env
            .new_string("android.intent.extra.MIME_TYPES")
            .map_err(|e| format!("JNI string error: {:?}", e))?;

        let mime_filters = [
            "application/octet-stream",
            "application/zip",
            "application/x-zip-compressed",
            "application/x-gameboy-rom",
            "application/x-gba-rom",
        ];

        let string_class = env
            .find_class("java/lang/String")
            .map_err(|e| format!("String class not found: {:?}", e))?;
        let mime_array = env
            .new_object_array(mime_filters.len() as i32, &string_class, JObject::null())
            .map_err(|e| format!("Failed to create MIME object array: {:?}", e))?;

        for (i, &mime) in mime_filters.iter().enumerate() {
            let j_mime = env
                .new_string(mime)
                .map_err(|e| format!("JNI string error: {:?}", e))?;
            env.set_object_array_element(&mime_array, i as i32, &j_mime)
                .map_err(|e| format!("Failed to set MIME array element: {:?}", e))?;
        }

        env.call_method(
            &intent,
            "putExtra",
            "(Ljava/lang/String;[Ljava/lang/String;)Landroid/content/Intent;",
            &[
                JValue::Object(&extra_mime_types_key),
                JValue::Object(&mime_array),
            ],
        )
        .map_err(|e| format!("Failed to putExtra EXTRA_MIME_TYPES: {:?}", e))?;

        // 5. activity.startActivityForResult(intent, request_code)
        env.call_method(
            activity,
            "startActivityForResult",
            "(Landroid/content/Intent;I)V",
            &[JValue::Object(&intent), JValue::Int(request_code)],
        )
        .map_err(|e| format!("Failed to startActivityForResult: {:?}", e))?;

        info!("SAF document picker intent launched successfully (Request Code: {:#X})", request_code);
        Ok(())
    }

    /// Reads binary ROM bytes from an Android `content://...` Content URI using `ContentResolver.openInputStream`.
    pub fn read_bytes_from_content_uri(
        vm: &JavaVM,
        activity: &JObject,
        uri_str: &str,
    ) -> Result<Vec<u8>, String> {
        let mut env = vm
            .attach_current_thread()
            .map_err(|e| format!("Failed to attach JNI thread: {:?}", e))?;

        // 1. Get ContentResolver: activity.getContentResolver()
        let content_resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .map_err(|e| format!("Failed to get ContentResolver: {:?}", e))?
            .l()
            .map_err(|e| format!("ContentResolver object error: {:?}", e))?;

        // 2. Parse URI: Uri.parse(uri_str)
        let j_uri_str = env
            .new_string(uri_str)
            .map_err(|e| format!("JNI string error: {:?}", e))?;
        let uri_class = env
            .find_class("android/net/Uri")
            .map_err(|e| format!("Uri class not found: {:?}", e))?;
        let uri = env
            .call_static_method(
                &uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&j_uri_str)],
            )
            .map_err(|e| format!("Failed to parse Uri: {:?}", e))?
            .l()
            .map_err(|e| format!("Uri object error: {:?}", e))?;

        // 3. Open InputStream: contentResolver.openInputStream(uri)
        let input_stream = env
            .call_method(
                &content_resolver,
                "openInputStream",
                "(Landroid/net/Uri;)Ljava/io/InputStream;",
                &[JValue::Object(&uri)],
            )
            .map_err(|e| format!("Failed to openInputStream: {:?}", e))?
            .l()
            .map_err(|e| format!("InputStream object error: {:?}", e))?;

        if input_stream.is_null() {
            return Err(format!("ContentResolver returned null InputStream for URI: {}", uri_str));
        }

        // 4. Read bytes from InputStream in chunks (64KB buffer)
        let chunk_size = 64 * 1024;
        let byte_array = env
            .new_byte_array(chunk_size as i32)
            .map_err(|e| format!("Failed to allocate byte array: {:?}", e))?;

        let mut result = Vec::new();
        let mut temp_buf = vec![0i8; chunk_size];

        loop {
            let bytes_read = env
                .call_method(
                    &input_stream,
                    "read",
                    "([B)I",
                    &[JValue::Object(&byte_array)],
                )
                .map_err(|e| format!("InputStream.read() failed: {:?}", e))?
                .i()
                .map_err(|e| format!("read() return error: {:?}", e))?;

            if bytes_read <= 0 {
                break;
            }

            env.get_byte_array_region(&byte_array, 0, &mut temp_buf[..bytes_read as usize])
                .map_err(|e| format!("get_byte_array_region failed: {:?}", e))?;

            let u8_slice: &[u8] = unsafe {
                std::slice::from_raw_parts(temp_buf.as_ptr() as *const u8, bytes_read as usize)
            };
            result.extend_from_slice(u8_slice);
        }

        // 5. Close InputStream
        let _ = env.call_method(&input_stream, "close", "()V", &[]);

        info!("Successfully read {} bytes from Content URI '{}'", result.len(), uri_str);
        Ok(result)
    }

    /// Resolves the app scoped storage directory targeting `context.getExternalFilesDir(null)` or `context.getFilesDir()`.
    pub fn get_app_storage_dir(vm: &JavaVM, activity: &JObject) -> Option<PathBuf> {
        let mut env = vm.attach_current_thread().ok()?;

        // Try context.getExternalFilesDir(null)
        let ext_dir_obj = env
            .call_method(
                activity,
                "getExternalFilesDir",
                "(Ljava/lang/String;)Ljava/io/File;",
                &[JValue::Object(&JObject::null())],
            )
            .ok()?
            .l()
            .ok()?;

        let target_file_obj = if !ext_dir_obj.is_null() {
            ext_dir_obj
        } else {
            // Fallback to internal context.getFilesDir()
            env.call_method(activity, "getFilesDir", "()Ljava/io/File;", &[])
                .ok()?
                .l()
                .ok()?
        };

        if target_file_obj.is_null() {
            return None;
        }

        let abs_path_str: JString = env
            .call_method(&target_file_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .ok()?
            .l()
            .ok()?
            .into();

        let path_rust: String = env.get_string(&abs_path_str).ok()?.into();
        Some(PathBuf::from(path_rust))
    }

    /// Queries the display name (file name) of a Content URI using `OpenableColumns.DISPLAY_NAME`.
    pub fn query_uri_display_name(
        vm: &JavaVM,
        activity: &JObject,
        uri_str: &str,
    ) -> Option<String> {
        let mut env = vm.attach_current_thread().ok()?;

        let content_resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .ok()?
            .l()
            .ok()?;

        let j_uri_str = env.new_string(uri_str).ok()?;
        let uri_class = env.find_class("android/net/Uri").ok()?;
        let uri = env
            .call_static_method(
                &uri_class,
                "parse",
                "(Ljava/lang/String;)Landroid/net/Uri;",
                &[JValue::Object(&j_uri_str)],
            )
            .ok()?
            .l()
            .ok()?;

        // Projection: [OpenableColumns.DISPLAY_NAME] ("_display_name")
        let display_name_col = env.new_string("_display_name").ok()?;
        let string_class = env.find_class("java/lang/String").ok()?;
        let projection = env
            .new_object_array(1, &string_class, JObject::null())
            .ok()?;
        let _ = env.set_object_array_element(&projection, 0, &display_name_col);

        let cursor = env
            .call_method(
                &content_resolver,
                "query",
                "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
                &[
                    JValue::Object(&uri),
                    JValue::Object(&projection),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                    JValue::Object(&JObject::null()),
                ],
            )
            .ok()?
            .l()
            .ok()?;

        if cursor.is_null() {
            return None;
        }

        let has_first = env
            .call_method(&cursor, "moveToFirst", "()Z", &[])
            .ok()?
            .z()
            .ok()?;

        if !has_first {
            let _ = env.call_method(&cursor, "close", "()V", &[]);
            return None;
        }

        let name_col_idx = env
            .call_method(
                &cursor,
                "getColumnIndexOrThrow",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&display_name_col)],
            )
            .ok()?
            .i()
            .ok()?;

        let display_name_jstr: JString = env
            .call_method(
                &cursor,
                "getString",
                "(I)Ljava/lang/String;",
                &[JValue::Int(name_col_idx)],
            )
            .ok()?
            .l()
            .ok()?
            .into();

        let _ = env.call_method(&cursor, "close", "()V", &[]);

        let name_rust: String = env.get_string(&display_name_jstr).ok()?.into();
        Some(name_rust)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_android_scoped_storage_path_construction() {
        let base_dir = PathBuf::from("/data/user/0/com.pixeldrive.emulator/files");
        let storage = AndroidStorage::new(base_dir.clone());

        let save_path = storage.get_save_path("Pokemon FireRed (USA).gba");
        assert_eq!(
            save_path,
            base_dir.join("saves/Pokemon FireRed (USA).sav")
        );

        let state_path = storage.get_state_path("Pokemon FireRed (USA).gba", 0);
        assert_eq!(
            state_path,
            base_dir.join("states/Pokemon FireRed (USA).slot0.state")
        );

        let state3_path = storage.get_state_path("Pokemon FireRed (USA).gba", 3);
        assert_eq!(
            state3_path,
            base_dir.join("states/Pokemon FireRed (USA).slot3.state")
        );
    }

    #[test]
    fn test_android_atomic_write_and_flush() -> std::io::Result<()> {
        let temp_dir = std::env::temp_dir().join("android_scoped_storage_test");
        let storage = AndroidStorage::new(temp_dir.clone());

        let game_title = "Metroid_Fusion";
        let dummy_sram = vec![0xDE, 0xAD, 0xBE, 0xEF];

        storage.flush_sram(game_title, &dummy_sram)?;
        assert!(storage.get_save_path(game_title).exists());

        let loaded = storage.load_save(game_title).expect("Save must be loadable");
        assert_eq!(loaded, dummy_sram);

        let dummy_state = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        storage.write_state(game_title, 1, &dummy_state)?;
        assert!(storage.state_exists(game_title, 1));

        let loaded_state = storage.load_state(game_title, 1).expect("State must load");
        assert_eq!(loaded_state, dummy_state);

        let _ = fs::remove_dir_all(temp_dir);
        Ok(())
    }
}
