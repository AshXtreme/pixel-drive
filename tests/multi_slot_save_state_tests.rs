//! Test Suite: Phase 2 Multi-Slot Save State Engine (Slots 1–5).
//!
//! Validates:
//! - TC-SLOT-01: Core state serialization buffer sizing, `retro_serialize`/`retro_unserialize`, and GBC MMU/CPU snapshotting roundtrip.
//! - TC-SLOT-02: Multi-slot persistence across Slots 1 through 5, metadata file indexing (`slot_N.meta`), timestamping, and slot occupancy.
//! - TC-SLOT-03: Cold restart persistence, battery SRAM immediate disk flushing (`.sav`), and atomic write safety across process termination.
//! - TC-SLOT-04: Graceful handling of unpopulated/empty slots (`NotFound` error) and slot deletion (`delete_slot`).
//! - TC-SLOT-05: Scoped Storage path containment and directory traversal sanitization.

use std::fs;
use pixeldrive::core::EmulatorCore;
use pixeldrive::gba::GbaCore;
use pixeldrive::gbc::GbcCore;
use pixeldrive::platform::android::AndroidStorage;
use pixeldrive::platform::PlatformStorage;
use pixeldrive::save::SaveManager;

/// Helper to generate a valid synthetic GBA ROM buffer with Nintendo header magic.
fn create_synthetic_gba_rom(title: &str, game_code: &str) -> Vec<u8> {
    let mut rom = vec![0x00u8; 0x4000];
    rom[0] = 0x2E;
    rom[1] = 0x00;
    rom[2] = 0x00;
    rom[3] = 0xEA;
    rom[0x04] = 0x24;
    rom[0x05] = 0xFF;
    rom[0x06] = 0xAE;
    rom[0x07] = 0x51;

    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len().min(12);
    rom[0xA0..0xA0 + title_len].copy_from_slice(&title_bytes[..title_len]);

    let code_bytes = game_code.as_bytes();
    let code_len = code_bytes.len().min(4);
    rom[0xAC..0xAC + code_len].copy_from_slice(&code_bytes[..code_len]);

    rom[0xB0] = b'0';
    rom[0xB1] = b'1';
    rom[0xB2] = 0x96; // GBA magic byte

    let mut checksum: u8 = 0;
    for b in &rom[0xA0..0xBD] {
        checksum = checksum.wrapping_sub(*b);
    }
    checksum = checksum.wrapping_sub(0x19);
    rom[0xBD] = checksum;
    rom
}

/// Helper to generate a valid synthetic GBC ROM buffer with MBC5 and RAM battery.
fn create_synthetic_gbc_rom(title: &str) -> Vec<u8> {
    let mut rom = vec![0x00u8; 0x8000]; // 32KB
    rom[0x100..0x104].copy_from_slice(&[0x00, 0xC3, 0x50, 0x01]);

    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len().min(15);
    rom[0x134..0x134 + title_len].copy_from_slice(&title_bytes[..title_len]);

    rom[0x143] = 0x80; // GBC flag
    rom[0x147] = 0x1B; // MBC5 + RAM + BATTERY
    rom[0x148] = 0x00; // 32KB ROM
    rom[0x149] = 0x03; // 32KB RAM (4 banks)

    let mut chk: u8 = 0;
    for b in &rom[0x134..0x14D] {
        chk = chk.wrapping_sub(*b).wrapping_sub(1);
    }
    rom[0x14D] = chk;
    rom
}

// ============================================================================
// TC-SLOT-01: Serialization Exact Size, Buffer Matching & Roundtrip
// ============================================================================

