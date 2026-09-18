<p align="center">
  <img src="assets/icon.png" width="160" height="160" alt="PixelDrive Icon" />
</p>

<h1 align="center">🕹️ PixelDrive (v1.4)</h1>

<p align="center">
  <strong>A high-performance, unified handheld emulator engineered in Rust for Android, macOS, and Windows.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Version-1.4.0-blue.svg" alt="Version 1.4.0" />
  <img src="https://img.shields.io/badge/Rust-2021%20Edition-orange.svg" alt="Rust Edition 2021" />
  <img src="https://img.shields.io/badge/Platform-Android%20%7C%20macOS%20%7C%20Windows-brightgreen.svg" alt="Platforms" />
  <img src="https://img.shields.io/badge/Tests-155%20Passed-success.svg" alt="155 Passing Tests" />
  <img src="https://img.shields.io/badge/License-GPL--3.0-blue.svg" alt="GPL-3.0 License" />
</p>

<p align="center">
  <a href="#-core-features-v14">Features</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-scoped-storage-layout">Storage Layout</a> •
  <a href="#-installation--quickstart">Quickstart</a> •
  <a href="#-controls--hotkeys">Controls</a> •
  <a href="#-build--packaging-workflows">Build & Packaging</a> •
  <a href="#-testing--verification">Verification</a> •
  <a href="#-legal-disclaimer">Legal</a> •
  <a href="#-license">License</a>
</p>

---

## ✨ Core Features (v1.4)

### 🌟 Interactive Home Screen Carousel & Instant Resumption
- **GPU-Accelerated Carousel:** Fluid cover flow library view rendered directly via WGPU with paging indicators and title metadata.
- **Integer-Scaled State Snapshots:** Real-time state thumbnails captured with crisp nearest-neighbor integer downscaling (`thumbnails/<crc32>.png`), preserving sharp pixel art without bilinear blur.
- **Seamless Auto-Save Resumption:** Exiting back to the Home Screen automatically serializes core state (`states/<crc32>/auto_save.state`), captures a snapshot thumbnail, and flushes battery SRAM. Tapping a game tile resumes active gameplay instantly at the exact frame without restarting through boot sequences.
- **OS Lifecycle Protection:** Fully integrated with Android `MainEvent::Pause` and window focus transitions to safeguard progress against phone calls, task switching, and system memory reclaim.

### 🎮 In-Game Modal Pause Menu & Event Routing
- **Non-Intrusive In-Game HUD:** Tap the **☰ (Menu)** icon at any time to open the modal pause menu with options to **Resume Game**, **Load ROM**, **Save / Load States**, **Reset Game**, **Settings**, and **Cheats**.
- **Execution & Audio Halting:** Halts core simulation ticks and pauses the audio stream with exponential gain ramp-down to eliminate pops, clicks, or ring buffer underruns.
- **Strict Touch Isolation:** Touch events on modal controls are strictly consumed, preventing ghost inputs or unintended presses on underlying virtual controls.

### ⚡ Decoupled Triple-Buffered Core Execution & Dynamic Audio Pacing
- **Thread Separation:** The emulator core runs on a dedicated high-priority background worker thread (`PixelDrive-CoreThread`), completely isolated from UI rendering and event dispatch.
- **Lock-Free Triple Buffering (`SharedFrameBuffer`):** The core writes completed frames into a back buffer and commits them to the front slot via atomic index swapping. The WGPU renderer copies the front buffer with zero lock contention, eliminating screen tearing and render stalls.
- **Independent Refresh Rates:** The UI pass and display swapchain run smoothly at native panel rates (60Hz, 90Hz, 120Hz) without throttling, lagging, or desynchronizing emulation pacing.
- **Waterline-Driven Audio Pacer (`AudioDrivenPacer`):** Synchronizes emulation execution to the audio ring buffer waterline (safe thresholds: 1,200–2,800 stereo frames at 48kHz) rather than CPU spinlocks or rigid `thread::sleep`, eliminating frame stutter and thermal throttling on mobile hardware.

