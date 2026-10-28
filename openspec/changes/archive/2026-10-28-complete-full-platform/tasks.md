# Tasks

## 1. Windows Host Engine (Phase 7)
- [x] 1.1 Scaffold `crates/remote-desk-windows` with DXGI desktop duplication and Windows Graphics Capture abstraction
- [x] 1.2 Implement Win32 SendInput coordinate normalization (0..65535) and stuck-key fail-safe release engine
- [x] 1.3 Add automated unit tests verifying Windows capture and input injection (`cargo test -p remote-desk-windows`)

## 2. Worldwide Connectivity, TURN Relay & Unified Host Daemon (Phase 8)
- [x] 2.1 Enhance `crates/remote-desk-net` with ICE candidate gathering (host/srflx/relay) and TURN fallback logic
- [x] 2.2 Enhance `crates/remote-desk-signal` with TURN token provisioning and live rendezvous signaling server
- [x] 2.3 Implement `crates/remote-desk-host` daemon binary integrating capture, state machine, WebRTC DataChannels, and input injection

## 3. Production Hardening, Packaging & Release (Phase 9)
- [x] 3.1 Linux packaging configurations (Flatpak, Debian, RPM, Arch, AppImage, systemd units)
- [x] 3.2 Windows packaging configurations (WiX MSI definition, PowerShell service setup)
- [x] 3.3 Android release configuration (ProGuard/R8 rules, Fastlane metadata)
- [x] 3.4 Security governance (`SECURITY.md`) and updated standards compliance register (`docs/standards-register.md`)
- [x] 3.5 Comprehensive end-to-end integration test suite and root `README.md`
