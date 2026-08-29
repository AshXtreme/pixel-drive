<p align="center">
  <img src="assets/icon.png" width="160" height="160" alt="PixelDrive Icon" />
</p>

<h1 align="center">🕹️ PixelDrive</h1>

<p align="center">
  <strong>A modern, high-performance Game Boy (GB / GBC) and Game Boy Advance (GBA) emulator built in Rust for Android, macOS, and Windows.</strong>
</p>

<p align="center">
  <a href="#-features">Features</a> •
  <a href="#-installation--quickstart">Installation</a> •
  <a href="#-controls--hotkeys">Controls</a> •
  <a href="#-architecture">Architecture</a> •
  <a href="#-packaging--distribution">Packaging</a> •
  <a href="#-legal-disclaimer">Legal</a> •
  <a href="#-license">License</a>
</p>

---

## ✨ Features

- **Multi-System Dual-Core Architecture:**
  - **Game Boy / Game Boy Color:** Native cycle-accurate pure-Rust emulation core featuring full 4-channel APU audio synthesis, MBC1/2/3/5 cartridge banking, and accurate PPU rendering.
  - **Game Boy Advance:** High-performance dynamic Libretro core bridge (`libloading`) with pre-bundled official `libmgba_core` dynamic libraries (`.so`, `.dylib`, `.dll`) and built-in ARM7TDMI / HLE BIOS interpreter fallback.
- **Cross-Platform Hardware-Accelerated Rendering (WGPU):**
  - Native GPU acceleration across **Vulkan / OpenGL ES (Android)**, **Metal (macOS)**, and **DirectX 12 / Vulkan (Windows/Linux)**.
  - **Real-Time WGSL Shaders:** Instant cycling between Nearest-Neighbor integer scaling, authentic LCD subpixel grid lines, and GBA color correction tone curves.
  - **Procedural Touch Overlay:** GPU-rendered on-screen virtual game controller with subpixel anti-aliasing, tactile press feedback, and responsive aspect-ratio letterboxing.
- **Native Android Experience (Android 8.0+ / API 26+):**
  - **Storage Access Framework (SAF):** Full Android document picker integration with streaming JNI byte loading for `.gba`, `.gbc`, `.gb`, and `.zip` archives.
  - **Low-Latency AAudio / Oboe Engine:** Real-time stereo audio stream ($\le 30\,\text{ms}$ latency) with Catmull-Rom cubic Hermite spline resampling and lock-free ring buffering.
  - **Multi-Touch Gamepad & Haptic Feedback:** Multi-finger tracking supporting simultaneous D-Pad chords, floating dynamic centers, A+B bridge hitbox, and hardware vibration feedback (`VibrationEffect`).
- **Low-Latency Desktop Audio Engine:**
  - Real-time stereo audio pipeline powered by **cpal** (CoreAudio on macOS, WASAPI on Windows) with lock-free ring buffering (`ringbuf`).
- **Save Management & Persistence:**
  - **Battery Saves (`.sav`):** In-game cartridge RAM automatically flushes to scoped storage or `./saves/<rom_name>.sav`.
  - **Real-Time Save States:** Persistent multi-slot state snapshots saved to disk (`./saves/<rom_name>.state1..9`) with instant Quick Save / Quick Load.
- **Modern On-Screen Display & HUD:**
  - On-screen touch menu / load button on mobile, and desktop menu bar (`rfd` / `egui`) with shader cycling, audio mute toggle, save slot manager, and live FPS diagnostics.

---

## 🚀 Installation & Quickstart

### 🤖 Android: Signed Universal APK (.apk)
Download `PixelDrive-Android-v1.2.1.apk` from the **[Releases](../../releases)** page (supports `arm64-v8a` physical devices and `x86_64` emulators/BlueStacks):
```bash
# Install directly via ADB:
adb install -r PixelDrive-Android-v1.2.1.apk
```
Or download and open the APK directly on your Android device to install.

### 🍎 macOS: Disk Image Installer (.dmg)
Download the latest `PixelDrive-v1.2.1.dmg` from the **[Releases](../../releases)** page:
1. Open `PixelDrive-v1.2.1.dmg`.
2. Drag **PixelDrive.app** into your **Applications** folder.
3. Launch PixelDrive from Launchpad, Spotlight, or Finder.

