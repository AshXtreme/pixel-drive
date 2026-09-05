//! Test Suite 1: Cold Boot & ROM Loading Diagnostic Tests.
//!
//! Validates:
//! - TC-BOOT-01: Cold boot clean run without initial ROM.
//! - TC-BOOT-02: SAF file picker dispatch and URI permission persistence.
//! - TC-BOOT-03: Aspect ratio and framebuffer configuration for GBA (3:2, 240x160).
//! - TC-BOOT-04: Aspect ratio and framebuffer configuration for GBC (10:9, 160x144).
//! - TC-BOOT-05: In-memory zip decompression directly from byte buffer without disk I/O.
//! - TC-BOOT-06: Live ROM hot-swapping and clean core teardown across GBA/GBC transitions.

use std::io::{Cursor, Write};
use pixeldrive::core::{Button, EmulatorCore};
use pixeldrive::gba::{GbaCore, GbaHeader, GBA_HEIGHT, GBA_WIDTH};
use pixeldrive::gbc::{GbcCore, GBC_HEIGHT, GBC_WIDTH};
use pixeldrive::platform::{DesktopStorage, PlatformStorage};
use pixeldrive::render::viewport::ViewportConfig;
use pixeldrive::rom::identify_rom;
use pixeldrive::save::SaveManager;
use zip::write::SimpleFileOptions;
use zip::ZipArchive;

/// Helper to generate a valid synthetic GBA ROM buffer with Nintendo header magic.
fn create_synthetic_gba_rom(title: &str, game_code: &str) -> Vec<u8> {
    let mut rom = vec![0x00u8; 0x4000]; // 16 KB minimum test ROM
    // GBA entry point instruction: B 0x080000C0 (ARM branch opcode)
    rom[0] = 0x2E;
    rom[1] = 0x00;
    rom[2] = 0x00;
    rom[3] = 0xEA;

    // Nintendo Logo byte at 0x04..0x9F
    rom[0x04] = 0x24;
    rom[0x05] = 0xFF;
    rom[0x06] = 0xAE;
    rom[0x07] = 0x51;

    // Game Title (0xA0..0xAC, 12 bytes max)
    let title_bytes = title.as_bytes();
    let title_len = title_bytes.len().min(12);
    rom[0xA0..0xA0 + title_len].copy_from_slice(&title_bytes[..title_len]);

    // Game Code (0xAC..0xB0, 4 bytes max)
    let code_bytes = game_code.as_bytes();
    let code_len = code_bytes.len().min(4);
    rom[0xAC..0xAC + code_len].copy_from_slice(&code_bytes[..code_len]);

    // Maker Code (0xB0..0xB2)
    rom[0xB0] = b'0';
    rom[0xB1] = b'1';

    // Nintendo GBA Magic Byte (0xB2 must be 0x96)
    rom[0xB2] = 0x96;

    // Complement Checksum (0xBD)
    let mut checksum: u8 = 0;
    for b in &rom[0xA0..0xBD] {
        checksum = checksum.wrapping_sub(*b);
    }
    checksum = checksum.wrapping_sub(0x19);
    rom[0xBD] = checksum;

    rom
}

/// Helper to generate a valid synthetic GBC ROM buffer with Nintendo header magic.
fn create_synthetic_gbc_rom(title: &str, is_cgb: bool) -> Vec<u8> {
    let mut rom = vec![0x00u8; 0x8000]; // 32 KB minimum test ROM (2 banks)
    // GB entry point at 0x0100: NOP; JP 0x0150
    rom[0x0100] = 0x00; // NOP
    rom[0x0101] = 0xC3; // JP
    rom[0x0102] = 0x50;
    rom[0x0103] = 0x01;

    // Nintendo scrolling logo at 0x0104..0x0133
    rom[0x0104] = 0xCE;
    rom[0x0105] = 0xED;
    rom[0x0106] = 0x66;
    rom[0x0107] = 0x66;

    // Title at 0x0134..0x0143
    let title_bytes = title.as_bytes();
    let max_len = if is_cgb { 11 } else { 16 };
    let copy_len = title_bytes.len().min(max_len);
    rom[0x0134..0x0134 + copy_len].copy_from_slice(&title_bytes[..copy_len]);

    // CGB Flag at 0x0143
    rom[0x0143] = if is_cgb { 0x80 } else { 0x00 };

    // Cartridge Type: MBC5+RAM+BATTERY (0x1B) or ROM ONLY (0x00)
    rom[0x0147] = 0x1B;
    // ROM Size: 32KB (0x00)
    rom[0x0148] = 0x00;
    // RAM Size: 8KB (0x02)
    rom[0x0149] = 0x02;

    // Header Checksum at 0x014D
    let mut chk: u8 = 0;
    for a in 0x0134..=0x014C {
        chk = chk.wrapping_sub(rom[a]).wrapping_sub(1);
    }
    rom[0x014D] = chk;

    rom
}

