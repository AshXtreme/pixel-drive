# PixelDrive

![Rust](https://img.shields.io/badge/Language-Rust-orange.svg)
![Platform](https://img.shields.io/badge/Platform-macOS-lightgrey.svg)
![License](https://img.shields.io/badge/License-MIT-blue.svg)
![Target](https://img.shields.io/badge/Target-60%20FPS-green.svg)

> **Unified macOS GBC/GBA Emulator in Rust**

PixelDrive is a high-performance, single-window macOS handheld emulator built entirely in safe Rust. It unifies Game Boy / Game Boy Color (8-bit) and Game Boy Advance (32-bit) emulation into a single, cohesive desktop application.

---

## 🔑 Key Features

- **Single Native Window:** Unified macOS app window handling game selection, rendering, audio, and inputs.
- **Metal Acceleration:** Hardware-accelerated 2D surface rendering targeting a rock-solid 60 FPS via `pixels` / `wgpu`.
- **Drag-and-Drop ROM Ingestion:** Seamlessly drag `.gb`, `.gbc`, or `.gba` files directly into the window to hot-swap cores automatically.
- **Trait-Based Core Architecture:** Clean decoupling of windowing system and hardware emulation engines.
- **Low-Latency Stereo Audio:** Direct audio sample piping via `cpal` to macOS audio outputs.

---

## 🏗 Architecture Overview

PixelDrive separates the macOS application shell (`src/frontend/`) from hardware emulation engines through a shared `EmulatorCore` trait (`src/core/`):

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        PixelDrive macOS Frontend                       │
│            (winit event loop + pixels framebuffer + cpal)              │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                     ┌──────────────┴──────────────┐
                     │  EmulatorCore (Rust Trait) │
                     └──────────────┬──────────────┘
                                    │
                   ┌────────────────┴────────────────┐
                   ▼                                 ▼
        ┌─────────────────────┐           ┌─────────────────────┐
        │     GbcCore         │           │     GbaCore         │
        │  - Sharp LR35902    │           │  - ARM7TDMI Core    │
        │  - 160x144 Frame    │           │  - 240x160 Frame    │
        │  - 4-Channel APU    │           │  - DirectSound APU  │
        └─────────────────────┘           └─────────────────────┘
```

- **Frontend (`src/frontend/`):** Manages `winit` event loop, Metal pixel scaling, drag-and-drop file ingestion, and `cpal` audio buffers.
- **Unified Trait (`src/core/`):** `EmulatorCore` trait defines `step_frame()`, `framebuffer()`, `display_dimensions()`, `handle_input()`, and `audio_buffer()`.
- **Hardware Cores (`src/gbc/`, `src/gba/`):**
  - **GBC Engine:** Sharp LR35902 CPU, 64 KB memory bus, scanline PPU, and 4-channel APU.
  - **GBA Engine:** 32-bit ARM7TDMI processor (ARM & THUMB modes), 32-bit memory layout, tilemap & bitmap PPU modes, DirectSound APU.

---

## 🛠 Tech Stack

- **`Rust`** (2021 edition): Safe, zero-cost abstractions for fast emulation logic.
- **`winit`**: Native macOS window management and event loop handling.
- **`pixels`**: Metal-backed hardware-accelerated 2D pixel buffer surface.
- **`cpal`**: Low-latency cross-platform audio output rendering.
- **`log` / `env_logger`**: Standard logging abstractions and stdout formatting.

---

## 🚀 How to Build & Run

### Prerequisites
- macOS (Apple Silicon M1/M2/M3/M4 or Intel)
- Rust toolchain (`cargo` & `rustc`)

### Build
```bash
cargo build --release
```

### Run
```bash
cargo run --release
```

---

## 🗺 Project Roadmap

- [x] **Phase 1: Project Setup & Window Shell**
  - Initialize Cargo workspace and module structure.
  - Setup `winit` event loop and `pixels` framebuffer renderer.
  - Define `EmulatorCore` trait interface and fallback animated test grid.

- [ ] **Phase 2: Game Boy / GBC Core Engine**
  - Implement Sharp LR35902 opcode interpreter.
  - Build 64 KB memory bus & MBC bank switching (MBC1/3/5).
  - Add scanline PPU renderer, keyboard input mapping, and ROM parsing.

- [ ] **Phase 3: Game Boy Advance Core Engine**
  - Build ARM7TDMI execution pipeline (ARM and THUMB mode decoders).
  - Implement 32-bit memory map (EWRAM, IWRAM, VRAM, ROM).
  - Add GBA PPU modes (0–5 tilemap & affine bitmap rendering) and DirectSound FIFO.

- [ ] **Phase 4: Audio, Persistence & App Packaging**
  - Connect low-latency `cpal` audio stream buffers.
  - Enable persistent `.sav` SRAM battery saves.
  - Package standalone macOS `PixelDrive.app` bundle.

---

## 📜 License

This project is licensed under the [MIT License](LICENSE).