### 🖥️ Adaptive Dual-Backend Graphics (Vulkan & OpenGLES)
- **Automatic Driver Probing:** Probes system graphics drivers at runtime to select the optimal hardware API.
- **Android Vulkan with OpenGLES Fallback:** Boots with high-throughput Vulkan; dynamically catches driver initialization faults or unsupported device extensions and falls back seamlessly to OpenGL ES with zero application crashes.
- **Desktop Hardware Acceleration:** Native Metal backend on macOS and DirectX 12 / Vulkan on Windows via `wgpu`.
- **Custom WGSL Shaders:** Instant real-time cycling between Nearest-Neighbor integer scaling, authentic LCD subpixel grid lines, and calibrated GBA color correction tone curves.
- **Double-Buffered Texture Streaming:** Employs double-buffered staging textures (`DoubleBufferedTextureStream`) optimized for tiled mobile GPUs (Qualcomm Adreno, ARM Mali).

### 💾 5-Slot Save State Manager & Atomic SRAM Persistence
- **5-Slot State Management:** Independent on-screen slot selector for saving and loading states across 5 dedicated slots per game (`slot_1.state` through `slot_5.state`).
- **Live Visual Previews:** Accompanied by PNG thumbnail snapshots (`slot_N_thumb.png`) and Bincode metadata files (`slot_N.meta`) storing Unix timestamps and occupancy flags.
- **Atomic Disk Writes:** Scoped storage operations utilize atomic staging file renames (`.tmp` $\rightarrow$ destination) to prevent corruption during unexpected shutdowns.
- **Battery Saves (`.sav`):** In-game cartridge SRAM automatically flushes to disk upon state saves, periodic 5-second intervals, and application exit.

### 🎨 Virtual Touch Layout Customizer & Themes
- **Interactive Drag-and-Drop Editor:** Reposition virtual controls (D-Pad, A/B cluster, A+B bridge, Start/Select, L/R shoulders) with normalized boundary clamping (`[0.05, 0.95]`) across diverse screen aspect ratios (16:9, 19.5:9, 21:9).
- **Live Sliders:** Adjust control scale (75%–150%) and opacity (20%–100%) backed by real-time GPU uniform buffer updates.
- **Color Themes:** Instant cycling between **Dark Slate**, **AMOLED Black**, and **Classic DMG Gray**.
- **Persistent Preferences:** Automatically saves custom layouts to `config/touch_layout.json`.

### 📳 Zero-Permission Native Android Haptic Engine
- **Permissionless Feedback:** Dispatches `performHapticFeedback` directly to the Android Activity `DecorView` via JNI, requiring zero invasive manifest permissions (`android.permission.VIBRATE`).
- **Rising-Edge Filtering:** Evaluates edge transitions (`current_mask & !prev_mask`) for distinct tactile clicks; stationary contact and continuous holds produce zero vibration spam.
- **Tactile Responses:** Differentiated physical responses for button presses (`VirtualKey`) and D-Pad quadrant transitions (`KeyboardTap`), fully configurable via Settings.

### 🧬 Per-Game Memory Cheat Engine
- **Automated ROM Identification:** Computes CRC32 checksums using `crc32fast` and extracts internal cartridge headers to manage dedicated cheat files (`cheats/<crc32>.cht`).
- **Multi-Format Code Support:** Parses and decodes GameShark GBC (`01XXYYZZ`), GameShark GBA, Action Replay MAX, CodeBreaker, and Raw memory poke rules.
- **Safe Pre-Frame Injection:** Applies memory patches prior to each frame step with strict boundary validation on GBA EWRAM (`0x02000000..=0x0203FFFF`), IWRAM (`0x03000000..=0x03007FFF`), and GBC WRAM (`$C000..=$DFFF`), preventing memory corruption and segfaults.