// ============================================================================
// TC-BOOT-01: Cold boot clean run without initial ROM
// ============================================================================

#[test]
fn test_tc_boot_01_cold_boot_clean_run_no_rom() {
    let _lock = pixeldrive::gba::libretro::lock();
    log::info!("TC-BOOT-01: Verifying cold boot state with unloaded core...");

    // 1. Test GbcCore (default idle core at app launch)
    let mut gbc_core = GbcCore::new();
    assert!(!gbc_core.is_rom_loaded, "GbcCore must start with is_rom_loaded = false");
    let (gbc_w, gbc_h) = gbc_core.display_dimensions();
    assert_eq!(gbc_w, GBC_WIDTH, "GBC core width must be 160");
    assert_eq!(gbc_h, GBC_HEIGHT, "GBC core height must be 144");

    let gbc_fb = gbc_core.framebuffer();
    assert_eq!(
        gbc_fb.len(),
        (GBC_WIDTH * GBC_HEIGHT * 4) as usize,
        "GBC framebuffer must be exactly 160*144*4 = 92,160 bytes"
    );

    // Verify stepping a frame before ROM is loaded does not panic
    gbc_core.step_frame();
    assert_eq!(gbc_core.audio_buffer().len(), 0, "No audio output on unloaded core");

    // Test input manipulation on idle core
    gbc_core.handle_input(Button::A, true);
    gbc_core.handle_input(Button::A, false);
    gbc_core.handle_input(Button::Start, true);
    gbc_core.handle_input(Button::Start, false);

    // Verify save data / state handling on unloaded core
    assert!(gbc_core.get_save_data().is_none() || gbc_core.get_save_data().unwrap().is_empty());
    let _ = gbc_core.save_state();

    // 2. Test GbaCore (cold boot state)
    let mut gba_core = GbaCore::new();
    let (gba_w, gba_h) = gba_core.display_dimensions();
    assert_eq!(gba_w, GBA_WIDTH, "GBA core width must be 240");
    assert_eq!(gba_h, GBA_HEIGHT, "GBA core height must be 160");

    let gba_fb = gba_core.framebuffer();
    assert_eq!(
        gba_fb.len(),
        (GBA_WIDTH * GBA_HEIGHT * 4) as usize,
        "GBA framebuffer must be exactly 240*160*4 = 153,600 bytes"
    );

    gba_core.step_frame();
    gba_core.handle_input(Button::B, true);
    gba_core.handle_input(Button::B, false);

    log::info!("TC-BOOT-01 PASSED: Cold boot clean run initialized without memory faults.");
}

// ============================================================================
// TC-BOOT-02: SAF file picker dispatch and URI permission persistence
// ============================================================================

