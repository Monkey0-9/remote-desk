# Product Requirements Specification: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-REQ-001 |
| **Standards Compliance** | ISO/IEC/IEEE 12207:2026, IEEE 730:2026, NIST SSDF SP 800-218, OWASP MASVS v2.0 |
| **System Classification** | Real-time Peer-to-Peer Remote Desktop & Control Subsystem |
| **Target Platforms** | Linux (Wayland/X11), Windows (10/11), Android (API 26+) |
| **Status** | Approved Baseline for Milestone 0 |

---

## 1. Executive Summary & Product Vision

**Remote-Desk** is an open-source, privacy-first, low-latency remote desktop system designed to achieve the familiar usability and ergonomic navigation of Microsoft Remote Desktop / Windows App, while incorporating the self-hosting, peer-to-peer, and lightweight operational principles demonstrated by RustDesk. 

### Core Tenets
1. **Uncompromising Privacy & Zero Recording by Default**: Screen recording and cloud session persistence are strictly disabled by default. Telemetry is zero-knowledge, strictly opt-in, and stripped of all PII, keystrokes, and frame buffers.
2. **Sub-Millisecond Pipeline Discipline & Low Resource Overhead**: Idle CPU footprint < 0.5%, idle memory footprint < 60 MB RSS on desktop host. End-to-end glass-to-glass latency target < 40ms over local networks.
3. **Decentralized Cryptographic Identity**: No mandatory central server or database. Devices manage mutual trust via Ed25519 public keys, ephemeral X25519 key agreements, and authenticated Noise / WebRTC DTLS-SRTP sessions.
4. **Verifiable Engineering Quality**: Every feature must map directly to an auditable requirement ID, have automated tests, and provide execution evidence before being declared complete.

---

## 2. Requirement Taxonomy & Verification Matrix

Requirements are structured with unambiguous identifiers, verification methods, and acceptance criteria:
- **Verification Methods**:
  - `[T]` **Test**: Automated unit, integration, or E2E software test.
  - `[D]` **Demonstration**: Observable operational execution against real system boundaries.
  - `[A]` **Analysis**: Cryptographic proof, static code analysis, or mathematical modeling.
  - `[I]` **Inspection**: Code review, documentation audit, or configuration validation.

```
       +-----------------------------------------------------------+
       |                  REQUIREMENT TAXONOMY                     |
       +-----------------------------------------------------------+
       |  SYS-REQ   : High-level System Capabilities               |
       |  FUNC-CAP  : Screen Capture (Wayland / X11 / DXGI)        |
       |  FUNC-INP  : Remote Input Injection & Pointer Handling    |
       |  FUNC-NET  : WebRTC Transport, ICE, STUN/TURN, Signaling  |
       |  FUNC-UI   : Android & Desktop UX / Touch Gestures        |
       |  SEC-REQ   : Cryptography, Auth, OWASP MASVS, Revocation  |
       |  NFR-PERF  : Latency, Frame Rate, CPU, RAM, Bandwidth    |
       +-----------------------------------------------------------+
```

---

## 3. System-Level Requirements (SYS-REQ)

### `REQ-SYS-001`: Cross-Platform Core Separation
- **Description**: The system shall isolate all protocol parsing, session orchestration, cryptography, state machines, and signaling handling into a platform-agnostic Rust core crate (`remote-desk-core`).
- **Rationale**: Prevents code duplication and architectural drift between Linux, Windows, and Android.
- **Verification Method**: `[T]`
- **Acceptance Criteria**: The core crate compiles for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and `aarch64-linux-android` without conditional compilation flags referencing desktop GUI or native capture headers.

### `REQ-SYS-002`: Standalone Local Operation (No Mandatory Central DB)
- **Description**: Host and controller shall be capable of establishing direct peer connections over local area networks (LAN) or direct IP/domain endpoints without contacting any centralized database, account server, or external cloud infrastructure.
- **Verification Method**: `[D]`
- **Acceptance Criteria**: In an isolated network sandbox with WAN routing severed (DNS/WAN packet drop), two devices can pair, authenticate, stream video, and transmit input using direct local signaling.

---

## 4. Functional Requirements: Screen Capture & Video Pipeline (FUNC-CAP)

### `REQ-CAP-001`: Linux Wayland Screen Capture via XDG Desktop Portal
- **Description**: Under Wayland sessions, screen capture shall interface with `org.freedesktop.portal.ScreenCast` and stream frames via PipeWire DMA-BUF / MemFd buffer exchange.
- **Verification Method**: `[T]` & `[D]`
- **Acceptance Criteria**: 
  1. Requests capture session via DBus portal interface with explicit user permission dialog.
  2. Subscribes to PipeWire stream and receives YUV420/RGB frame buffers.
  3. Safely handles user portal rejection, returning `Error::PortalPermissionDenied` without crashing or freezing.