### ⚙️ Multi-System Emulation Cores
- **Game Boy / Game Boy Color:** Pure-Rust cycle-accurate core featuring full 4-channel APU audio synthesis, MBC1/2/3/5 cartridge banking, and accurate PPU line-renderer.
- **Game Boy Advance:** High-performance dynamic Libretro bridge (`libloading`) hosting official `libmgba_core` shared libraries with an integrated ARM7TDMI / HLE BIOS interpreter fallback.

---

## 🏛️ Architecture

PixelDrive enforces strict decoupling between the user interface, rendering pipeline, emulation execution, and storage subsystems. Emulation runs asynchronously on a dedicated worker thread, pacing itself dynamically via the audio buffer waterline and publishing finished video frames to a lock-free triple buffer.

```text
+======================================================================================================+
|                                        PIXELDRIVE ARCHITECTURE                                       |
+======================================================================================================+
|                                                                                                      |
|  +------------------------------------------------------------------------------------------------+  |
|  |                                  USER INTERFACE & EVENT LOOP                                   |  |
|  |  +---------------------------+  +---------------------------+  +----------------------------+  |  |
|  |  | Home Screen Carousel      |  | Modal Pause Menu & HUD    |  | Virtual Touch Overlay      |  |  |
|  |  | - ROM Library Navigation  |  | - 5-Slot State Manager    |  | - Hitbox State Machine     |  |  |
|  |  | - Instant State Resumption|  | - Cheat Engine Config     |  | - Rising-Edge Haptics (JNI)|  |  |
|  |  | - Integer Scaled Thumbs   |  | - Real-time Theme Editor  |  | - Drag Layout & Clamping   |  |  |
|  |  +-------------+-------------+  +-------------+-------------+  +-------------+--------------+  |  |
|  +----------------|------------------------------|------------------------------|-----------------+  |
|                   |                              |                              |                    |
|                   | Touch / OSD Draws            | User Actions                 | Merged Joypad      |
|                   v                              v                              v State Matrix       |
|  +-------------------------------------+         |               +--------------------------------+  |
|  |        WGPU RENDER PIPELINE         |         |               |   CORE WORKER THREAD           |  |
|  |  (Vulkan / OpenGL ES / Metal / DX12)|         |               |   ("PixelDrive-CoreThread")    |  |
|  |                                     |         |               |                                |  |
|  |  +-------------------------------+  |         |               |  +--------------------------+  |  |
|  |  | DoubleBufferedTextureStream   |  |         |               |  | Active Emulation Core    |  |  |
|  |  | - Slot 0 (GPU In-Flight)      |  |         |               |  | - GBC: Cycle-Accurate    |  |  |
|  |  | - Slot 1 (CPU Staging Upload) |  |         |               |  | - GBA: ARM7 / mGBA Core  |  |  |
|  |  +---------------+---------------+  |         |               |  +------------+-------------+  |  |
|  |                  ^                  |         |               |               |                |  |
|  |                  | copy_front_to()  |         |               |               | Framebuffer    |  |
|  |                  | (Non-blocking)   |         |               |               v Output         |  |
|  |  +---------------+---------------+  |         |               |  +--------------------------+  |  |
|  |  | WGSL Shaders & Compositor     |  |         |               |  | Memory Cheat Injector    |  |  |
|  |  | - Nearest-Neighbor Filter     |  |         |               |  | - GameShark / AR / Raw   |  |  |
|  |  | - LCD Subpixel Grid Lines     |  |         |               |  | - EWRAM / IWRAM / WRAM   |  |  |
|  |  | - GBA Color Correction Curve  |  |         |               |  +------------+-------------+  |  |
|  |  | - Procedural Touch Overlay    |  |         |               |               |                |  |
|  |  +---------------+---------------+  |         |               |               v publish_frame()|  |
|  +------------------|------------------+         |               |  +--------------------------+  |  |
|                     | Swapchain                  |               |  | Lock-Free Triple Buffer  |  |  |
|                     v (60/90/120Hz)              |               |  | (SharedFrameBuffer)      |  |  |
|            [ Display Surface ]                   |               |  | - Back: Writing          |  |  |
|                                                  |               |  | - Front: Atomic Swapped  |  |  |
|                                                  |               |  | - Free: Next Staging     |  |  |
|                                                  |               |  +------------+-------------+  |  |
|                                                  |               +---------------|----------------+  |
|                                                  |                               |                   |
|                                                  |                               v                   |
|                                                  |             +----------------------------------+  |
|                                                  |             | Dynamic Audio-Driven Frame Pacer |  |
|                                                  |             | - Waterline Tracking (1.5-3.5 f) |  |
|                                                  |             | - Eliminates CPU Spin & Jitter   |  |
|                                                  |             +-----------------+----------------+  |
|                                                  |                               |                   |
|                                                  |                               v Push Audio PCM    |
|  +-----------------------------------------------+-------------------------------+----------------+  |
|  |                               SUBSYSTEMS & PERSISTENCE PIPELINE                                |  |
|  |                                                                                                |  |
|  |  +------------------------------------+       +---------------------------------------------+  |  |
|  |  | Audio Engine (Low-Latency Ringbuf) |       | Scoped Storage & Persistence Engine         |  |  |
|  |  | - Catmull-Rom Hermite Resampler    |       | - config/     (recent_roms, touch_layout)   |  |  |
|  |  | - Android: AAudio / Oboe Engine    |       | - states/     (slot_1..5.state, auto_save)  |  |  |
|  |  | - Desktop: CPAL (CoreAudio/WASAPI) |       | - thumbnails/ (<crc32>.png live snapshots)  |  |  |
|  |  | - Safe Ring Buffer Waterline       |       | - cheats/     (<crc32>.cht patch rules)     |  |  |
|  |  | - Exponential Anti-Pop Ramp-down   |       | - saves/      (atomic battery SRAM .sav)    |  |  |
|  |  +------------------------------------+       +---------------------------------------------+  |  |
|  +------------------------------------------------------------------------------------------------+  |
|                                                                                                      |
+======================================================================================================+
```

