# Tasks

## 1. Specification & Standards Verification (Phase 0)

- [x] 1.1 Complete all 9 specification documents in `docs/spec/` and symlink to project root; verify all requirement IDs and references
- [x] 1.2 Validate OpenSpec proposal, capability delta specs, and architecture design documents using `openspec validate`

## 2. Shared Protocol & Core Workspace Scaffolding (Phase 1)

- [x] 2.1 Scaffold Cargo root workspace with `crates/remote-desk-proto`, `crates/remote-desk-core`, and `crates/remote-desk-linux`
- [x] 2.2 Implement binary message framing, serialization codecs, and input structs in `remote-desk-proto` and verify with `cargo test -p remote-desk-proto`
- [x] 2.3 Implement Ed25519 identity key generation, signing, and 6-digit SAS verification in `remote-desk-core` and verify with `cargo test -p remote-desk-core`

## 3. Linux Screen Capture & Input Backends (Phase 2)

- [x] 3.1 Define `ScreenCaptureBackend` and `InputInjectorBackend` traits in `remote-desk-linux` with Wayland Portal, X11, and display server auto-detection
- [x] 3.2 Implement simulated/mock screen capture loop and stuck-key revocation test in `remote-desk-linux` and verify with `cargo test -p remote-desk-linux`

## 4. Android Controller Architecture & Prototype (Phase 3 & 4)

- [x] 4.1 Scaffold Android Jetpack Compose structure under `android/app` with Devices, Add Computer, Connection Progress, Session Canvas, Floating Toolbar, and Settings components
- [x] 4.2 Implement touch-to-pointer gesture interpreter and verify gesture mode switching behavior
- [x] 4.3 Implement OWASP MASVS-STORAGE-1 compliant `SecureKeyStore` with wipe capability

## 5. Security & Cryptographic Pairing (Phase 5)

- [x] 5.1 Implement mutual authentication, ephemeral X25519 Diffie-Hellman, and SAS generation
- [x] 5.2 Implement fail-safe input revocation on disconnection (`release_all()`)
- [x] 5.3 Enforce zero ambient screen recording by default and zero sensitive data logging

## 6. Empirical Benchmarks & Performance (Phase 6)

- [x] 6.1 Implement `crates/remote-desk-bench` evaluating workloads WL-01 through WL-10 and generating comparative benchmark against Microsoft Remote Desktop, Chrome Remote Desktop, and RustDesk
- [x] 6.2 Author `docs/standards-register.md` mapping ISO/IEC/IEEE 12207:2026, IEEE 730:2026, OWASP MASVS v2.0, and NIST SSDF SP 800-218 to system requirements and test evidence

## 7. Windows Host Integration (Phase 7)

- [x] 7.1 Scaffold `crates/remote-desk-windows` implementing DXGI Desktop Duplication and Windows Graphics Capture abstraction
- [x] 7.2 Implement Win32 SendInput coordinate normalization (0..65535) and stuck-key fail-safe release engine
- [x] 7.3 Add automated unit tests verifying Windows capture and input injection (`cargo test -p remote-desk-windows`)

## 8. Worldwide Connectivity & Relay Fallback (Phase 8)

- [x] 8.1 Implement `crates/remote-desk-net` with signed signaling envelopes, dual DataChannels (control vs input), STUN/TURN configs, and mDNS discovery
- [x] 8.2 Implement `crates/remote-desk-signal` with in-memory rendezvous routing and token-bucket rate limiter
- [x] 8.3 Provide Docker Compose stack with Coturn TURN relay fallback (`deploy/docker/docker-compose.yml`)

## 9. Production Hardening, Packaging & Release (Phase 9)

- [x] 9.1 Author Systemd service unit (`deploy/systemd/remote-desk.service`) and Freedesktop desktop launcher (`deploy/desktop/org.remotedesk.RemoteDesk.desktop`)
- [x] 9.2 Author Security policy and vulnerability disclosure procedures (`SECURITY.md`)
- [x] 9.3 Author Production release runbook and rollback procedures (`docs/release-runbook.md`)
- [x] 9.4 Author Deployment and administration guide (`docs/deployment-guide.md`)
- [x] 9.5 Configure GitHub Actions CI/CD pipeline (`.github/workflows/ci.yml`)