### `REQ-CAP-002`: Linux X11 Screen Capture Fallback
- **Description**: Under native X11 sessions, the host shall support capture using MIT-SHM (`XShmGetImage`) or `XDamage` extension when hardware-accelerated portal pipelines are not present.
- **Verification Method**: `[T]`
- **Acceptance Criteria**: X11 backend detects display `:0`, monitors damage regions, extracts modified rects, and emits raw frames to the video encoder with zero memory leaks.

### `REQ-CAP-003`: Windows Native Desktop Duplication
- **Description**: Under Windows 10/11 hosts, screen capture shall use DXGI Desktop Duplication API (`IDXGIOutputDuplication`) with fallback to Windows Graphics Capture (`Windows.Graphics.Capture`) on modern multi-GPU setups.
- **Verification Method**: `[D]`
- **Acceptance Criteria**: Desktop frames are captured directly from GPU VRAM surfaces with < 5ms capture latency per frame at 1080p60.

### `REQ-CAP-004`: Multi-Monitor Topology Discovery & Switching
- **Description**: The host shall discover all attached virtual and physical displays, their pixel dimensions, DPI scaling factors, and refresh rates, and allow controller-directed display switching.
- **Verification Method**: `[T]`
- **Acceptance Criteria**: Protocol message `DisplayTopologyEvent` transmits a list of monitors with IDs, bounding rects, and primary monitor status; controller can select specific display ID via `SelectMonitorRequest`.

---

## 5. Functional Requirements: Remote Input Injection (FUNC-INP)

### `REQ-INP-001`: Absolute and Relative Pointer Positioning
- **Description**: The input engine shall support both absolute coordinate positioning (normalized `0.0 .. 1.0` float mapped to target monitor resolution) and relative cursor delta movements (`dx, dy`).
- **Verification Method**: `[T]`
- **Acceptance Criteria**: Synthetic mouse events injected on the host reflect exact target coordinates within ±1 pixel accuracy regardless of controller-to-host aspect ratio mismatch.

### `REQ-INP-002`: Linux Remote Desktop Portal & Evdev/UInput Input Backend
- **Description**: Under Linux, input shall be injected via `org.freedesktop.portal.RemoteDesktop` when available. For headless or administrative daemon sessions, safe `uinput` virtual devices (`/dev/uinput`) may be utilized when authorized.
- **Verification Method**: `[D]`
- **Acceptance Criteria**: Mouse move, button click (Left, Middle, Right, Back, Forward), mouse wheel scroll, and key presses are injected into the active session without privilege escalation bugs.

### `REQ-INP-003`: Immediate Input Authorization Revocation
- **Description**: When a session terminates, is paused, or input permission is revoked by the host user, all currently held keys and mouse buttons shall be immediately released on the host OS.
- **Verification Method**: `[T]`
- **Acceptance Criteria**: Simulating connection tear-down while `Key::Shift` and `Mouse::Left` are active results in instantaneous synthetic `KeyUp` and `MouseUp` injection within 10ms, eliminating "stuck key" anomalies.

---

## 6. Functional Requirements: Networking, Transport & Signaling (FUNC-NET)

### `REQ-NET-001`: WebRTC PeerConnection with ICE Optimization
- **Description**: Real-time media (H.264/AV1/VP9) and reliable control channels shall be transmitted over standard WebRTC (`RTCPeerConnection`).
- **Verification Method**: `[T]`
- **Acceptance Criteria**:
  1. Video transmitted over secure SRTP with Congestion Control (TWCC).
  2. Input and control messages transmitted over unordered/ordered SCTP DataChannels (`remote-desk-control`).
  3. Direct host-to-host ICE candidates (Host and Server-Reflexive via STUN) prioritized before Relay candidates.

### `REQ-NET-002`: TURN Relay Fallback
- **Description**: When direct UDP/P2P hole-punching fails (e.g., Symmetric NAT to Symmetric NAT, restrictive enterprise firewalls), the networking subsystem shall seamlessly fall back to TURN over UDP (port 3478) or TLS (port 443).
- **Verification Method**: `[T]`
- **Acceptance Criteria**: End-to-end payload encryption remains intact across TURN relays. The relay server is treated as an untrusted forwarder with zero plaintext inspection ability.

