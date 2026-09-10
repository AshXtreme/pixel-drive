<p align="center">
  <img src="assets/icon.png" width="160" height="160" alt="PixelDrive Icon" />
</p>

<h1 align="center">🕹️ PixelDrive (v1.4)</h1>

<p align="center">
  <strong>A modern, high-performance Game Boy (GB / GBC) and Game Boy Advance (GBA) handheld emulator built in Rust for Android, macOS, and Windows.</strong>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation--quickstart">Installation</a> •
  <a href="#-controls--hotkeys">Controls</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-packaging--distribution">Packaging</a> •
  <a href="#-testing--verification">Testing</a> •
  <a href="#-legal-disclaimer">Legal</a> •
  <a href="#-license">License</a>
</p>

---

## ✨ Features

### 🌟 Home Screen Library Carousel & Instant Resumption (New in v1.4)
- **Interactive Library Carousel:** Modern, GPU-rendered library view showcasing recently loaded games with live snapshot cards and paging indicators.
- **Auto-State Snapshot on Exit:** Exiting back to the Home Screen automatically captures the active WGPU framebuffer as a downscaled JPEG thumbnail (`thumbnails/<crc32>.jpg`), serializes the active core state (`states/<crc32>/auto_save.state`), and flushes battery SRAM (`.sav`).
- **Instant Gameplay Resumption:** Tapping any game tile instantly reloads the core, unserializes `auto_save.state`, and steps the frame pipeline, restoring active gameplay at the exact frame without rebooting through title screens.
- **OS Lifecycle Protection:** Automatically saves snapshots and state on Android `MainEvent::Pause` (app minimization, phone calls) to prevent progress loss.

### 🎮 In-Game Modal Pause Menu & Event Routing
- **Non-Intrusive In-Game HUD:** Tap the **☰ (Menu)** button at any time to open the modal pause menu with options to **Resume Game**, **Load ROM**, **Save / Load States**, **Reset Game**, **Settings**, and **Cheats**.
- **Execution & Audio Halting:** Halts the emulation tick loop and cleanly pauses the low-latency AAudio/Oboe stream with exponential decay to eliminate audio pops and buffer underruns.
- **Strict Touch Isolation:** Touch events on the modal HUD are strictly intercepted, preventing ghost presses on underlying virtual gamepad controls.

### 💾 Multi-Slot Save States (Slots 1–5) & SRAM Persistence
- **5-Slot State Management:** Direct on-screen slot selector for saving and loading states across 5 dedicated slots per game (`slot_1.state` through `slot_5.state`).
- **Timestamped Metadata:** Saves corresponding metadata files (`slot_N.meta`) recording unix timestamps and occupancy status via `bincode`.
- **Atomic Disk Writes:** Scoped storage file operations use a staging temporary file rename strategy to prevent state corruption.
- **Battery Saves (`.sav`):** In-game cartridge SRAM automatically flushes to disk on save events, periodic 5-second intervals, and on exit.

### 🎨 Virtual Touch Customizer & Themes
- **Interactive Layout Editor:** Drag and reposition on-screen controls (D-Pad, A/B cluster, Start/Select, L/R shoulders) with normalized viewport clamping `[0.05, 0.95]`.
- **Real-Time Sliders:** Adjust control scale (75%–150%) and opacity (20%–100%) with live WGPU uniform buffer updates.
- **Color Theme Presets:** Instant cycling between **Dark Slate**, **AMOLED Black**, and **Classic DMG Gray** themes.
- **Config Persistence:** All layout adjustments and preferences persist to `<internal_files_dir>/config/touch_layout.json`.

### 🧬 Per-Game Cheat Code Engine
- **Automated ROM Identification:** Computes CRC32 checksums using `crc32fast` and extracts internal header metadata to manage per-game `.cht` files (`cheats/<crc32>.cht`).
- **Multi-Format Code Support:** Parses GameShark GBC (`01XXYYZZ`), GameShark GBA, Action Replay MAX, CodeBreaker, and Raw memory patches.
- **Safe Pre-Frame Injection:** Pokes values into memory before each frame with strict bounds checks on GBA EWRAM (`0x02000000..=0x0203FFFF`), IWRAM (`0x03000000..=0x03007FFF`), and GBC WRAM (`$C000–$DFFF`) to guard against corruption or segfaults.

### 📳 Low-Latency Native Android Haptic Engine
- **Zero-Permission Feedback:** Invokes `performHapticFeedback` on the activity DecorView via JNI, requiring zero invasive manifest permissions (`VIBRATE`).
- **Rising-Edge Filtering:** Evaluates rising edge transitions (`current_mask & !prev_mask`) for tactile clicks; stationary moves produce no vibration spam.
- **Tactile Effects:** Delivers distinct effects for button presses (`VirtualKey`) and D-Pad quadrant slides (`KeyboardTap`), fully gated by a Settings toggle.

