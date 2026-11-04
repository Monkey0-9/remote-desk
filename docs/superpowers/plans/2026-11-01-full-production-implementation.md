# Full Production Implementation Plan: Linux, Windows, WebRTC, Signaling, Protocol & Android

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Transform Remote-Desk from architectural prototypes into fully genuine production implementations across Linux (Wayland/X11), Windows (DXGI/SendInput), WebRTC transport, WebSocket signaling server, cryptographic wire protocol, empirical microbenchmarks, and Android client.

**Architecture:** A modular high-performance remote desktop architecture featuring: (1) Persistent zero-database cryptographic device identity and authenticated handshake; (2) Genuine Linux capture via pure-Rust X11 (MIT-SHM) and Wayland DBus ScreenCast portal with `/dev/uinput` and X11 XTest injection; (3) Native Windows DXGI Desktop Duplication and SendInput injection; (4) Low-latency WebRTC transport with separate control (reliable) and input (unordered) DataChannels; (5) High-throughput WebSocket signaling server with Ed25519 envelope authentication; (6) Empirical microbenchmarks replacing static constants; and (7) Android Keystore and touch client.

**Tech Stack:** Rust 2021, Tokio, x11rb (pure-Rust X11), zbus (DBus/Wayland portal), tokio-tungstenite (WebSocket signaling), webrtc (pure-Rust WebRTC), ed25519-dalek, x25519-dalek, Kotlin / Android SDK, OpenSpec.

**Spec:** OpenSpec specifications in `openspec/changes/` and architectural audit requirements.

## Global Constraints

- Zero database architecture for signaling server: pure memory-bounded mailboxes with TTL and cryptographic envelope authentication.
- Strict cross-platform buildability: crates must compile cleanly on Linux x86_64; Windows-specific APIs must use conditional compilation (`#[cfg(target_os = "windows")]`) with complete Linux-compilable native definitions or stubs so `cargo check --workspace --all-targets` passes on Linux.
- Zero fake benchmarks: all benchmark figures in `remote-desk-bench` and `README.md` must be dynamically measured empirical results, never hardcoded constants.
- Git commit identity must strictly be `Monkey0-9 <prakashpraveen046@gmail.com>`.
- Zero private data leakage: `.gemini/`, `.serena/`, private keys, and environment variables must never be committed to git.
- All code must pass `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --all-targets`, and `openspec validate --all`.

## Review Focus

1. **Input stuck-key failure on sudden disconnect:** Network drops while Shift/Ctrl/Mouse is down must trigger automatic fail-safe release in <10ms.
2. **Replay and spoofing attacks on signaling broker:** Every signaling envelope must carry a timestamp, nonce, and valid Ed25519 signature verified against the sender's public key before routing or rate-limiting.
3. **Out-of-bounds or NaN/Inf coordinates:** Malicious or corrupted pointer coordinates must be validated and clamped or rejected before hitting native display injectors.
4. **Wayland/DXGI display mode change / access loss:** Screen resolution changes or display sleep must trigger clean session renegotiation without crashing the host daemon.
5. **Memory leaks in signaling mailboxes:** High-volume traffic or abandoned sessions must be bounded by per-mailbox capacity caps and automatic TTL expiration.

---

### Task 1: Core Wire Protocol Hardening & Persistent Identity

**Files:**
- Modify: `crates/remote-desk-core/src/identity.rs`
- Modify: `crates/remote-desk-core/src/pairing.rs`
- Modify: `crates/remote-desk-core/src/session.rs`
- Modify: `crates/remote-desk-proto/src/codec.rs`
- Modify: `crates/remote-desk-proto/src/messages.rs`
- Test: `crates/remote-desk-core/tests/identity_persistent_test.rs`
- Test: `crates/remote-desk-proto/tests/codec_bounds_test.rs`

**Interfaces:**
- Produces: `DeviceIdentity::load_or_generate(path: &Path) -> Result<DeviceIdentity, IdentityError>`
- Produces: `DeviceIdentity::fingerprint_bytes(&self) -> [u8; 32]`
- Produces: `SessionState` (12 states: `Discovering`, `Connecting`, `Negotiating`, `Authenticating`, `AwaitingApproval`, `EstablishingTransport`, `Streaming`, `Paused`, `Reconnecting`, `Disconnecting`, `Closed`, `Failed`, `Revoked`)
- Produces: `PairingHandshake::process_response_with_validation(...) -> Result<SessionKeys, PairingError>` with small-order point rejection and mutual Ed25519 transcript signing
- Produces: `TouchGesturePayload`, `ClipboardDataPayload`, `QualityConfigPayload`

