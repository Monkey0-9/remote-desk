# Proposal: Complete Full Remote-Desk Platform

## Why

While Phases 0 through 6 established the architectural specifications, shared core protocols, Linux capture engine, mobile UI foundation, cryptographic pairing, and empirical benchmarks, the platform still requires native Windows host capture/input integration (Phase 7), worldwide ICE/TURN relay networking and an integrated host daemon (Phase 8), and production packaging, installer automation, and security release policies (Phase 9) to deliver a 100% complete, fully verified, production-grade remote desktop solution.

## What Changes

- **Phase 7: Windows Host Engine (`crates/remote-desk-windows`)**:
  - Implement DXGI Desktop Duplication and Windows Graphics Capture abstractions.
  - Implement Win32 `SendInput` injector with absolute/relative coordinate normalization and immediate stuck-key revocation on disconnection.
  - Support display topology enumeration, monitor switching, and simulated cross-platform test fallbacks for Linux CI.
- **Phase 8: Worldwide Connectivity & Unified Host Daemon (`crates/remote-desk-host`, enhancements to `remote-desk-net` and `remote-desk-signal`)**:
  - Implement full ICE negotiation (Host, Server-Reflexive via STUN, and Relay via TURN fallback).
  - Implement integrated `remote-desk-host` daemon binary coordinating capture, encoding, session state machine, SAS authentication, and input injection.
  - Provide live rendezvous signaling router with rate limiting and TURN credential negotiation.
- **Phase 9: Production Hardening, Packaging & Release**:
  - Linux packaging: Flatpak manifest, Debian package control/rules, RPM spec, Arch PKGBUILD, AppImage recipe, and systemd units.
  - Windows packaging: WiX MSI installer definition and background service PowerShell deployment script.
  - Android packaging: Release ProGuard/R8 rules and Fastlane metadata.
  - Security & Standards: `SECURITY.md` vulnerability disclosure policy, audit trails, complete user guide in `README.md`, and standards register updates.

## Capabilities

### New Capabilities
- `windows-host-engine`: DXGI/Windows Graphics Capture screen capture, Win32 input injection, and display topology enumeration on Windows hosts.
- `worldwide-turn-signaling`: Full ICE negotiation, TURN relay fallback, out-of-band signaling broker, and unified host daemon.
- `production-packaging-release`: Multi-platform packaging recipes (Flatpak, Debian, RPM, Arch, AppImage, WiX MSI), systemd services, and security governance policies.

### Modified Capabilities
<!-- No requirement changes to existing capabilities; all prior capabilities remain backward-compatible and intact. -->

## Impact

- **New Crates**: `crates/remote-desk-windows`, `crates/remote-desk-host`.
- **Modified Crates**: `crates/remote-desk-net`, `crates/remote-desk-signal`, root `Cargo.toml`.
- **Packaging Files**: New `packaging/` directory containing Linux, Windows, and systemd configurations.
- **Documentation**: New `SECURITY.md`, updated `README.md`, updated `docs/standards-register.md`.