### ⚙️ Multi-System Dual-Core Architecture
- **Game Boy / Game Boy Color:** Native cycle-accurate pure-Rust emulation core featuring full 4-channel APU audio synthesis, MBC1/2/3/5 cartridge banking, and accurate PPU rendering.
- **Game Boy Advance:** High-performance dynamic Libretro core bridge (`libloading`) with pre-bundled official `libmgba_core` dynamic libraries (`.so`, `.dylib`, `.dll`) and built-in ARM7TDMI / HLE BIOS interpreter fallback.

### 🖥️ Hardware-Accelerated Rendering (WGPU)
- Native GPU acceleration across **Vulkan / OpenGL ES (Android)**, **Metal (macOS)**, and **DirectX 12 / Vulkan (Windows/Linux)**.
- **Real-Time WGSL Shaders:** Instant cycling between Nearest-Neighbor integer scaling, authentic LCD subpixel grid lines, and GBA color correction tone curves.
- **Procedural Touch Overlay:** GPU-rendered on-screen virtual game controller with subpixel anti-aliasing and responsive aspect-ratio letterboxing.

### 🔊 Low-Latency Audio Pipeline
- **Android (AAudio / Oboe):** Real-time stereo audio stream ($\le 30\,\text{ms}$ latency) with Catmull-Rom cubic Hermite spline resampling and lock-free ring buffering.
- **Desktop (cpal):** Low-latency stereo audio pipeline (CoreAudio on macOS, WASAPI on Windows) with lock-free ring buffering (`ringbuf`).

---

## 🚀 Installation & Quickstart

### 🤖 Android: Signed Universal APK (.apk)
Download `PixelDrive-Android-v1.4.apk` from the **[Releases](../../releases)** page (supports `arm64-v8a` physical devices and `x86_64` emulators/BlueStacks):
```bash
# Install directly via ADB:
adb install -r PixelDrive-Android-v1.4.apk
```
Or transfer and open the APK directly on your Android device to install.

### 🍎 macOS: Disk Image Installer (.dmg)
Download the latest `PixelDrive-v1.4.dmg` from the **[Releases](../../releases)** page:
1. Open `PixelDrive-v1.4.dmg`.
2. Drag **PixelDrive.app** into your **Applications** folder.
3. Launch PixelDrive from Launchpad, Spotlight, or Finder.

### 🪟 Windows: Portable Standalone (.zip)
Download `PixelDrive-Windows-v1.4.zip` from the **[Releases](../../releases)** page:
1. Extract `PixelDrive-Windows-v1.4.zip` to your desired directory.
2. Double-click **PixelDrive.exe** to launch.

### 🛠️ Build from Source (Cargo)