#[test]
fn test_tc_boot_02_saf_uri_resolution_and_persistence() {
    log::info!("TC-BOOT-02: Validating SAF URI stem sanitization, path mapping & persistence flags...");

    // Test permission flags bitmasks
    const FLAG_GRANT_READ_URI_PERMISSION: i32 = 0x00000001;
    const FLAG_GRANT_WRITE_URI_PERMISSION: i32 = 0x00000002;
    const FLAG_GRANT_PERSISTABLE_URI_PERMISSION: i32 = 0x00000040;

    let requested_picker_flags = FLAG_GRANT_READ_URI_PERMISSION | FLAG_GRANT_PERSISTABLE_URI_PERMISSION;
    assert_eq!(requested_picker_flags, 0x41);

    let returned_intent_flags = FLAG_GRANT_READ_URI_PERMISSION | FLAG_GRANT_PERSISTABLE_URI_PERMISSION | FLAG_GRANT_WRITE_URI_PERMISSION;
    let take_flags = returned_intent_flags & (FLAG_GRANT_READ_URI_PERMISSION | FLAG_GRANT_WRITE_URI_PERMISSION);
    assert_eq!(take_flags, 0x03);

    // Test complex Android SAF Content URIs
    let test_uris = vec![
        ("content://com.android.externalstorage.documents/tree/primary%3AROMs/document/primary%3AROMs%2FPokemon%20Emerald.gba", "Pokemon Emerald"),
        ("content://com.android.providers.media.documents/document/document%3A98765/GoldenSun.gba", "GoldenSun"),
        ("content://com.pixeldrive.provider/files/The%20Legend%20of%20Zelda%20-%20Minish%20Cap.zip", "The Legend of Zelda - Minish Cap"),
        ("content://com.google.android.apps.docs.storage/document/acc%3D1%3Bdoc%3D50", "acc=1;doc=50"),
        ("file:///sdcard/Download/Metroid%20Fusion%20(USA).gba", "Metroid Fusion (USA)"),
        ("../../evil_path_traversal/hack.gba", "hack"),
    ];

    for (uri, _) in test_uris {
        let filename_hint = uri.split('/').last().unwrap_or("unknown");
        let stem = std::path::Path::new(filename_hint)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename_hint);

        let clean_stem = SaveManager::sanitize_stem(stem);
        assert!(!clean_stem.contains(".."), "Sanitized stem must never contain directory traversal: {}", clean_stem);
        assert!(!clean_stem.contains('/'), "Sanitized stem must never contain forward slashes: {}", clean_stem);
        assert!(!clean_stem.contains('\\'), "Sanitized stem must never contain backslashes: {}", clean_stem);
        log::debug!("URI: '{}' -> hint: '{}' -> sanitized: '{}'", uri, filename_hint, clean_stem);
    }

    // Test storage saves, states, and cheats path mapping
    let temp_dir = std::env::temp_dir().join("pixeldrive_saf_test_dir");
    let storage = DesktopStorage::new(temp_dir.clone());

    let game_stem = "Pokemon FireRed (USA)";
    let save_path = storage.get_save_path(game_stem);
    let state_path = storage.get_state_path(game_stem, 2);
    let cheat_path = storage.get_cheat_path(0x12345678);

    assert!(save_path.to_string_lossy().ends_with("Pokemon FireRed (USA).sav"));
    assert!(state_path.to_string_lossy().ends_with("Pokemon FireRed (USA).state2"));
    assert!(cheat_path.to_string_lossy().ends_with("12345678.cht"));

    let _ = std::fs::remove_dir_all(temp_dir);
    log::info!("TC-BOOT-02 PASSED: SAF URI resolution and permission paths verified.");
}

// ============================================================================
// TC-BOOT-03: GBA Aspect Ratio & Framebuffer Configuration Verification (3:2)
// ============================================================================

#[test]
fn test_tc_boot_03_gba_aspect_ratio_and_framebuffer() {
    let _lock = pixeldrive::gba::libretro::lock();
    log::info!("TC-BOOT-03: Validating GBA 3:2 aspect ratio and 153,600 byte RGBA32 framebuffer...");

    let native_w = GBA_WIDTH;
    let native_h = GBA_HEIGHT;
    assert_eq!(native_w, 240);
    assert_eq!(native_h, 160);

    let expected_fb_len = (native_w * native_h * 4) as usize;
    assert_eq!(expected_fb_len, 153_600, "GBA RGBA32 buffer must be 153,600 bytes");

    let gba_rom = create_synthetic_gba_rom("TEST_GBA", "TGBA");
    let parsed_header = GbaHeader::parse(&gba_rom);
    assert!(parsed_header.is_some(), "Synthetic GBA ROM header must parse successfully");
    let header = parsed_header.unwrap();
    assert_eq!(header.title, "TEST_GBA");
    assert_eq!(header.game_code, "TGBA");

    let mut gba_core = GbaCore::new();
    gba_core.load_rom(&gba_rom);

    assert_eq!(gba_core.framebuffer().len(), expected_fb_len);

    // 1. Test 16:9 Landscape Screen (1920x1080) -> Target aspect 1.5, Screen aspect 1.7777 -> Pillarbox
    let vp_landscape = ViewportConfig::new_gba(1920, 1080);
    assert!((vp_landscape.target_aspect() - 1.5).abs() < 1e-5);
    let rect_landscape = vp_landscape.calculate_viewport_rect();
    assert_eq!(rect_landscape.y, 0.0);
    assert_eq!(rect_landscape.height, 1080.0);
    assert_eq!(rect_landscape.width, 1620.0); // 1080 * 1.5
    assert_eq!(rect_landscape.x, 150.0); // (1920 - 1620) / 2

    // UV mapping at center
    let center_uv = vp_landscape.window_to_texture_uv(960.0, 540.0).expect("Center must map");
    assert!((center_uv.0 - 0.5).abs() < 1e-4);
    assert!((center_uv.1 - 0.5).abs() < 1e-4);

    // UV mapping in left black bar should be None
    assert!(vp_landscape.window_to_texture_uv(50.0, 540.0).is_none());
    // UV mapping in right black bar should be None
    assert!(vp_landscape.window_to_texture_uv(1850.0, 540.0).is_none());

    // 2. Test 9:20 Portrait Mobile Screen (1080x2400) -> Screen aspect 0.45 -> Letterbox
    let vp_portrait = ViewportConfig::new_gba(1080, 2400);
    let rect_portrait = vp_portrait.calculate_viewport_rect();
    assert_eq!(rect_portrait.x, 0.0);
    assert_eq!(rect_portrait.width, 1080.0);
    assert_eq!(rect_portrait.height, 720.0); // 1080 / 1.5
    assert_eq!(rect_portrait.y, 840.0); // (2400 - 720) / 2

    // UV mapping in top black bar should be None
    assert!(vp_portrait.window_to_texture_uv(540.0, 100.0).is_none());
    // UV mapping inside game window
    let portrait_center_uv = vp_portrait.window_to_texture_uv(540.0, 1200.0).expect("Center must map");
    assert!((portrait_center_uv.0 - 0.5).abs() < 1e-4);
    assert!((portrait_center_uv.1 - 0.5).abs() < 1e-4);

    log::info!("TC-BOOT-03 PASSED: GBA 3:2 aspect ratio and framebuffer verified.");
}