### `REQ-NET-003`: Out-of-Band & Lightweight Signaling Protocols
- **Description**: Signaling shall decouple connection establishment from transport. The core protocol shall define minimal JSON/CBOR signaling payloads (`Offer`, `Answer`, `Candidate`, `AuthChallenge`) capable of transmission via local LAN broadcast (mDNS), direct HTTP/WebSocket, or self-hosted rendezvous servers.
- **Verification Method**: `[T]`
- **Acceptance Criteria**: Signaling messages are idempotent, strictly schema-validated, and reject unauthenticated offers prior to ICE negotiation.

---

## 7. Functional Requirements: User Interface & Interaction (FUNC-UI)

### `REQ-UI-001`: Android Navigation Architecture
- **Description**: The mobile controller application shall implement four distinct primary operational areas:
  1. **Devices Screen**: Saved paired computers, status indicators, quick connect.
  2. **Add Computer**: Pairing workflow, QR scan, manual IP/ID entry.
  3. **Active Session**: Maximized remote desktop canvas with unobtrusive floating overlay.
  4. **Settings & Privacy**: Quality toggles, gesture controls, trusted device management.
- **Verification Method**: `[D]` & `[I]`
- **Acceptance Criteria**: Matches the wireframes in Section 3 of the product specification; active session defaults to immersive full-screen display with auto-hiding floating pill.

### `REQ-UI-002`: Dual Touch Interaction Modes (Direct Touch vs Pointer)
- **Description**: Android client must provide instantaneous switching between:
  - **Direct-Touch Mode**: Direct 1:1 touch-to-remote click mapping, pinch-to-zoom, two-finger scroll.
  - **Mouse-Pointer Mode**: Floating virtual cursor moved via relative dragging on canvas, tap for left-click, two-finger tap for right-click, three-finger drag for window move.
- **Verification Method**: `[T]` & `[D]`
- **Acceptance Criteria**: Mode toggle in floating toolbar instantly switches gesture recognizer pipeline with zero visual stutter; gesture mappings conform strictly to documented specification.

### `REQ-UI-003`: Remote Session Floating Control Bar
- **Description**: The session control bar shall provide:
  - Pointer/Touch mode switch
  - Soft keyboard toggle (with modifier keys: Ctrl, Alt, Super/Win, Esc, Tab)
  - Display aspect ratio fit/stretch/zoom selector
  - Monitor switcher (when remote host has >1 screen)
  - Disconnect button with instant termination
- **Verification Method**: `[D]`
- **Acceptance Criteria**: Control bar is draggable, docks to edges, auto-minimizes to translucent pill after 3 seconds of inactivity, and never permanently covers active desktop canvas areas.

---

## 8. Security & Cryptographic Requirements (SEC-REQ)

### `REQ-SEC-001`: Cryptographic Device Identity & Ed25519 Keys
- **Description**: Every host and controller node shall generate an asymmetric Ed25519 identity keypair stored securely in the host OS secret store (Linux Secret Service / Keyring, Windows Credential Manager, Android Keystore).
- **Verification Method**: `[T]` & `[A]`
- **Acceptance Criteria**: Device IDs are cryptographically derived from the SHA-256 fingerprint of the Ed25519 public key. Private keys are never exported or transmitted over network sockets.

### `REQ-SEC-002`: Mutual Authentication & Pairing Workflow
- **Description**: New connections between untrusted devices require explicit mutual pairing.
- **Verification Method**: `[T]`
- **Acceptance Criteria**:
  1. Controller initiates pairing request with public key fingerprint.
  2. Host OS presents an interactive authorization dialog displaying controller name and 6-digit SAS (Short Authentication String) fingerprint comparison.
  3. Unpaired or rejected controllers cannot receive screen frames or inject input.

### `REQ-SEC-003`: OWASP MASVS Security Verification
- **Description**: Android controller application shall conform to OWASP MASVS v2.0 Level 1 (MASVS-L1):
  - `MASVS-STORAGE`: Sensitive tokens/keys stored solely in `EncryptedSharedPreferences` / Hardware Keystore.
  - `MASVS-CRYPTO`: Industry-standard AEAD primitives (ChaCha20-Poly1305 / AES-256-GCM).
  - `MASVS-AUTH`: Local biometric or PIN lock option before initiating connection.
  - `MASVS-NETWORK`: TLS 1.3 enforced for signaling; strict certificate pinning support.
- **Verification Method**: `[T]` & `[I]`
- **Acceptance Criteria**: Static and dynamic security analysis passes with zero critical or high vulnerabilities.

### `REQ-SEC-004`: Zero Recording & Ephemeral Memory Guarantees
- **Description**: The application host and controller shall not persist video frame buffers to disk or swap storage. Diagnostic logs shall redact frame pointers, keyboard scancodes, clipboard contents, and credentials.
- **Verification Method**: `[T]` & `[A]`
- **Acceptance Criteria**: Automated test scans log outputs during active typing and screen streaming; asserts zero occurrences of plain-text passwords or secret keys.