---

## 📂 Scoped Storage Layout

PixelDrive manages its data within isolated, scoped platform directories (`filesDir` on Android, standard app data directories on desktop). All writes are atomic to prevent state corruption.

```text
<storage_root>/
├── config/
│   ├── recent_roms.json        # ROM library metadata, CRC32 checksums, last-played timestamps
│   └── touch_layout.json       # Virtual touch overlay layout, coordinates, scales, opacity, theme
├── states/
│   └── <game_title_or_crc32>/
│       ├── auto_save.state     # Instant resumption state written on exit/pause
│       ├── slot_1.state        # Serialized core memory for Slot 1
│       ├── slot_1.meta         # Bincode metadata (Unix timestamp, slot active status)
│       ├── slot_1_thumb.png    # Live preview snapshot for Slot 1
│       └── ... (slots 2 through 5)
├── thumbnails/
│   └── <crc32_hex>.png         # Sharp, integer-scaled thumbnail state snapshots for Carousel
├── cheats/
│   └── <crc32_hex>.cht         # Per-game cheat codes (GameShark, Action Replay, raw pokes)
└── saves/
    └── <game_title>.sav        # Battery-backed cartridge SRAM data (atomically flushed)
```

---

## 🚀 Installation & Quickstart

### 🤖 Android: Signed Multi-ABI APK (.apk)
Download `PixelDrive-Android-v1.4.apk` from the **[Releases](../../releases)** page (supports `arm64-v8a` physical hardware and `x86_64` emulators):
```bash
# Install directly via ADB:
adb install -r PixelDrive-Android-v1.4.apk
```
Or open the APK directly on your device file manager to install.

### 🍎 macOS: Disk Image Installer (.dmg)
Download `PixelDrive-v1.4.dmg` from the **[Releases](../../releases)** page:
1. Open `PixelDrive-v1.4.dmg`.
2. Drag **PixelDrive.app** into your **Applications** folder.
3. Launch PixelDrive from Launchpad, Spotlight, or Finder.