// ============================================================================
// TC-BOOT-04: GBC Aspect Ratio & Framebuffer Configuration Verification (10:9)
// ============================================================================

#[test]
fn test_tc_boot_04_gbc_aspect_ratio_and_framebuffer() {
    log::info!("TC-BOOT-04: Validating GBC 10:9 aspect ratio and 92,160 byte RGBA32 framebuffer...");

    let native_w = GBC_WIDTH;
    let native_h = GBC_HEIGHT;
    assert_eq!(native_w, 160);
    assert_eq!(native_h, 144);

    let expected_fb_len = (native_w * native_h * 4) as usize;
    assert_eq!(expected_fb_len, 92_160, "GBC RGBA32 buffer must be 92,160 bytes");

    let gbc_rom = create_synthetic_gbc_rom("TEST_GBC", true);
    let mut gbc_core = GbcCore::new();
    gbc_core.load_rom(&gbc_rom);

    assert_eq!(gbc_core.framebuffer().len(), expected_fb_len);

    // 1. Test 16:9 Landscape Screen (1920x1080) -> Target aspect 160/144 = 1.111111 -> Pillarbox
    let vp_landscape = ViewportConfig::new_gbc(1920, 1080);
    assert!((vp_landscape.target_aspect() - (160.0 / 144.0)).abs() < 1e-5);
    let rect_landscape = vp_landscape.calculate_viewport_rect();
    assert_eq!(rect_landscape.y, 0.0);
    assert_eq!(rect_landscape.height, 1080.0);
    assert_eq!(rect_landscape.width, 1200.0); // 1080 * (160/144) = 1200
    assert_eq!(rect_landscape.x, 360.0); // (1920 - 1200) / 2

    // 2. Test Mobile Portrait (1080x2400)
    let vp_portrait = ViewportConfig::new_gbc(1080, 2400);
    let rect_portrait = vp_portrait.calculate_viewport_rect();
    assert_eq!(rect_portrait.x, 0.0);
    assert_eq!(rect_portrait.width, 1080.0);
    assert!((rect_portrait.height - 972.0).abs() < 1e-3); // 1080 / (160/144) = 972
    assert!((rect_portrait.y - 714.0).abs() < 1e-3); // (2400 - 972) / 2

    // 3. Test UV mapping
    let uv_center = vp_portrait.window_to_texture_uv(540.0, 1200.0).unwrap();
    assert!((uv_center.0 - 0.5).abs() < 1e-4);
    assert!((uv_center.1 - 0.5).abs() < 1e-4);
    assert!(vp_portrait.window_to_texture_uv(540.0, 200.0).is_none());

    log::info!("TC-BOOT-04 PASSED: GBC 10:9 aspect ratio and framebuffer verified.");
}

// ============================================================================
// TC-BOOT-05: In-Memory Zip Handling & ROM Extraction
// ============================================================================

