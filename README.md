# 🕹️ PixelDrive

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Target](https://img.shields.io/badge/Target-60%20FPS-green.svg)

A modern, high-performance **Game Boy (GB)**, **Game Boy Color (GBC)**, and **Game Boy Advance (GBA)** emulator written in **Rust**, powered by **WGPU** for hardware-accelerated rendering, an ultra-low-latency **CPAL** audio engine, and a dynamic **Libretro Core Bridge**.

---

## ✨ Features

### 🎮 Dual-Core Emulation Architecture
- **Game Boy / Game Boy Color (GB / GBC):**
  - Native, pure-Rust cycle-accurate CPU emulation (Sharp LR35902).
  - Scanline-based Pixel Processing Unit (PPU) supporting background, window, and sprite layers.
  - Complete Game Boy Color features: GBC Background/OBJ Palette RAM banking, VRAM bank switching, and high-speed General DMA (GDMA) / H-Blank DMA (HDMA).
  - Memory Bank Controller support: **ROM-Only**, **MBC1**, **MBC2** (with 512×4-bit built-in RAM), **MBC3** (with RTC), and **MBC5**.
  - Cycle-accurate 4-Channel Sound Synthesizer (APU): Square 1 with Sweep/Envelope, Square 2, Custom Wave RAM Channel 3, and Noise LFSR Channel 4.
- **Game Boy Advance (GBA):**
  - Dynamic C-ABI Libretro Core Bridge (mGBA) loaded at runtime via `libloading`.
  - Zero-latency video buffer conversion (0RGB1555 / RGB565 / XRGB8888 to RGBA32).
  - High-performance ARM7TDMI fallback core with EWRAM, IWRAM, VRAM, and Keypad I/O handling.

### 🔊 Studio-Grade Audio Engine
- **Host Output Stream:** Powered by `cpal` with lock-free ring buffer communication (`ringbuf`).
- **High-Quality Resampler:** Catmull-Rom 4-point cubic Hermite spline interpolation with 2nd-order Butterworth low-pass anti-aliasing filter and DC blockers.
- **Dynamic Rate Control:** Automatic fine-grained clock pacing to eliminate crackling, underflows, and latency buildup.
- **Mute Toggle:** Instantaneous audio mute (`M`) with zero buffer desync.

### 💾 Complete Save Persistence & Save States
- **Battery-Backed Saves (`.sav`):** In-game saves for SRAM, Flash 64K/128K, and MBC battery RAM automatically flushed to `./saves/<rom_name>.sav` every 5 seconds and on application close.
- **Real-Time Save States (`.state1` .. `.state9`):** Full-system snapshots capturing exact CPU, PPU, APU, Timer, and Memory states with instant Quick Load (`F1` to Save, `F5` / `F2` to Load, `1`–`9` for slots).

### ⚡ Fast-Forward & Dynamic Pacing
- **2x Fast-Forward Toggle:** Click `Tab` to switch between 1.0x normal speed and 2x accelerated speed.
- **Smart Audio Throttling:** Automatically drops audio frames during fast-forward to prevent queue backlog and resumes audio smoothly upon deactivation.

### 🖥️ Modern Graphics & Archive Ingestion
- **Hardware-Accelerated Rendering:** Powered by `pixels` and `wgpu` with native **Metal** (macOS / Apple Silicon), **Vulkan** (Linux / Windows), and **DirectX 12** backends.
- **Drag-and-Drop Ingestion:** Drag `.gb`, `.gbc`, `.gba` files or compressed `.zip` archives directly into the window.
- **Dynamic Viewport Resizing:** Automatic resolution and aspect ratio switching (160×144 for GBC, 240×160 for GBA) with seamless window scaling.

---

## 🎮 Controls & Hotkeys

### Gameplay Controls

| GBA / GBC Button | Primary Mapping | Secondary Mapping (WASD Layout) |
| :--- | :--- | :--- |
| **D-Pad Up** | `Up Arrow` | `W` |
| **D-Pad Down** | `Down Arrow` | `S` |
| **D-Pad Left** | `Left Arrow` | `A` |
| **D-Pad Right** | `Right Arrow` | `D` |
| **A Action Button** | `Z` | `J` |
| **B Action Button** | `X` | `K` |
| **L Shoulder** | `Q` | `U` |
| **R Shoulder** | `E` | `I` |
| **Start** | `Enter` | — |
| **Select** | `Right Shift` | `Left Shift` / `Backspace` |

### ⚡ System Hotkeys

| Action | Keybinding |
| :--- | :--- |
| **Toggle 2x Fast-Forward** | `Tab` |
| **Mute / Unmute Sound** | `M` |
| **Save State Snapshot** | `F1` |
| **Quick Load State Snapshot** | `F5` or `F2` |
| **Select Save State Slot (1–9)** | Number keys `1` .. `9` |

---

## 🚀 Getting Started

### 1. Prerequisites
- [Rust & Cargo](https://www.rust-lang.org/tools/install) (1.75 or later recommended).

### 2. Setting Up Libretro GBA Cores (Optional for GBA)
PixelDrive dynamically discovers Libretro cores in the `./cores/` directory:

```bash
mkdir -p cores
# macOS (Apple Silicon / Intel):
# Place mgba_libretro.dylib in cores/

# Linux:
# Place mgba_libretro.so in cores/

# Windows:
# Place mgba_libretro.dll in cores/
```

### 3. Build & Run

```bash
# Build optimized release binary
cargo build --release

# Run emulator window
cargo run --release

# Or launch directly with a ROM or ZIP archive:
cargo run --release -- path/to/game.gba
# or
cargo run --release -- path/to/game.gbc
# or
cargo run --release -- path/to/game.zip
```

### 4. Running Tests

```bash
cargo test -- --test-threads=1
```

---

## 📁 Project Architecture

```
PixelDrive/
├── cores/               # Libretro dynamic shared libraries (.dylib, .so, .dll)
├── saves/               # Auto-saved in-game .sav and real-time .state1 snapshots
├── src/
│   ├── audio/           # Host audio engine (cpal, ringbuf, cubic resampler, filters)
│   ├── core/            # Unified EmulatorCore trait and button input definitions
│   ├── gba/             # GBA emulation (Libretro C-ABI bridge, ARM7TDMI fallback, MMU)
│   ├── gbc/             # Native GBC emulation (Sharp LR35902 CPU, PPU, APU, MMU, MBC)
│   ├── save.rs          # Unified Save Manager (battery RAM & binary save state files)
│   └── main.rs          # Winit event loop, pixels rendering, and hotkey management
├── Cargo.toml
└── README.md
```

---

## 📜 License

This project is licensed under the [MIT License](LICENSE).