### 🪟 Windows: Standalone Release (.zip)
Download `PixelDrive-Windows-v1.4.zip` from the **[Releases](../../releases)** page:
1. Extract `PixelDrive-Windows-v1.4.zip` to your chosen location.
2. Double-click **PixelDrive.exe** to run.

### 🛠️ Run from Source (Cargo)
Ensure you have the latest stable [Rust toolchain](https://rustup.rs/) installed:
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

### Android Virtual Touch Controls

| Virtual Button | Behavior / Action |
| :--- | :--- |
| **8-Way D-Pad** | Directional movement with tactile quadrant sliding |
| **A / B Buttons** | Primary action buttons with multi-touch chord support |
| **A+B Bridge** | Centered hitbox allowing single-touch trigger of both A + B |
| **L / R Shoulders** | Top-left and top-right shoulder triggers (GBA) |
| **Start / Select** | Utility menu buttons |
| **Fast-Forward (FF)** | Toggles emulation speed (1x, 2x, 4x, 8x, Max) |
| **Quick Save (QS)** | Snapshots real-time game state to the active slot |
| **Quick Load (QL)** | Restores real-time game state from the active slot |
| **Menu Button (☰)** | Opens in-game modal pause menu (Resume, Load, Save/Load, Reset, Settings, Cheats) |

### Desktop Keyboard Controls (Player 1)

| Emulated Button | Primary Keyboard Key | Alternative Key |
| :--- | :--- | :--- |
| **D-Pad Up / Down / Left / Right** | `W` / `S` / `A` / `D` | `Up` / `Down` / `Left` / `Right` Arrow Keys |
| **A Button** | `K` | `Z` |
| **B Button** | `J` | `X` |
| **L Shoulder** (GBA) | `Q` | `U` |
| **R Shoulder** (GBA) | `E` | `I` |
| **Start Button** | `Return` (Enter) | — |
| **Select Button** | `Backspace` | `Right Shift` |

### System & Emulator Hotkeys

| Action | Hotkey |
| :--- | :--- |
| **Toggle Menu / OSD** | `Esc` |
| **Toggle Fast-Forward** | `Tab` |
| **Toggle Audio Mute** | `M` |
| **Cycle Video Shaders** | `F4` (Nearest $\rightarrow$ LCD Grid $\rightarrow$ Color Correction $\rightarrow$ LCD+Color) |
| **Quick Save State** | `F1` (Saves to active slot) |
| **Quick Load State** | `F2` (Loads from active slot) |
| **Select Save Slot (1–5)** | `1` – `5` |
| **Pause / Resume Simulation** | `P` |
| **Reset Emulation Core** | `R` |

---

## 📦 Build & Packaging Workflows

PixelDrive includes fully automated build and packaging scripts for all supported targets.

### 🤖 Android Packaging (`aarch64-linux-android` & `x86_64-linux-android`)

Prerequisites:
- [Android NDK](https://developer.android.com/ndk) (r25c or newer)
- Android SDK with Platform Tools
- `cargo-ndk` (`cargo install cargo-ndk`)

```bash
# 1. Add rustup Android targets
rustup target add aarch64-linux-android x86_64-linux-android

# 2. Automated Multi-ABI packaging:
# Compiles native cdylibs with 16KB page alignment (-Wl,-z,max-page-size=16384),
# bundles libc++_shared.so and libmgba_core.so, strips symbols, and assembles the APK.
./scripts/package_android.sh --release --tag v1.4

# Output: dist/PixelDrive-Android-v1.4.apk
```

Manual step compilation with `cargo-ndk`:
```bash
cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build --release --lib
cd android && ./gradlew assembleRelease
```

---

### 🪟 Windows Packaging (`x86_64-pc-windows-msvc` / Cross-Compile)

#### Native Build on Windows:
```cmd
:: 1. Build release binary with embedded icon resources
cargo build --release --target x86_64-pc-windows-msvc

:: 2. Assemble standalone zip distribution:
scripts\package_windows.bat v1.4

:: Output: dist\PixelDrive-Windows-v1.4.zip
```

#### Cross-Compilation from macOS / Linux:
```bash
# 1. Add Windows target
rustup target add x86_64-pc-windows-gnu

# 2. Compile release binary
cargo build --release --target x86_64-pc-windows-gnu

# 3. Assemble Windows distribution bundle:
./scripts/package_windows.sh --tag v1.4

# Output: dist/PixelDrive-Windows-v1.4.zip
```

---

### 🍎 macOS Universal Binary Packaging (`aarch64-apple-darwin` + `x86_64-apple-darwin`)

PixelDrive packages as a universal Mach-O application bundle supporting both Apple Silicon (`arm64`) and Intel (`x86_64`) Macs.

```bash
# 1. Add Apple Silicon and Intel targets
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# 2. Compile both release slices
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# 3. Merge into a Universal Mach-O binary using lipo
mkdir -p target/release
lipo -create -output target/release/pixeldrive \
  target/aarch64-apple-darwin/release/pixeldrive \
  target/x86_64-apple-darwin/release/pixeldrive

# 4. Generate macOS .app bundle and DMG installer:
./scripts/build_macos_dmg.sh --tag v1.4

# Output: dist/PixelDrive-v1.4.dmg (with drag-and-drop /Applications symlink)
```

---

## 🧪 Testing & Verification

The PixelDrive codebase is validated by **155 passing automated tests** spanning unit testing, core execution, lifecycle transitions, and scoped storage persistence:

```bash
# Run the complete test suite
cargo test
```

### Test Suite Breakdown

| Test Suite | File | Focus Area | Status |
| :--- | :--- | :--- | :--- |
| **Unit Tests** | `src/` | APU/PPU logic, cheat engines, shaders, viewport math | 129 Passed |
| **Cold Boot Loading** | `tests/cold_boot_rom_loading_tests.rs` | SAF URI resolution, ZIP ROM extraction, live core hotswap | 6 Passed |
| **Mobile Hardware Optimizations** | `tests/mobile_hardware_optimization_tests.rs` | Audio-driven pacer, triple-buffering, texture streaming | 5 Passed |
| **Modal Pause Menu** | `tests/modal_pause_menu_tests.rs` | Touch isolation, audio resume integrity, action routing | 5 Passed |
| **Multi-Slot Save States** | `tests/multi_slot_save_state_tests.rs` | Slot serialization, metadata persistence, cold restart SRAM | 5 Passed |
| **System Lifecycle** | `tests/system_lifecycle_stability_tests.rs` | Android surface pause/resume, audio focus loss, soak run | 5 Passed |
| **Virtual Touch Layout** | `tests/virtual_button_layout_tests.rs` | Clamping boundaries, drag translation, JSON persistence | 5 Passed |
| **Total** | | | **155 Passed** |

Verify Android compilation for ARM64 and x86_64 targets:
```bash
cargo ndk -t arm64-v8a check
cargo ndk -t x86_64 check
```

---

## ⚖️ Legal Disclaimer

**PixelDrive** is an independent open-source emulation project developed strictly for educational, research, and archival preservation purposes. PixelDrive is **NOT** affiliated with, authorized, endorsed, or sponsored by **Nintendo Co., Ltd.**, **Nintendo of America Inc.**, or any of their affiliates.

- **No Proprietary ROMs or BIOS Bundled:** PixelDrive does not distribute, host, or link to copyrighted ROM images or proprietary BIOS binaries. Users must supply their own legally acquired game backups.
- **Nominative Trademark Usage:** "Game Boy", "Game Boy Color", and "Game Boy Advance" are registered trademarks of Nintendo Co., Ltd. These terms are used solely for nominative descriptive identification under Fair Use principles.

For additional legal guidelines, consult [LEGAL.md](LEGAL.md).

---

## 📄 License

PixelDrive is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See the [LICENSE](LICENSE) file for complete terms.