### 🪟 Windows: Portable Standalone (.zip)
Download `PixelDrive-Windows-v1.2.1.zip` from the **[Releases](../../releases)** page:
1. Extract `PixelDrive-Windows-v1.2.1.zip` to your desired directory.
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
| **8-Way D-Pad** | Smooth directional navigation with floating touch center |
| **A / B Buttons** | Action buttons with multi-touch chord support |
| **A+B Bridge** | Central chord hitbox to trigger A + B simultaneously |
| **L / R Shoulder** | Top-left and top-right shoulder triggers (GBA) |
| **Start / Select** | Lower menu utility buttons |
| **Fast-Forward (FF)** | Toggle 2x emulation speed with audio throttling |
| **Quick Save (QS)** | Snapshot real-time game state to active slot |
| **Quick Load (QL)** | Restore real-time game state snapshot |
| **Load ROM / Menu** | Top-left menu icon to trigger Android Document Picker |

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
| **Toggle Fast-Forward (2x Speed)** | `Tab` |
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
│       └── release.yml     # Automated multi-platform CI/CD release workflow
├── android/                # Native Android Gradle project & packaging
│   ├── app/
│   │   ├── src/main/
│   │   │   ├── AndroidManifest.xml # NativeActivity configuration & SAF permissions
│   │   │   ├── java/com/pixeldrive/emulator/RomPickerActivity.java # SAF document picker
│   │   │   ├── jniLibs/    # Pre-compiled native cdylibs (arm64-v8a, x86_64)
│   │   │   └── res/        # Adaptive app icons and launch themes
│   │   └── build.gradle.kts # Android packaging configuration
├── assets/                 # High-resolution icons and platform metadata
│   ├── macos/              # AppIcon.icns & Info.plist
│   ├── windows/            # icon.ico & resources.rc (Windows VERSIONINFO)
│   └── icon.png            # Master 1024x1024 application logo
├── cores/                  # Dynamic Libretro core libraries (.so / .dylib / .dll)
├── dist/                   # Built production packages (.apk, .dmg, .zip)
├── saves/                  # Persistent battery saves (.sav) and save states (.state1..9)
├── scripts/                # Packaging and release automation scripts
│   ├── package_android.sh  # Multi-ABI Android APK release packager
│   ├── build_macos_dmg.sh  # macOS .app bundle & .dmg installer generator
│   ├── package_windows.bat # Native Windows release batch packager (.zip)
│   └── package_windows.sh  # Cross-platform Windows release packager
├── shaders/                # GPU WGSL shaders
│   ├── shader.wgsl         # Integer scaling, LCD grid lines & color correction
│   └── overlay.wgsl        # Procedural virtual gamepad touch overlay
├── src/
│   ├── main.rs             # Desktop entry point, WGPU event loop, input dispatch
│   ├── lib.rs              # Unified library crate exports & Android entry point
│   ├── core/               # Shared EmulatorCore trait, Button matrix & system enums
│   ├── gbc/                # Native Game Boy / Game Boy Color core
│   │   ├── cpu.rs          # LR35902 8-bit Z80-derivative CPU & opcode decoder
│   │   ├── mmu.rs          # 16-bit memory bus, VRAM/WRAM banking, DMA/HDMA
│   │   ├── ppu.rs          # Pixel Processing Unit (Modes 0-3, palettes, OAM sprites)
│   │   ├── mbc.rs          # Cartridge controllers (ROM Only, MBC1, MBC2, MBC3, MBC5)
│   │   ├── apu.rs          # 4-channel audio synthesizer (Square 1/2, Wave, Noise)
│   │   └── joypad.rs       # Active-low joypad matrix
│   ├── gba/                # Game Boy Advance emulation layer
│   │   ├── libretro.rs     # FFI Libretro dynamic bridge with AV & static callbacks
│   │   ├── cpu.rs          # ARM7TDMI 32-bit CPU core & mode registers
│   │   ├── arm.rs          # ARM instruction decoder & barrel shifter
│   │   ├── thumb.rs        # 16-bit THUMB instruction decoder
│   │   ├── mmu.rs          # 32-bit GBA memory map, DMA controller & Flash/SRAM
│   │   ├── ppu.rs          # GBA PPU with Modes 0-5 bitmap and affine backgrounds
│   │   ├── bios.rs         # SWI BIOS routines & HLE fallback
│   │   └── keypad.rs       # GBA KEYINPUT 10-button active-low matrix
│   ├── input/              # Multi-finger touch state machine & keyboard mapping
│   │   └── touch.rs        # VirtualButton hitboxes, multi-touch tracking, chords
│   ├── platform/           # Platform abstraction layer
│   │   ├── android/        # Android NativeActivity, AAudio, SAF, Haptics, JNI
│   │   └── desktop/        # Desktop file dialogs, storage paths, CPAL audio
│   ├── render/             # Hardware-accelerated rendering & video shaders
│   │   ├── mod.rs          # WGPU ShaderPipeline controller & render pass
│   │   ├── overlay.rs      # Procedural touch overlay renderer & uniform buffer
│   │   ├── shaders.rs      # WGSL shaders (Nearest, LCD Grid, Color Correction)
│   │   └── viewport.rs     # Dynamic viewport calculations & aspect ratio letterboxing
│   ├── audio/              # Low-latency CPAL stereo audio engine & lock-free ring buffer
│   ├── save.rs             # Battery save (.sav) & state snapshot manager (.state1..9)
│   ├── ui/                 # egui OSD overlay, top menu bar, and live HUD
│   └── error.rs            # Unified PixelDriveError enum with thiserror
├── Cargo.toml              # Dependencies and release optimization profiles
├── LEGAL.md                # Legal disclaimers and trademark acknowledgments
├── SECURITY.md             # Security policy and vulnerability disclosure guidelines
└── LICENSE                 # GNU General Public License v3.0 (GPL-3.0)
```

---

## 📦 Packaging & Distribution

### 🤖 Build Android Release Package (APK):
```bash
./scripts/package_android.sh
```
Compiles `arm64-v8a` and `x86_64` native cdylibs using `cargo-ndk`, strips debug symbols, bundles `libc++_shared.so` and `libmgba_core.so`, and builds `dist/PixelDrive-Android-v1.2.1.apk`.

### 🍎 Build macOS DMG Installer:
```bash
./scripts/build_macos_dmg.sh
```
Produces `dist/PixelDrive-v1.2.1.dmg` with `/Applications` drag-and-drop symlink.

### 🪟 Build Windows Release Package:
On Windows (Command Prompt / PowerShell):
```cmd
scripts\package_windows.bat
```
Or on Unix / CI toolchains:
```bash
./scripts/package_windows.sh
```
Produces `dist/PixelDrive-Windows-v1.2.1.zip` (and `PixelDrive-Windows-x86_64.zip`) containing the compiled `PixelDrive.exe` with embedded `.rc` icons and metadata.

---

## 🧪 Testing & Verification

Run the full unit and integration test suite (90 passing tests):

```bash
cargo test -- --test-threads=1
```

Verify Android compilation for `arm64-v8a`:

```bash
cargo ndk -t arm64-v8a check --lib
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