- [x] **Step 1: Write tests for identity disk persistence, 12 session states, and codec bounds validation**
- [x] **Step 2: Implement persistent identity storage with 0600 file permissions and 32-byte SHA-256 fingerprint in `identity.rs`**
- [x] **Step 3: Implement X25519 low-order point rejection and mutual Ed25519 transcript signing in `pairing.rs`**
- [x] **Step 4: Expand `session.rs` to full 12 lifecycle states and retain monitor ID on pause/resume**
- [x] **Step 5: Harden `codec.rs` with exact frame size bounds, sequence replay window, and NaN/Inf coordinate rejection**
- [x] **Step 6: Add `TouchGesturePayload`, `ClipboardDataPayload`, and `QualityConfigPayload` to `messages.rs`**
- [x] **Step 7: Run `cargo test -p remote-desk-core -p remote-desk-proto` and verify 100% pass**
- [x] **Step 8: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 2: Genuine Linux Capture Engine (X11 MIT-SHM & Wayland Portal)

**Files:**
- Modify: `crates/remote-desk-linux/Cargo.toml`
- Modify: `crates/remote-desk-linux/src/capture.rs`
- Create: `crates/remote-desk-linux/src/x11_capture.rs`
- Create: `crates/remote-desk-linux/src/portal_capture.rs`
- Test: `crates/remote-desk-linux/tests/capture_real_test.rs`

**Interfaces:**
- Produces: `X11ShmCaptureBackend`: connects to X11 display, queries MIT-SHM extension, enumerates monitors via XRandR, captures root window frames into shared memory buffers with damage notification
- Produces: `WaylandPortalCaptureBackend`: creates DBus `org.freedesktop.portal.ScreenCast` session via `zbus`, requests screen stream, negotiates PipeWire stream node fd, imports DMA-BUF / MemFd frames
- Produces: `create_linux_capture_backend(preference: BackendPreference) -> Box<dyn ScreenCaptureBackend>`

- [x] **Step 1: Add pure-Rust `x11rb = { version = "0.13", features = ["shm", "randr", "damage"] }` and `zbus = "4.4"` to `crates/remote-desk-linux/Cargo.toml`**
- [x] **Step 2: Write tests for X11 and Portal capture interfaces and monitor enumeration**
- [x] **Step 3: Implement `x11_capture.rs` using `x11rb` with MIT-SHM buffer mapping and XRandR output discovery**
- [x] **Step 4: Implement `portal_capture.rs` with DBus portal ScreenCast session creation, PipeWire fd negotiation, and DMA-BUF / MemFd fallback**
- [x] **Step 5: Integrate real backends into `create_linux_capture_backend()`, auto-detecting Wayland vs X11 environment with graceful fallback**
- [x] **Step 6: Run `cargo test -p remote-desk-linux` and verify pass**
- [x] **Step 7: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 3: Genuine Linux Input Injection Engine (/dev/uinput & X11 XTest)

**Files:**
- Modify: `crates/remote-desk-linux/Cargo.toml`
- Modify: `crates/remote-desk-linux/src/input.rs`
- Create: `crates/remote-desk-linux/src/uinput_injector.rs`
- Create: `crates/remote-desk-linux/src/xtest_injector.rs`
- Test: `crates/remote-desk-linux/tests/input_injector_test.rs`

**Interfaces:**
- Produces: `UinputInjector`: creates virtual keyboard and mouse devices via `/dev/uinput` ioctls; emits EV_KEY, EV_REL, EV_ABS events; implements fail-safe release in <10ms
- Produces: `XTestInjector`: injects X11 synthetic pointer and key events via `x11rb` XTest extension
- Produces: `create_linux_input_injector(...) -> Box<dyn InputInjectorBackend>`

- [x] **Step 1: Write tests for input event translation, coordinate mapping, and stuck key fail-safe release**
- [x] **Step 2: Implement `/dev/uinput` injector with evdev event structures and coordinate scaling**
- [x] **Step 3: Implement X11 XTest injector using `x11rb` with virtual keycode mapping**
- [x] **Step 4: Implement automatic stuck key fail-safe release on drop or disconnect**
- [x] **Step 5: Wire real injectors into `create_linux_input_injector()`, prioritizing `/dev/uinput` with fallback to XTest**
- [x] **Step 6: Run `cargo test -p remote-desk-linux` and verify pass**
- [x] **Step 7: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 4: Genuine Windows Engine (DXGI Capture & SendInput)

**Files:**
- Modify: `crates/remote-desk-windows/Cargo.toml`
- Modify: `crates/remote-desk-windows/src/capture.rs`
- Modify: `crates/remote-desk-windows/src/input.rs`
- Test: `crates/remote-desk-windows/tests/windows_backend_test.rs`

