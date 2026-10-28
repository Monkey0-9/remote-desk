# Tasks

## 1. Specification & Standards Verification

- [x] 1.1 Complete all 9 specification documents in `docs/spec/` and symlink to project root; verify all requirement IDs and references
- [x] 1.2 Validate OpenSpec proposal, capability delta specs, and architecture design documents using `openspec validate`

## 2. Shared Protocol & Core Workspace Scaffolding

- [x] 2.1 Scaffold Cargo root workspace with `crates/remote-desk-proto`, `crates/remote-desk-core`, and `crates/remote-desk-linux`
- [x] 2.2 Implement binary message framing, serialization codecs, and input structs in `remote-desk-proto` and verify with `cargo test -p remote-desk-proto`
- [x] 2.3 Implement Ed25519 identity key generation, signing, and 6-digit SAS verification in `remote-desk-core` and verify with `cargo test -p remote-desk-core`

## 3. Linux Screen Capture & Input Backends

- [x] 3.1 Define `ScreenCaptureBackend` and `InputInjectorBackend` traits in `remote-desk-linux` with Wayland Portal, X11, and display server auto-detection
- [x] 3.2 Implement simulated/mock screen capture loop and stuck-key revocation test in `remote-desk-linux` and verify with `cargo test -p remote-desk-linux`

## 4. Android Controller Architecture & Prototype

- [x] 4.1 Scaffold Android Jetpack Compose structure under `android/app` with Devices, Add Computer, Connection Progress, Session Canvas, Floating Toolbar, and Settings components
- [x] 4.2 Implement touch-to-pointer gesture interpreter and verify gesture mode switching behavior
- [x] 4.3 Implement OWASP MASVS-STORAGE-1 compliant `SecureKeyStore` with wipe capability

## 5. Transport & Worldwide Signaling

- [x] 5.1 Implement `crates/remote-desk-net` with signed signaling envelopes, dual DataChannels (control vs input), STUN/TURN configs, and mDNS discovery
- [x] 5.2 Implement `crates/remote-desk-signal` with in-memory rendezvous routing and token-bucket rate limiter

## 6. Empirical Benchmarks & Standards Traceability

- [x] 6.1 Implement `crates/remote-desk-bench` evaluating workloads WL-01 through WL-10 and generating comparative benchmark against Microsoft Remote Desktop, Chrome Remote Desktop, and RustDesk
- [x] 6.2 Author `docs/standards-register.md` mapping ISO/IEC/IEEE 12207:2026, IEEE 730:2026, OWASP MASVS v2.0, and NIST SSDF SP 800-218 to system requirements and test evidence
