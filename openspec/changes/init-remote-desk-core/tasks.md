# Tasks

## 1. Specification & Standards Verification

- [x] 1.1 Complete all 9 specification documents in `docs/spec/` and symlink to project root; verify all requirement IDs and references
- [x] 1.2 Validate OpenSpec proposal, capability delta specs, and architecture design documents using `openspec validate`

## 2. Shared Protocol & Core Workspace Scaffolding

- [x] 2.1 Scaffold Cargo root workspace with `crates/remote-desk-proto`, `crates/remote-desk-core`, and `crates/remote-desk-linux`
- [x] 2.2 Implement binary message framing, serialization codecs, and input structs in `remote-desk-proto` and verify with `cargo test -p remote-desk-proto`
- [x] 2.3 Implement Ed25519 identity key generation, signing, and 6-digit SAS verification in `remote-desk-core` and verify with `cargo test -p remote-desk-core`

## 3. Linux Screen Capture & Input Prototype

- [x] 3.1 Define `ScreenCaptureBackend` and `InputInjectorBackend` traits in `remote-desk-linux` with Wayland Portal and X11 backends
- [x] 3.2 Implement simulated/mock screen capture loop and stuck-key revocation test in `remote-desk-linux` and verify with `cargo test -p remote-desk-linux`

## 4. Android Controller Architecture & Prototype

- [x] 4.1 Scaffold Android Jetpack Compose structure under `android/app` with Devices, Session Canvas, and Floating Toolbar components
- [x] 4.2 Implement touch-to-pointer gesture interpreter and verify gesture mode switching behavior