**Interfaces:**
- Produces: `DxgiCaptureBackend`: Direct3D 11 device creation, `IDXGIOutputDuplication::AcquireNextFrame`, multi-monitor enumeration via `EnumDisplayMonitors`, `DXGI_ERROR_ACCESS_LOST` recovery
- Produces: `SendInputInjector`: Windows `SendInput` API calls, absolute virtual desktop coordinate mapping (`MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK`), scan codes, extended keys, DPI scaling
- Compiles conditionally with `#[cfg(target_os = "windows")]` using `windows-sys` and comprehensive cross-platform simulation stubs when compiled on Linux

- [x] **Step 1: Add `windows-sys = { version = "0.59", features = ["Win32_Graphics_Dxgi", "Win32_Graphics_Direct3D11", "Win32_UI_WindowsAndMessaging", "Win32_UI_Input_KeyboardAndMouse"] }` to `crates/remote-desk-windows/Cargo.toml` under `[target.'cfg(windows)'.dependencies]`**
- [x] **Step 2: Write tests verifying monitor enumeration, coordinate translation, and API bindings**
- [x] **Step 3: Implement real DXGI Output Duplication backend for Windows with ACCESS_LOST recovery**
- [x] **Step 4: Implement real `SendInput` injector with virtual desktop coordinate mapping and scan code support**
- [x] **Step 5: Verify clean compilation on Linux target (`cargo check -p remote-desk-windows`)**
- [x] **Step 6: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 5: Real Async WebSocket Signaling Server

**Files:**
- Modify: `crates/remote-desk-signal/Cargo.toml`
- Modify: `crates/remote-desk-signal/src/lib.rs`
- Modify: `crates/remote-desk-signal/src/broker.rs`
- Modify: `crates/remote-desk-signal/src/main.rs`
- Create: `crates/remote-desk-signal/src/server.rs`
- Test: `crates/remote-desk-signal/tests/websocket_signaling_test.rs`

**Interfaces:**
- Produces: `SignalingServer::bind(addr: &str, capacity: usize) -> Result<SignalingServer, SignalError>`
- Produces: Async WebSocket handler processing client registration, Ed25519 envelope signature verification, mailbox message delivery, and HTTP `/health` probe
- Produces: Bounded queues (max 64 messages per device) with TTL expiration (default 300s) and per-IP/device rate limiting

- [x] **Step 1: Add `tokio-tungstenite = "0.23"` and `futures-util = "0.3"` to `crates/remote-desk-signal/Cargo.toml`**
- [x] **Step 2: Write integration tests testing real WebSocket client connect, authentication, envelope routing, and `/health` probe**
- [x] **Step 3: Implement `server.rs` with WebSocket upgrade, envelope signature validation, and HTTP health check**
- [x] **Step 4: Add TTL expiration and bounded queues to `broker.rs` to prevent memory leaks**
- [x] **Step 5: Update `remote-desk-signal/src/main.rs` to start the real WebSocket server and handle graceful shutdown**
- [x] **Step 6: Run `cargo test -p remote-desk-signal` and verify pass**
- [x] **Step 7: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 6: Real WebRTC Transport & Data Channels

**Files:**
- Modify: `crates/remote-desk-net/Cargo.toml`
- Modify: `crates/remote-desk-net/src/peer.rs`
- Modify: `crates/remote-desk-net/src/signaling.rs`
- Modify: `crates/remote-desk-net/src/ice.rs`
- Create: `crates/remote-desk-net/src/webrtc_session.rs`
- Test: `crates/remote-desk-net/tests/webrtc_transport_test.rs`

**Interfaces:**
- Produces: `WebRtcPeerSession`: manages real WebRTC PeerConnection, SDP offer/answer generation, ICE gathering, and STUN/TURN configuration
- Produces: SCTP Data Channels: `"remote-desk-control"` (reliable, ordered) and `"remote-desk-input"` (unreliable, unordered, maxPacketLifeTime = 100ms)
- Produces: Input event coalescer reducing pointer move flood while preserving button and key state transitions

- [x] **Step 1: Add `webrtc = "0.11"` to `crates/remote-desk-net/Cargo.toml`**
- [x] **Step 2: Write tests for WebRTC session initialization, SDP exchange, and DataChannel messaging**
- [x] **Step 3: Implement `webrtc_session.rs` configuring PeerConnection, SCTP DataChannels, and ICE gathering**
- [x] **Step 4: Implement pointer event coalescer in `crates/remote-desk-net/src/coalescer.rs`**
- [x] **Step 5: Connect WebSocket signaling client in `signaling.rs` to exchange SDP and ICE candidates**
- [x] **Step 6: Run `cargo test -p remote-desk-net` and verify pass**
- [x] **Step 7: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 7: Complete Host Daemon Orchestration