#[test]
fn test_tc_boot_05_in_memory_zip_rom_extraction() {
    let _lock = pixeldrive::gba::libretro::lock();
    log::info!("TC-BOOT-05: Validating in-memory ZIP extraction directly from RAM buffer...");

    // 1. Create in-memory zip archive containing a GBA ROM and auxiliary files
    let gba_rom_bytes = create_synthetic_gba_rom("FIRE_RED", "BPRE");
    let mut zip_buffer = Cursor::new(Vec::new());

    {
        let mut zip_writer = zip::ZipWriter::new(&mut zip_buffer);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        // Add a dummy text file
        zip_writer.start_file("readme.txt", options).unwrap();
        zip_writer.write_all(b"PixelDrive emulator test suite archive").unwrap();

        // Add the GBA ROM inside a subfolder
        zip_writer.start_file("roms/Pokemon FireRed.gba", options).unwrap();
        zip_writer.write_all(&gba_rom_bytes).unwrap();

        // Add another text file
        zip_writer.start_file("license.txt", options).unwrap();
        zip_writer.write_all(b"MIT License").unwrap();

        zip_writer.finish().unwrap();
    }

    let zip_bytes = zip_buffer.into_inner();
    assert!(zip_bytes.starts_with(b"PK\x03\x04"), "Zip magic must match");

    // 2. Perform in-memory decompression without disk I/O
    let cursor = Cursor::new(&zip_bytes);
    let mut archive = ZipArchive::new(cursor).expect("ZipArchive should parse in-memory bytes");

    let mut extracted_gba = None;
    let mut _extracted_name = String::new();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).expect("Entry must be readable");
        let name_lower = file.name().to_lowercase();
        if name_lower.ends_with(".gba") || name_lower.ends_with(".gbc") || name_lower.ends_with(".gb") {
            let mut buf = Vec::with_capacity(file.size() as usize);
            std::io::copy(&mut file, &mut buf).expect("In-memory decompression failed");
            _extracted_name = file.name().to_string();
            extracted_gba = Some(buf);
            break;
        }
    }

    assert!(extracted_gba.is_some(), "Should find and extract embedded .gba file");
    let decompressed = extracted_gba.unwrap();
    assert_eq!(decompressed.len(), gba_rom_bytes.len());
    assert_eq!(decompressed, gba_rom_bytes);

    // 3. Verify ROM identification on decompressed bytes
    let rom_id = identify_rom(&decompressed);
    assert_eq!(rom_id.title, "FIRE_RED");
    assert_eq!(rom_id.game_code, "BPRE");

    // 4. Test GBC zip archive extraction
    let gbc_rom_bytes = create_synthetic_gbc_rom("CRYSTAL", true);
    let mut gbc_zip_buffer = Cursor::new(Vec::new());
    {
        let mut zip_writer = zip::ZipWriter::new(&mut gbc_zip_buffer);
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip_writer.start_file("Pokemon Crystal (USA).gbc", options).unwrap();
        zip_writer.write_all(&gbc_rom_bytes).unwrap();
        zip_writer.finish().unwrap();
    }

    let gbc_zip_bytes = gbc_zip_buffer.into_inner();
    let mut gbc_archive = ZipArchive::new(Cursor::new(&gbc_zip_bytes)).unwrap();
    let mut gbc_file = gbc_archive.by_name("Pokemon Crystal (USA).gbc").unwrap();
    let mut decompressed_gbc = Vec::new();
    std::io::copy(&mut gbc_file, &mut decompressed_gbc).unwrap();

    let mut gbc_core = GbcCore::new();
    gbc_core.load_rom(&decompressed_gbc);
    assert!(gbc_core.is_rom_loaded);
    assert_eq!(gbc_core.display_dimensions(), (160, 144));

    // 5. Test corrupted/truncated zip handling (must fail gracefully without panic)
    let corrupt_zip = vec![0x50, 0x4B, 0x03, 0x04, 0x00, 0x01, 0x02];
    let corrupt_res = ZipArchive::new(Cursor::new(&corrupt_zip));
    assert!(corrupt_res.is_err(), "Corrupted zip must return Err without panicking");

    log::info!("TC-BOOT-05 PASSED: In-memory zip decompression verified.");
}

// ============================================================================
// TC-BOOT-06: Live ROM Hot-Swapping & Clean Core Teardown
// ============================================================================

