# Design: Complete Full Remote-Desk Platform

## Context

Phases 0 through 6 implemented the shared protocol (`remote-desk-proto`), security and pairing core (`remote-desk-core`), Linux portal and X11 capture (`remote-desk-linux`), network signaling structures (`remote-desk-net`, `remote-desk-signal`), Android UI and gesture modules (`android/`), and benchmarking suite (`remote-desk-bench`). Completing the platform requires implementing Phase 7 (Windows Host Integration), Phase 8 (Worldwide ICE/TURN connectivity and unified host daemon), and Phase 9 (production packaging, service automation, and security policies).

## Goals / Non-Goals

**Goals:**
- Deliver `remote-desk-windows` providing DXGI Desktop Duplication and Win32 `SendInput` with coordinate normalization, stuck-key revocation, and headless/mock test fallback for cross-platform Linux CI.
- Enhance `remote-desk-net` and `remote-desk-signal` with ICE candidate gathering (host, srflx, relay) and TURN fallback logic.
- Deliver `remote-desk-host` daemon binary integrating display capture, state machine progression, WebRTC DataChannels, SAS verification, and input injection.
- Provide comprehensive production packaging configurations for Linux (Flatpak, Debian, RPM, Arch, AppImage, systemd), Windows (WiX MSI, PowerShell service), and Android (ProGuard/R8, Fastlane).
- Publish `SECURITY.md` and update `standards-register.md`.

**Non-Goals:**
- Proprietary cloud relay infrastructure requiring vendor subscription or centralized user account databases.
- Modifying previously stabilized core serialization codecs or breaking existing wire protocol formats.

## Decisions

### 1. Windows Host Engine Architecture with Dual-Target Compilation
- **Decision**: In `crates/remote-desk-windows`, use conditional compilation:
  - On `target_os = "windows"`, link against `windows` crate bindings (`windows::Win32::Graphics::Dxgi`, `windows::Win32::UI::Input::KeyboardAndMouse`) to execute real DXGI Desktop Duplication and Win32 `SendInput`.
  - On `not(target_os = "windows")` (Linux development hosts and CI containers), provide `MockWindowsCaptureBackend` and `MockWindowsInputInjector` implementing the identical traits.
- **Rationale**: Allows cross-compilation and automated unit testing on Linux developer machines and standard CI runners without requiring dedicated Windows build runners for protocol validation.
- **Alternatives Considered**: Building a standalone C++ DLL (rejected: violates pure-Rust memory safety and workspace consistency).

### 2. ICE Candidate Priority and Encrypted TURN Relay Fallback
- **Decision**: Prioritize ICE candidates in strict RFC 8445 order: direct host candidates (priority 1), STUN server-reflexive candidates (priority 2), and TURN relay candidates (priority 3). All payload traffic sent across TURN relays is encrypted with DTLS 1.3 / SRTP.
- **Rationale**: Ensures lowest latency for LAN and hole-punched WAN sessions, while guaranteeing 100% connectability behind symmetric enterprise NATs without decrypting data at the relay.
- **Alternatives Considered**: Direct TCP tunneling (rejected: introduces TCP head-of-line blocking on lossy wireless networks).

### 3. Standalone Unified Host Daemon (`remote-desk-host`)
- **Decision**: Create an orchestrating binary `remote-desk-host` that ties together screen capture (auto-detecting Linux Wayland/X11 or Windows), input injection, rendezvous signaling, and SAS mutual authentication.
- **Rationale**: Provides users and system administrators with a single runnable binary (`remote-desk-host --bind 0.0.0.0:8443`) that can be executed interactively or managed as a systemd / Windows service.
- **Alternatives Considered**: Launching separate microservices for capture, signaling, and input (rejected: excessive inter-process communication latency and complexity).

### 4. Native Linux and Windows Distribution Packaging Standards
- **Decision**: Provide declarative distribution manifests under `packaging/`:
  - Linux: Flatpak (`org.remotedesk.RemoteDesk.yml`), Debian (`debian/control`), RPM spec, Arch PKGBUILD, AppImage recipe, and systemd service files (`remote-desk.service`).
  - Windows: WiX Toolset XML (`remote-desk.wxs`) for standard MSI deployment and PowerShell service registration.
- **Rationale**: Standardizes distribution across all major desktop Linux distributions and enterprise Windows environments without relying on third-party packaging web services.

## Risks / Trade-offs

- **[Risk]** Linux CI environment cannot test real Win32 driver input execution.
  - **Mitigation**: Implement full synthetic mock injectors that mirror the exact Windows coordinate mapping, boundary clamping, and held-key bitmasks, with automated unit tests for stuck-key revocation.
- **[Risk]** Symmetric NATs fail UDP direct hole punching.
  - **Mitigation**: Seamless fallback to TURN relay over UDP/TLS port 3478/443 with explicit user diagnostic status in UI.
- **[Risk]** Host crash or unexpected disconnect leaves modifier keys (Ctrl/Alt/Shift) stuck down.
  - **Mitigation**: Both Linux and Windows backends implement mandatory `release_all()` fail-safe calls on session termination.