---

## 9. Non-Functional Performance Requirements (NFR-PERF)

| Requirement ID | Metric | Target (LAN) | Target (WAN / Relay) | Verification Method |
|---|---|---|---|---|
| `REQ-PERF-001` | Input-to-Visible-Response Latency | Median < 35ms, P95 < 60ms | Median < 80ms, P95 < 130ms | `[T]` (High-speed optical / timestamp probe) |
| `REQ-PERF-002` | Stream Frame Rate | 60 FPS (sustained) | 30-60 FPS (adaptive bandwidth) | `[T]` (Frame interval jitter monitor) |
| `REQ-PERF-003` | Frame Drop Rate | < 0.2% on 1 Gbps LAN | < 2.0% under 2% simulated packet loss | `[T]` (Sequence counter audit) |
| `REQ-PERF-004` | Host Idle CPU Usage | < 0.5% CPU (single core) | < 0.5% CPU | `[T]` (`/proc/stat` & `perf` profiler) |
| `REQ-PERF-005` | Host Idle RAM Usage | < 60 MB RSS | < 60 MB RSS | `[T]` (Massif / Memory tracker) |
| `REQ-PERF-006` | Controller Active RAM | < 120 MB RSS (Android) | < 120 MB RSS (Android) | `[T]` (Android Profiler dumpsys) |
| `REQ-PERF-007` | Connection Setup Time | < 1.2s (P2P LAN) | < 2.5s (STUN / TURN negotiated) | `[T]` (Signaling handshake timer) |

---

## 10. Standards & Engineering Framework Compliance

### ISO/IEC/IEEE 12207:2026 Process Mapping
- **System Analysis & Design**: Documented in `architecture.md` and `ui-spec.md`.
- **Software Construction**: Clean separation of modules, unit-test-first implementation.
- **Software Verification**: Automated test suites running in CI with documented test commands.
- **Release Management**: Controlled milestone gates from Phase 0 to Phase 9.

### IEEE 730:2026 Software Quality Assurance
- Verification checklists enforced before code merging.
- Traceability between Requirements (`REQ-*`), Tests (`TEST-*`), and Source Modules.

---

## 11. Traceability Matrix Summary

| Requirement ID | Module Owner | Test Suite Reference | Milestone Phase |
|---|---|---|---|
| `REQ-SYS-001` | Architecture Core | `crates/remote-desk-core/tests/cross_platform.rs` | Phase 1 |
| `REQ-SYS-002` | Networking Core | `crates/remote-desk-core/tests/direct_p2p.rs` | Phase 1 |
| `REQ-CAP-001` | Linux Backend | `crates/remote-desk-linux/tests/portal_screencast.rs` | Phase 2 |
| `REQ-CAP-002` | Linux Backend | `crates/remote-desk-linux/tests/x11_capture.rs` | Phase 2 |
| `REQ-CAP-003` | Windows Backend | `crates/remote-desk-windows/tests/dxgi_capture.rs` | Phase 7 |
| `REQ-INP-001` | Shared Protocol | `crates/remote-desk-proto/tests/input_mapping.rs` | Phase 4 |
| `REQ-INP-002` | Linux Backend | `crates/remote-desk-linux/tests/input_injection.rs` | Phase 4 |
| `REQ-INP-003` | Linux Backend | `crates/remote-desk-linux/tests/stuck_key_revocation.rs` | Phase 4 |
| `REQ-NET-001` | Networking Agent | `crates/remote-desk-net/tests/webrtc_p2p.rs` | Phase 8 |
| `REQ-NET-002` | Networking Agent | `crates/remote-desk-net/tests/turn_fallback.rs` | Phase 8 |
| `REQ-UI-001` | Android Agent | `android/app/src/androidTest/NavigationTest.kt` | Phase 3 |
| `REQ-UI-002` | Android Agent | `android/app/src/androidTest/GestureModeTest.kt` | Phase 3 |
| `REQ-SEC-001` | Security Agent | `crates/remote-desk-core/tests/device_identity.rs` | Phase 5 |
| `REQ-SEC-002` | Security Agent | `crates/remote-desk-core/tests/mutual_pairing.rs` | Phase 5 |
| `REQ-SEC-003` | Security Agent | `android/app/src/androidTest/OwaspMasvsTest.kt` | Phase 5 |
| `REQ-PERF-001` | Performance Agent | `benches/latency_e2e.rs` | Phase 6 |