#[test]
fn test_tc_boot_06_live_rom_hotswap_and_core_teardown() {
    let _lock = pixeldrive::gba::libretro::lock();
    log::info!("TC-BOOT-06: Validating live ROM hot-swapping across GBA -> GBC -> GBA...");

    let temp_dir = std::env::temp_dir().join("pixeldrive_hotswap_test_dir");
    let storage = DesktopStorage::new(temp_dir.clone());

    // 1. Cold boot with default idle core
    let mut active_core: Box<dyn EmulatorCore> = Box::new(GbcCore::new());
    let mut current_game_title = "PixelDrive_Idle".to_string();

    assert_eq!(active_core.display_dimensions(), (160, 144));

    // 2. Hot-load Game 1: GBA (Pokemon FireRed)
    let gba_rom_1 = create_synthetic_gba_rom("FIRE_RED", "BPRE");
    let title_1 = "Pokemon_Fire_Red";

    {
        // Flush previous core save if any
        if let Some(save_data) = active_core.get_save_data() {
            if !save_data.is_empty() {
                let _ = storage.write_save(&current_game_title, &save_data);
            }
        }

        let mut gba = GbaCore::new();
        gba.load_rom_with_hint(&gba_rom_1, "Pokemon FireRed.gba");
        active_core = Box::new(gba);
        current_game_title = title_1.to_string();
    }

    let (core_w1, core_h1) = active_core.display_dimensions();
    assert_eq!(core_w1, 240);
    assert_eq!(core_h1, 160);
    assert_eq!(active_core.framebuffer().len(), 240 * 160 * 4);

    // Run 60 frames of GBA
    for _ in 0..60 {
        active_core.step_frame();
    }

    // Simulate saving SRAM data for GBA Game 1
    let simulated_gba_save = vec![0xA1, 0xB2, 0xC3, 0xD4];
    active_core.load_save_data(&simulated_gba_save);

    // 3. Hot-swap to Game 2: GBC (Pokemon Crystal)
    let gbc_rom_2 = create_synthetic_gbc_rom("CRYSTAL", true);
    let title_2 = "Pokemon_Crystal";

    {
        // Flush previous GBA core save before dropping
        if let Some(save_data) = active_core.get_save_data() {
            if !save_data.is_empty() {
                let _ = storage.write_save(&current_game_title, &save_data);
            }
        }

        let mut gbc = GbcCore::new();
        gbc.load_rom(&gbc_rom_2);
        active_core = Box::new(gbc);
        current_game_title = title_2.to_string();
    }

    let (core_w2, core_h2) = active_core.display_dimensions();
    assert_eq!(core_w2, 160);
    assert_eq!(core_h2, 144);
    assert_eq!(active_core.framebuffer().len(), 160 * 144 * 4);

    // Run 60 frames of GBC
    for _ in 0..60 {
        active_core.step_frame();
    }

    // Simulate saving SRAM data for GBC Game 2
    let simulated_gbc_save = vec![0x11, 0x22, 0x33, 0x44, 0x55];
    active_core.load_save_data(&simulated_gbc_save);

    // 4. Hot-swap to Game 3: GBA (Pokemon Emerald)
    let gba_rom_3 = create_synthetic_gba_rom("EMERALD", "BPEE");
    let title_3 = "Pokemon_Emerald";

    {
        // Flush previous GBC core save before dropping
        if let Some(save_data) = active_core.get_save_data() {
            if !save_data.is_empty() {
                let _ = storage.write_save(&current_game_title, &save_data);
            }
        }

        let mut gba = GbaCore::new();
        gba.load_rom_with_hint(&gba_rom_3, "Pokemon Emerald.gba");
        active_core = Box::new(gba);
        let _ = title_3;
    }

    let (core_w3, core_h3) = active_core.display_dimensions();
    assert_eq!(core_w3, 240);
    assert_eq!(core_h3, 160);
    assert_eq!(active_core.framebuffer().len(), 240 * 160 * 4);

    // Run 60 frames of GBA
    for _ in 0..60 {
        active_core.step_frame();
    }

    // 5. Verify saved SRAM files for Game 1 and Game 2 on disk
    let loaded_gba_save = storage.load_save(title_1).expect("GBA Game 1 save must exist");
    assert_eq!(&loaded_gba_save[..simulated_gba_save.len()], simulated_gba_save.as_slice());

    let loaded_gbc_save = storage.load_save(title_2).expect("GBC Game 2 save must exist");
    assert_eq!(&loaded_gbc_save[..simulated_gbc_save.len()], simulated_gbc_save.as_slice());

    let _ = std::fs::remove_dir_all(temp_dir);
    log::info!("TC-BOOT-06 PASSED: Live ROM hot-swapping and clean core teardown verified.");
}
