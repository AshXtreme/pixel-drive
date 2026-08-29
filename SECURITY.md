# 🔒 Security Policy

## Supported Versions

Security updates, vulnerability patches, and hardened binary builds are actively maintained for the following versions of PixelDrive:

| Version | Supported          |
| :------ | :----------------- |
| `v1.2.x`| :white_check_mark: |
| `v1.0.x`| :white_check_mark: |
| `< 1.0` | :x:                |

---

## 🛡️ Security Architecture & Hardening

PixelDrive is written in **Rust** to leverage compile-time memory safety, thread safety, and strong type invariants. Specific security measures integrated across the codebase include:

- **Path Traversal Protection:** All save files (`.sav`) and state snapshots (`.state1..9`) sanitize input ROM stems via `SaveManager::sanitize_stem`, preventing traversal outside the dedicated save directory.
- **Android Scoped Storage & SAF Isolation:** On Android, ROM bytes are streamed directly via temporary JNI file descriptors without exposing arbitrary filesystem access or requiring broad storage permissions.
- **Zip-Bomb & Resource Exhaustion Limits:** Archive decompression bounds all stream ingestion with hard caps (8 MB for GBC, 32 MB for GBA) using `std::io::Read::take`.
- **Arithmetic Overflow Hardening:** BIOS SWI signed division (`SWI 0x06`/`0x07`) guards against two's-complement overflow boundary panics (`i32::MIN / -1`).
- **C-ABI FFI Sentinel & Lifecycle Validation:** Foreign Function Interface (FFI) bridges validate all raw pointers and hardware frame buffer sentinels (`(void*)-1`) prior to slice creation, and strictly sequence initialization / deinitialization lifecycles.

---

## 🚨 Reporting a Vulnerability

If you discover a security vulnerability, memory safety flaw, or denial-of-service vector in PixelDrive, please report it responsibly:

1. **Do NOT open a public GitHub issue.**
2. Please submit a private vulnerability report via **GitHub Security Advisories** on the repository or email the maintainers directly.
3. Include detailed reproduction steps, minimal proof-of-concept (PoC) ROM or file, and affected component description.

### Response Commitment
- **Initial Response:** Within 48 hours.
- **Triage & Reproduction:** Within 5 business days.
- **Patch Release:** As promptly as possible following verified mitigation.