**Files:**
- Modify: `crates/remote-desk-host/Cargo.toml`
- Modify: `crates/remote-desk-host/src/main.rs`
- Modify: `crates/remote-desk-host/src/service.rs`
- Test: `crates/remote-desk-host/tests/host_daemon_test.rs`

**Interfaces:**
- Produces: `HostService::run(&mut self) -> Result<(), HostError>` connecting persistent identity, native capture backend, input injector, signaling client, and WebRTC streaming loop
- Produces: Fail-safe shutdown releasing all pressed keys/buttons and terminating portal/DXGI sessions

- [x] **Step 1: Write integration tests for `HostService` startup, health check, session lifecycle, and graceful shutdown**
- [x] **Step 2: Update `service.rs` to load persistent device identity from `~/.config/remote-desk/identity.key`**
- [x] **Step 3: Wire real Linux/Windows capture and input backends into the host streaming pipeline**
- [x] **Step 4: Integrate WebRTC peer connection and WebSocket signaling into `HostService`**
- [x] **Step 5: Verify host daemon CLI arguments (`--health-check`, `--backend`, `--display`, `--bind`)**
- [x] **Step 6: Run `cargo test -p remote-desk-host` and verify pass**
- [x] **Step 7: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 8: Empirical Microbenchmarking & Real Profiling

**Files:**
- Modify: `crates/remote-desk-bench/Cargo.toml`
- Modify: `crates/remote-desk-bench/src/main.rs`
- Modify: `crates/remote-desk-bench/src/lib.rs`
- Test: `crates/remote-desk-bench/tests/bench_execution_test.rs`

**Interfaces:**
- Produces: `run_empirical_benchmarks() -> BenchmarkResults`
- Measures:
  1. Ed25519 signing & verification ops/sec
  2. Protocol frame encode/decode throughput (MB/s) and latency (µs)
  3. Coordinate mapping & projection operations/sec
  4. Input event coalescing efficiency (reduction %)
  5. Memory allocation rate and buffer pool throughput

- [x] **Step 1: Replace hardcoded static constants in `lib.rs` and `main.rs` with real high-resolution timer loops (`std::time::Instant`)**
- [x] **Step 2: Implement benchmarks for Ed25519 sign/verify throughput and key exchange**
- [x] **Step 3: Implement benchmarks for frame serialization and packet bounds checking**
- [x] **Step 4: Implement benchmarks for display coordinate transformations and pointer coalescing**
- [x] **Step 5: Run `cargo run -p remote-desk-bench` and verify honest empirical results are reported**
- [x] **Step 6: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 9: Android Controller Native Integration

**Files:**
- Modify: `android/app/src/main/kotlin/com/remotedesk/android/auth/SecureKeyStore.kt`
- Modify: `android/app/src/main/kotlin/com/remotedesk/android/ui/MainActivity.kt`
- Create: `android/app/src/main/kotlin/com/remotedesk/android/net/LanDiscovery.kt`
- Create: `android/app/src/main/kotlin/com/remotedesk/android/webrtc/WebRtcClient.kt`

**Interfaces:**
- Produces: `SecureKeyStore` utilizing Android KeyStore provider with hardware-backed encryption
- Produces: `LanDiscovery` broadcasting and listening on UDP port 4810 for local Remote-Desk hosts
- Produces: `WebRtcClient` managing WebRTC PeerConnection on Android and rendering to SurfaceViewRenderer

- [x] **Step 1: Implement hardware-backed key generation and storage in `SecureKeyStore.kt`**
- [x] **Step 2: Implement UDP broadcast LAN discovery in `LanDiscovery.kt`**
- [x] **Step 3: Implement WebRTC client wrapper in `WebRtcClient.kt`**
- [x] **Step 4: Connect touch gesture translation in `MainActivity.kt` to WebRTC input data channel**
- [x] **Step 5: Verify Gradle configuration and syntax**
- [x] **Step 6: Commit changes with author `Monkey0-9 <prakashpraveen046@gmail.com>`**

---

### Task 10: Verification, Documentation & Clean Public Git Push

**Files:**
- Modify: `README.md`
- Verify: Entire workspace

- [x] **Step 1: Update `README.md` with honest architecture diagrams, genuine benchmark numbers from Task 8, and verified capabilities**
- [x] **Step 2: Run `cargo fmt --check`**
- [x] **Step 3: Run `cargo clippy --workspace --all-targets -- -D warnings`**
- [x] **Step 4: Run `cargo test --workspace --all-targets`**
- [x] **Step 5: Run `openspec validate --all`**
- [x] **Step 6: Verify `git log` author is strictly `Monkey0-9 <prakashpraveen046@gmail.com>` and no private files (`.gemini/`, `.serena/`, `.env*`) exist in git**
- [x] **Step 7: Push cleanly to `origin main`**