Ensure you have [Rust (stable)](https://rustup.rs/) installed:

```bash
# Clone the repository
git clone https://github.com/AshXtreme/pixel-drive.git
cd pixel-drive

# Run PixelDrive desktop in Release mode
cargo run --release

# Or launch directly with a ROM file:
cargo run --release -- path/to/game.gba
```

---

## 🎮 Controls & Hotkeys

### Android Touch Controls

| Virtual Button | Description / Action |
| :--- | :--- |
| **8-Way D-Pad** | Directional navigation with tactile quadrant sliding |
| **A / B Buttons** | Action buttons with multi-touch chord support |
| **A+B Bridge** | Central chord hitbox to trigger A + B simultaneously |
| **L / R Shoulder** | Top-left and top-right shoulder triggers (GBA) |
| **Start / Select** | Lower menu utility buttons |
| **Fast-Forward (FF)** | Toggle fast-forward (1x, 2x, 4x, 8x, Max) |
| **Quick Save (QS)** | Snapshot real-time game state to active slot |
| **Quick Load (QL)** | Restore real-time game state snapshot |
| **Menu Button (☰)** | Opens in-game modal pause menu (Resume, Load, Save/Load, Reset, Settings, Cheats) |

### Desktop Gamepad & Keyboard Controls (Player 1)

| Game Boy / GBA Key | Keyboard Key |
| :--- | :--- |
| **D-Pad Up / Down / Left / Right** | `W` / `S` / `A` / `D` or `Arrow Keys` |
| **A Button** | `K` / `Z` |
| **B Button** | `J` / `X` |
| **L Shoulder** (GBA) | `Q` / `U` |
| **R Shoulder** (GBA) | `E` / `I` |
| **Start Button** | `Return` (Enter) |
| **Select Button** | `Backspace` / `Shift` |

### System & Emulator Hotkeys

| Action | Hotkey |
| :--- | :--- |
| **Toggle Menu Bar (OSD)** | `Esc` |
| **Toggle Fast-Forward** | `Tab` |
| **Toggle Audio Mute** | `M` |
| **Cycle Video Shaders** | `F4` (Nearest $\rightarrow$ LCD Grid $\rightarrow$ Color Correction $\rightarrow$ LCD+Color) |
| **Quick Save State** | `F1` (Saves to active slot) |
| **Quick Load State** | `F2` (Loads from active slot) |
| **Select Save Slot (1–9)** | `1` – `9` |
| **Pause / Resume Simulation** | `P` |
| **Reset Core Simulation** | `R` |

---

## 🏛️ Architecture & Codebase Layout

```text
PixelDrive/
├── .github/
│   └── workflows/
│       ├── release.yml         # Automated multi-platform CI/CD release workflow
│       └── build-windows.yml   # Windows-specific GitHub Actions workflow
├── android/                    # Native Android Gradle project & packaging
│   ├── app/
│   │   ├── src/main/
│   │   │   ├── AndroidManifest.xml # NativeActivity configuration
│   │   │   ├── java/com/pixeldrive/emulator/MainActivity.java
│   │   │   ├── jniLibs/        # Pre-compiled native cdylibs (arm64-v8a, x86_64)
│   │   │   └── res/            # Adaptive app icons and launch themes
│   │   └── build.gradle        # Android packaging configuration (v1.4)
├── assets/                     # High-resolution icons and platform metadata
│   ├── macos/                  # AppIcon.icns & Info.plist
│   ├── windows/                # icon.ico & resources.rc (Windows VERSIONINFO)
│   └── icon.png                # Master application logo
├── cores/                      # Dynamic Libretro core libraries (.so / .dylib / .dll)
├── dist/                       # Built production packages (.apk, .dmg, .zip)
├── saves/                      # Battery saves (.sav) and save states
├── scripts/                    # Packaging and release automation scripts
│   ├── package_android.sh      # Multi-ABI Android APK release packager (v1.4)
│   ├── build_macos_dmg.sh      # macOS .app bundle & .dmg installer generator (v1.4)
│   ├── package_windows.bat     # Native Windows release batch packager (.zip, v1.4)
│   └── package_windows.sh      # Cross-platform Windows release packager
├── shaders/                    # GPU WGSL shaders
│   ├── shader.wgsl             # Integer scaling, LCD grid lines & color correction
│   └── overlay.wgsl            # Procedural virtual gamepad touch overlay
├── src/
│   ├── main.rs                 # Desktop entry point & WGPU event loop
│   ├── lib.rs                  # Unified library crate exports & Android entry point
│   ├── cheats/                 # Per-game cheat code engine
│   │   ├── mod.rs              # Cheat models, persistence (.cht format), and parsing
│   │   └── engine.rs           # GameShark/AR decoding & EWRAM/IWRAM/WRAM memory injection
│   ├── core/                   # Shared EmulatorCore trait, Button matrix & system enums
│   ├── gbc/                    # Native Game Boy / Game Boy Color core
│   │   ├── cpu.rs              # LR35902 8-bit CPU & opcode decoder
│   │   ├── mmu.rs              # 16-bit memory bus, VRAM/WRAM banking, DMA/HDMA
│   │   ├── ppu.rs              # Pixel Processing Unit (Modes 0-3, palettes, OAM sprites)
│   │   ├── mbc.rs              # Cartridge controllers (ROM Only, MBC1, MBC2, MBC3, MBC5)
│   │   ├── apu.rs              # 4-channel audio synthesizer (Square 1/2, Wave, Noise)
│   │   └── joypad.rs           # Active-low joypad matrix
│   ├── gba/                    # Game Boy Advance emulation layer
│   │   ├── libretro.rs         # FFI Libretro dynamic bridge with AV & serialization callbacks
│   │   ├── cpu.rs              # ARM7TDMI 32-bit CPU core & mode registers
│   │   ├── arm.rs              # ARM instruction decoder & barrel shifter
│   │   ├── thumb.rs            # 16-bit THUMB instruction decoder
│   │   ├── mmu.rs              # 32-bit GBA memory map, DMA controller & Flash/SRAM
│   │   ├── ppu.rs              # GBA PPU with Modes 0-5 bitmap and affine backgrounds
│   │   ├── bios.rs             # SWI BIOS routines & HLE fallback
│   │   └── keypad.rs           # GBA KEYINPUT 10-button active-low matrix
│   ├── input/                  # Multi-finger touch state machine & keyboard mapping
│   │   └── touch.rs            # VirtualButton hitboxes, multi-touch tracking, chords, haptic filter
│   ├── library/                # Home Screen ROM library & snapshot engine
│   │   ├── db.rs               # LibraryManager, recent_roms.json schema & auto-save tracking
│   │   └── thumbnails.rs       # WGPU framebuffer capture, Lanczos downscale & JPEG encoder
│   ├── platform/               # Platform abstraction layer
│   │   ├── android/            # Android NativeActivity, AAudio, SAF, Haptics, Scoped Storage
│   │   └── desktop/            # Desktop file dialogs, storage paths, CPAL audio
│   ├── render/                 # Hardware-accelerated rendering & video shaders
│   │   ├── mod.rs              # WGPU ShaderPipeline controller & render pass
│   │   ├── overlay.rs          # Procedural touch overlay renderer & uniform buffer
│   │   ├── shaders.rs          # WGSL shaders (Nearest, LCD Grid, Color Correction)
│   │   └── viewport.rs         # Dynamic viewport calculations & aspect ratio letterboxing
│   ├── rom/                    # ROM identification & header extraction
│   │   └── identifier.rs       # CRC32 calculation & GBA/GBC cartridge header parser
│   ├── audio/                  # Low-latency CPAL stereo audio engine & lock-free ring buffer
│   ├── save.rs                 # Battery save (.sav) & multi-slot state snapshot manager
│   ├── ui/                     # UI components
│   │   ├── home_screen.rs      # GPU-accelerated Home Screen carousel renderer & state machine
│   │   ├── layout_config.rs    # Touch layout config, coordinate clamping & JSON persistence
│   │   ├── menu.rs             # In-game modal pause menu layout & hit testing
│   │   └── mod.rs              # egui OSD overlay, top menu bar, and live HUD
│   └── error.rs                # Unified PixelDriveError enum with thiserror
├── tests/                      # Integration test suites
│   ├── cold_boot_rom_loading_tests.rs
│   ├── modal_pause_menu_tests.rs
│   ├── multi_slot_save_state_tests.rs
│   ├── system_lifecycle_stability_tests.rs
│   └── virtual_button_layout_tests.rs
├── Cargo.toml                  # Dependencies and release optimization profiles (v1.4)
├── LEGAL.md                    # Legal disclaimers and trademark acknowledgments
├── SECURITY.md                 # Security policy and vulnerability disclosure guidelines
└── LICENSE                     # GNU General Public License v3.0 (GPL-3.0)
```

---

## 📦 Packaging & Distribution

### 🤖 Build Android Release Package (APK):
```bash
./scripts/package_android.sh --release --tag v1.4
```
Compiles `arm64-v8a` and `x86_64` native cdylibs using `cargo-ndk`, strips debug symbols, bundles `libc++_shared.so` and `libmgba_core.so`, and builds `dist/PixelDrive-Android-v1.4.apk` (~5.8MB).

### 🍎 Build macOS DMG Installer:
```bash
./scripts/build_macos_dmg.sh --release --tag v1.4
```
Produces `dist/PixelDrive-v1.4.dmg` with `/Applications` drag-and-drop symlink.

### 🪟 Build Windows Release Package:
On Windows (Command Prompt / PowerShell):
```cmd
scripts\package_windows.bat v1.4
```
Or on Unix / CI toolchains:
```bash
./scripts/package_windows.sh --tag v1.4
```
Produces `dist/PixelDrive-Windows-v1.4.zip` containing the compiled `PixelDrive.exe` with embedded `.rc` icons and metadata.

---

## 🧪 Testing & Verification

Run the full unit and integration test suite (**146 passing tests**):

```bash
cargo test
```

Verify Android compilation for ARM64 and x86_64:

```bash
cargo ndk -t arm64-v8a check
cargo ndk -t x86_64 check
```

---

## ⚖️ Legal Disclaimer

**PixelDrive** is an independent open-source emulator project developed solely for educational and archival preservation purposes. PixelDrive is **NOT** affiliated with, authorized, endorsed, or sponsored by **Nintendo Co., Ltd.**, **Nintendo of America Inc.**, or any of their subsidiaries.

- **No ROMs Included:** PixelDrive does **not** provide, bundle, or distribute proprietary BIOS files, copyrighted ROMs, or game assets.
- **Trademarks:** "Game Boy", "Game Boy Color", and "Game Boy Advance" are registered trademarks of Nintendo Co., Ltd. Mentioned solely for nominative descriptive identification under Fair Use principles.

For complete legal information, see [LEGAL.md](LEGAL.md).

---

## 📄 License

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See the [LICENSE](LICENSE) file for details.