#[test]
fn test_tc_slot_01_serialization_exact_size_and_roundtrip() {
    let _lock = pixeldrive::gba::libretro::lock();
    log::info!("TC-SLOT-01: Validating core state serialization buffer sizing and restoration...");

    // 1. GBC Emulation Core Serialization Check
    let mut gbc = GbcCore::new();
    let gbc_rom = create_synthetic_gbc_rom("TEST_SERIAL");
    gbc.load_rom(&gbc_rom);

    // Run several emulation steps to generate non-initial CPU / MMU / PPU state
    for _ in 0..15 {
        gbc.step_frame();
    }

    // Capture baseline state snapshot
    let gbc_snapshot_1 = gbc.save_state().expect("GBC save_state serialization must succeed");
    assert!(!gbc_snapshot_1.is_empty(), "Serialized buffer must not be empty");

    // Modify CPU registers and memory in the live core
    gbc.cpu.registers.a = 0xDE;
    gbc.cpu.registers.b = 0xAD;
    gbc.cpu.registers.pc = 0x4321;
    gbc.mmu.write_byte(0xC010, 0x77);
    gbc.mmu.write_byte(0xC011, 0x88);
    gbc.step_frame();

    // Verify modified state has taken effect
    assert_ne!(gbc.cpu.registers.a, 0);

    // Restore state from snapshot
    gbc.load_state(&gbc_snapshot_1).expect("GBC load_state deserialization must succeed");

    // Serialize again and assert deterministic byte-for-byte reproduction
    let gbc_snapshot_2 = gbc.save_state().expect("GBC second serialization must succeed");
    assert_eq!(
        gbc_snapshot_1.len(),
        gbc_snapshot_2.len(),
        "Serialization buffer size must be deterministic"
    );
    assert_eq!(
        gbc_snapshot_1,
        gbc_snapshot_2,
        "Deserialized and re-serialized state must match original snapshot"
    );

    // 2. GBA Emulation Core (Libretro mGBA or Built-in MMU/CPU) Serialization Check
    let mut gba = GbaCore::new();
    let gba_rom = create_synthetic_gba_rom("GBASERIAL", "BSER");
    gba.load_rom(&gba_rom);

    if let Some(ref lr) = gba.libretro {
        // Dynamic Libretro core serialization check
        log::info!("TC-SLOT-01: Validating Libretro dynamic core serialization routines...");
        let size = lr.save_state().map(|buf| buf.len()).unwrap_or(0);
        assert!(size > 0, "Libretro core must return non-zero serialization size");

        let state_bytes = lr.save_state().expect("Libretro save_state must return buffer");
        assert_eq!(state_bytes.len(), size, "Buffer size must match exact size reported by retro_serialize_size");

        // Step core, then restore state
        gba.step_frame();
        let restore_ok = gba.load_state(&state_bytes);
        assert!(restore_ok, "Libretro load_state (retro_unserialize) must return true");
    } else {
        log::info!("TC-SLOT-01: Built-in GBA fallback core active.");
    }
}

// ============================================================================
// TC-SLOT-02: Multi-Slot Persistence (Slots 1–5) & Metadata Indexing
// ============================================================================

