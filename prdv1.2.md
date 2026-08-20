# Product Requirements Document (PRD) — PixelDrive v1.2 (Android Edition)

## 1. Executive Summary
PixelDrive v1.2 expands the pure-Rust/WGPU Game Boy (GB/GBC) and Game Boy Advance (GBA) emulator to the Android platform. This release delivers a mobile-native experience featuring high-performance Vulkan/GLES rendering, low-latency audio via Oboe/AAudio, on-screen virtual touch controls with haptics, and Android Scoped Storage integration for ROM/save management.

---

## 2. Target Platform & Architecture

| Parameter | Target Specification |
| :--- | :--- |
| **Minimum Android Version** | Android 8.0 (API Level 26, Oreo) |
| **Target SDK / Compilation** | Android 14 / 15 (API Levels 34 / 35) |
| **Target Architectures** | `aarch64-linux-android` (Primary 64-bit), `armv7-linux-androideabi` (32-bit fallback) |
| **Graphics Backend** | WGPU targeting Vulkan (Primary) / OpenGLES 3.0 (Fallback) |
| **Audio Backend** | `cpal` with Oboe / AAudio driver backend |
| **Windowing / Lifecycle** | `android-activity` (`GameActivity` or `NativeActivity`) via `winit` |

---

## 3. Core Functional Requirements

### 3.1 Virtual Touch Controls & HUD
* **Multi-Touch Tracking:** Support simultaneous multi-finger interactions (e.g., diagonal D-Pad movement while holding B for running and tapping A for jumping).
* **Hitbox & Layout Engine:**
  * **D-Pad:** Configurable fixed vs. floating dynamic center; 8-way directional vector mapping with deadzones.
  * **Face Buttons:** Angled A/B buttons with a central bridge hitbox for sliding/chording both buttons.
  * **Triggers:** Dedicated L/R shoulder trigger zones positioned at top-left and top-right safe areas.
  * **System Controls:** Start, Select, Fast-Forward toggle, Quick Save/Load, and Menu overlay.
* **Procedural WGPU Overlay:** SDF-based (Signed Distance Field) anti-aliased virtual buttons rendered in a batched pass (no large raster UI textures required).
* **Customization:**
  * Opacity slider (0% to 100%).
  * Scale and positioning presets (Compact, Standard, Wide, Ergonomic).
  * Auto-hide when a physical Bluetooth/USB gamepad is connected.
* **Haptics:** Native Android vibration feedback via JNI/NDK on button activation.

### 3.2 Mobile Rendering & Display Pipeline
* **Orientation Support:** Native landscape-locked primary mode with auto-rotation (Sensor Landscape).
* **Display Scaling:**
  * Aspect Ratio Preservation: 10:9 (GB/GBC) and 3:2 (GBA).
  * Integer scaling mode and fit-to-screen with pillarboxing.
* **WGSL Mobile Shaders:**
  * Optimized mobile variants for LCD grid and GBA color correction to sustain 60 FPS without thermal throttling.
* **Safe-Area Insets:** Dynamic margins to account for display cutouts, camera notches, and gesture navigation bars.

### 3.3 Storage, ROM Loading & Saves
* **Android Storage Access Framework (SAF):** Native file picker integration to allow loading ROMs from Downloads, SD cards, or external storage without requiring broad storage permissions.
* **Scoped Storage Pathing:**
  * Internal App Storage (`context.getExternalFilesDir()`): Default storage for SRAM saves (`.sav`), save states (`.state`), and cached config.
  * Automated periodic SRAM flush to prevent save corruption on unexpected task termination or background eviction.

### 3.4 Android Lifecycle Management
* **`onPause` / `onStop`:** Auto-pause emulation loop, silence audio stream, and persist a recovery state.
* **`onResume`:** Re-acquire audio stream, restore WGPU surface swapchain, and resume emulation without frame drops.
* **Low-Power / Background Mode:** Prevent battery drain and thread spinning when the app is minimized.

---

## 4. Technical Architecture & Module Layout
src/
├── platform/
│   └── android/
│       ├── activity.rs      # android-activity lifecycle hooks & JNI bridge
│       ├── audio.rs         # AAudio / Oboe low-latency stream binder
│       ├── storage.rs       # SAF file picker & scoped storage paths
│       └── haptics.rs       # NDK / JNI VibrationEffect bridge
├── input/
│   ├── mod.rs               # Unified InputSource trait
│   ├── touch.rs             # Multi-touch pointer tracker & hitbox geometry
│   └── gamepad.rs           # Physical gamepad input via gilrs
├── render/
│   ├── mod.rs
│   ├── overlay.rs           # WGPU SDF touch overlay quad renderer
│   └── shaders/
│       └── overlay.wgsl     # Procedural button/D-pad shader
android/
├── app/
│   ├── src/main/AndroidManifest.xml
│   └── res/mipmap-*/ic_launcher.png

---

## 5. Non-Functional Requirements
* **Performance:** Stable 60.0 FPS execution on mid-range ARM64 hardware with under 15% overall CPU utilization.
* **Latency:** Audio buffer latency $\le 30\text{ms}$; touch-to-render input latency $\le 16.6\text{ms}$ (1 frame).
* **Package Size:** Final release APK/AAB binary size $\le 25\text{ MB}$ (with stripped symbols and LTO enabled).

---

## 6. Release Milestones

1. **M1: Android Build Harness & Surface Binding** — Compile Rust core with `cargo-ndk` and initialize WGPU surface via `android-activity`.
2. **M2: Virtual Touch Engine & Overlay** — Implement `touch.rs` and render procedural buttons via `overlay.wgsl`.
3. **M3: Audio & SAF Storage Integration** — Wire low-latency AAudio stream and document-picker ROM loader.
4. **M4: Haptics & Optimization** — Add vibration feedback, thermal optimizations, and APK packaging pipeline.