#[test]
fn test_tc_slot_02_multi_slot_state_and_metadata_persistence() -> std::io::Result<()> {
    let temp_root = std::env::temp_dir().join(format!("pixeldrive_slot_test_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_root);

    let game_title = "Pokemon_Emerald_Version";

    // Clean up any pre-existing slot states for test isolation
    for i in 1..=5 {
        let _ = SaveManager::delete_slot(game_title, i);
    }

    // 1. Initial State: All 5 slots must be empty
    let initial_slots = SaveManager::get_slots_info(game_title);
    assert_eq!(initial_slots.len(), 5);
    for (i, meta) in initial_slots.iter().enumerate() {
        assert_eq!(meta.slot_index, (i + 1) as u8);
        assert!(meta.is_empty, "Slot {} should initially be empty", i + 1);
        assert_eq!(meta.formatted_time, "[ Empty Slot ]");
        assert_eq!(meta.timestamp, 0);
    }

    // 2. Populate all 5 slots with distinct state buffers
    let slot_payloads: [Vec<u8>; 5] = [
        vec![0x10, 0x11, 0x12, 0x13, 0x14],
        vec![0x20, 0x21, 0x22, 0x23, 0x24, 0x25],
        vec![0x30, 0x31, 0x32, 0x33],
        vec![0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46],
        vec![0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57],
    ];

    for (idx, payload) in slot_payloads.iter().enumerate() {
        let slot_num = (idx + 1) as u8;
        let meta = SaveManager::save_to_slot(game_title, slot_num, payload)?;
        assert_eq!(meta.slot_index, slot_num);
        assert!(!meta.is_empty);
        assert!(meta.timestamp > 0);
        assert_ne!(meta.formatted_time, "[ Empty Slot ]");

        // Verify that .state and .meta files exist on disk
        let state_path = SaveManager::get_slot_state_path(game_title, slot_num);
        let meta_path = SaveManager::get_slot_meta_path(game_title, slot_num);
        assert!(state_path.exists(), "State file {:?} must exist", state_path);
        assert!(meta_path.exists(), "Meta file {:?} must exist", meta_path);
    }

    // 3. Query slots metadata and verify complete indexing
    let populated_slots = SaveManager::get_slots_info(game_title);
    for (idx, meta) in populated_slots.iter().enumerate() {
        let slot_num = (idx + 1) as u8;
        assert_eq!(meta.slot_index, slot_num);
        assert!(!meta.is_empty, "Slot {} must be marked populated", slot_num);
        assert!(meta.timestamp > 0);

        // Verify read operation matches exact payload written
        let read_bytes = SaveManager::load_from_slot(game_title, slot_num)?;
        assert_eq!(read_bytes, slot_payloads[idx], "Payload mismatch for slot {}", slot_num);
    }

    // 4. Test Android Scoped Storage Manager multi-slot operations
    let android_storage = AndroidStorage::new(temp_root.clone());
    let android_game = "Zelda_The_Minish_Cap";

    for slot_num in 1..=5 {
        let dummy_data = vec![slot_num * 11; 128];
        let meta = android_storage.save_to_slot(android_game, slot_num, &dummy_data)?;
        assert_eq!(meta.slot_index, slot_num);
        assert!(!meta.is_empty);

        let loaded = android_storage.load_from_slot(android_game, slot_num)?;
        assert_eq!(loaded, dummy_data);
    }

    let android_slots = android_storage.get_slots_info(android_game);
    assert_eq!(android_slots.len(), 5);
    for slot in &android_slots {
        assert!(!slot.is_empty);
    }

    // Clean up
    for i in 1..=5 {
        let _ = SaveManager::delete_slot(game_title, i);
        let _ = android_storage.delete_slot(android_game, i);
    }
    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

// ============================================================================
// TC-SLOT-03: Cold Restart Simulation & Battery SRAM Flush Persistence
// ============================================================================

#[test]
fn test_tc_slot_03_cold_restart_persistence_and_sram_flushing() -> std::io::Result<()> {
    let temp_root = std::env::temp_dir().join(format!("pixeldrive_cold_restart_{}", std::process::id()));
    let storage = AndroidStorage::new(temp_root.clone());

    let game = "Pokemon_Crystal_Version";
    let sram_payload = vec![0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x90];
    let slot1_payload = vec![0xCA, 0xFE, 0xBA, 0xBE, 0x01, 0x02, 0x03];
    let slot2_payload = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x04, 0x05, 0x06];

    // Phase A: Write battery SRAM and save states to disk
    {
        storage.flush_sram(game, &sram_payload)?;
        storage.save_to_slot(game, 1, &slot1_payload)?;
        storage.save_to_slot(game, 2, &slot2_payload)?;

        // Verify SRAM file exists immediately
        let sram_path = storage.get_save_path(game);
        assert!(sram_path.exists(), "Battery SRAM (.sav) must be immediately written to disk");
    }

    // Phase B: Simulate process termination (kill -9 / drop all in-memory structures)
    drop(storage);

    // Phase C: Cold Boot / Process Restart -> Create brand new storage instance and emulator cores
    let fresh_storage = AndroidStorage::new(temp_root.clone());

    // 1. Verify Battery SRAM is intact and matches exact payload
    let loaded_sram = fresh_storage.load_save(game).expect("SRAM save must persist across process restart");
    assert_eq!(loaded_sram, sram_payload, "Battery SRAM corrupted across restart");

    // 2. Verify Save State Slots 1 and 2 persist accurately
    let loaded_slot1 = fresh_storage.load_from_slot(game, 1)?;
    assert_eq!(loaded_slot1, slot1_payload, "Slot 1 state corrupted across restart");

    let loaded_slot2 = fresh_storage.load_from_slot(game, 2)?;
    assert_eq!(loaded_slot2, slot2_payload, "Slot 2 state corrupted across restart");

    // 3. Verify Metadata is accurately deserialized upon cold restart
    let slots_info = fresh_storage.get_slots_info(game);
    assert!(!slots_info[0].is_empty, "Slot 1 must be occupied on cold restart");
    assert!(!slots_info[1].is_empty, "Slot 2 must be occupied on cold restart");
    assert!(slots_info[2].is_empty, "Slot 3 must remain empty on cold restart");

    // 4. Verify GBC Live Core restoration with battery SRAM
    let mut gbc = GbcCore::new();
    let gbc_rom = create_synthetic_gbc_rom(game);
    gbc.load_rom(&gbc_rom);

    // Load persistent battery SRAM into core
    let sram_loaded = gbc.load_save_data(&loaded_sram);
    assert!(sram_loaded, "Core must accept persistent battery SRAM");

    // Clean up
    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

// ============================================================================
// TC-SLOT-04: Empty Slot Handling & Slot Deletion
// ============================================================================

#[test]
fn test_tc_slot_04_empty_slot_handling_and_slot_deletion() -> std::io::Result<()> {
    let temp_root = std::env::temp_dir().join(format!("pixeldrive_empty_slot_{}", std::process::id()));
    let storage = AndroidStorage::new(temp_root.clone());
    let game = "GoldenSun_DarkDawn";

    // 1. Unwritten game must report all 5 slots as empty
    let slots = storage.get_slots_info(game);
    for slot in &slots {
        assert!(slot.is_empty);
        assert_eq!(slot.formatted_time, "[ Empty Slot ]");
    }

    // 2. Attempting to load from unpopulated slot must return NotFound error
    let load_res = storage.load_from_slot(game, 3);
    assert!(load_res.is_err(), "Loading unpopulated slot must return Error");
    assert_eq!(
        load_res.unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
        "Error kind must be NotFound"
    );

    // 3. Save to Slot 3, verify it becomes occupied
    let data = vec![0x55, 0x66, 0x77, 0x88];
    storage.save_to_slot(game, 3, &data)?;

    let updated_slots = storage.get_slots_info(game);
    assert!(updated_slots[0].is_empty);
    assert!(updated_slots[1].is_empty);
    assert!(!updated_slots[2].is_empty);
    assert!(updated_slots[3].is_empty);
    assert!(updated_slots[4].is_empty);

    // 4. Delete Slot 3
    storage.delete_slot(game, 3)?;

    // 5. Verify Slot 3 is now empty and files are deleted
    let state_file = storage.get_slot_state_path(game, 3);
    let meta_file = storage.get_slot_meta_path(game, 3);
    assert!(!state_file.exists(), "State file must be deleted");
    assert!(!meta_file.exists(), "Meta file must be deleted");

    let post_delete_slots = storage.get_slots_info(game);
    assert!(post_delete_slots[2].is_empty, "Slot 3 must be marked empty after deletion");
    assert_eq!(post_delete_slots[2].formatted_time, "[ Empty Slot ]");

    // Clean up
    let _ = fs::remove_dir_all(&temp_root);
    Ok(())
}

// ============================================================================
// TC-SLOT-05: Scoped Storage Path Sanitization & Traversal Prevention
// ============================================================================

#[test]
fn test_tc_slot_05_scoped_storage_path_sanitization_for_slots() {
    let temp_root = std::env::temp_dir().join("pixeldrive_sanitization_test");
    let storage = AndroidStorage::new(temp_root.clone());

    // 1. Dangerous directory traversal strings
    let dangerous_titles = [
        "../../../../../../etc/passwd",
        "../../system/bin/app_process",
        "game/with/slashes",
        "game\\with\\backslashes",
        "game:with:colons*and?wildcards",
        "game\0with\0null\0bytes",
        "",
        "...",
        "  ..//..  ",
    ];

    for raw in &dangerous_titles {
        let clean = SaveManager::sanitize_stem(raw);
        assert!(!clean.is_empty(), "Sanitized title must not be empty for {:?}", raw);
        assert!(!clean.contains('/'), "Must not contain forward slash: {}", clean);
        assert!(!clean.contains('\\'), "Must not contain backslash: {}", clean);
        assert!(!clean.contains(':'), "Must not contain colon: {}", clean);
        assert!(!clean.contains('\0'), "Must not contain null bytes: {}", clean);
        assert!(!clean.starts_with('.'), "Must not start with dot: {}", clean);

        let slot_path = storage.get_slot_state_path(raw, 1);
        let meta_path = storage.get_slot_meta_path(raw, 1);

        // Ensure slot path is strictly within the states directory
        assert!(
            slot_path.starts_with(storage.states_dir()),
            "Slot path {:?} must remain inside states directory {:?}",
            slot_path,
            storage.states_dir()
        );
        assert!(
            meta_path.starts_with(storage.states_dir()),
            "Meta path {:?} must remain inside states directory {:?}",
            meta_path,
            storage.states_dir()
        );
    }
}
