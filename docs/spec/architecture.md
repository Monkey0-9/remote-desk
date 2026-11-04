# System Architecture & Technical Specifications: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-ARCH-004 |
| **Author / Agent** | Architecture & Systems Engineering Agent |
| **Standards Compliance** | ISO/IEC/IEEE 12207:2026, IEEE 730:2026 |
| **Implementation Core** | Rust (Edition 2021/2024), Kotlin/Compose (Android), Native C-FFI/UniFFI |
| **Status** | Approved Architectural Baseline |

---

## 1. Architectural Philosophy & Layered Decomposition

Remote-Desk is built on strict architectural boundaries separating:
1. **The Shared Core (`remote-desk-core`)**: 100% platform-agnostic, deterministic, memory-safe Rust codebase housing cryptographic identities, state machines, protocol serialization, session lifecycle, and signaling negotiation.
2. **Platform Capture & Input Backends (`remote-desk-linux`, `remote-desk-windows`)**: Native host OS integration modules implementing well-defined traits for screen frame extraction, display topology discovery, and synthetic input injection.
3. **Transport & Media Subsystem (`remote-desk-net`)**: WebRTC-based P2P data and media pipeline with ICE NAT traversal and TURN fallback.
4. **Target Clients**: Native Android client (Kotlin + Jetpack Compose + Wire Protocol Codec) and Web/Desktop Management Dashboard.

```
+-------------------------------------------------------------------------+
|                              CLIENT LAYER                               |
|   +------------------------------------+  +-------------------------+   |
|   | Android Client (Kotlin / Compose)  |  | Dashboard SPA / Web UI  |   |
|   +------------------------------------+  +-------------------------+   |
|                     │                                  │                |
|                     ▼ (Wire Framing / Socket)          ▼ (REST / WS)    |
+-------------------------------------------------------------------------+
|                            SHARED RUST CORE                             |
|  +-------------------------------------------------------------------+  |
|  | remote-desk-core                                                  |  |
|  | - Session State Machine       - Cryptographic Identity (Ed25519)  |  |
|  | - Peer Pairing Orchestrator   - Protocol Codecs & Framing         |  |
|  | - Configuration Management    - Diagnostic Metrics & Telemetry    |  |
|  +-------------------------------------------------------------------+  |
|            │                                              │             |
|            ▼                                              ▼             |
|  +---------------------------+             +--------------------------+ |
|  | remote-desk-net           |             | remote-desk-proto        | |
|  | - WebRTC RTCPeerConnection|             | - Wire Schemas (Bincode/ | |
|  | - ICE / STUN / TURN       |             |   Protobuf / JSON)       | |
|  | - SCTP DataChannels       |             | - Input Events           | |
|  | - Signaling Transports    |             | - Control Packets        | |
|  +---------------------------+             +--------------------------+ |
+-------------------------------------------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
|                        PLATFORM BACKENDS (HOST)                         |
|   +----------------------------------+ +------------------------------+ |
|   | remote-desk-linux                | | remote-desk-windows          | |
|   | - Wayland Portal (ScreenCast)    | | - DXGI Desktop Duplication   | |
|   | - PipeWire DMA-BUF Stream        | | - Windows Graphics Capture   | |
|   | - X11 MIT-SHM / XDamage          | | - Win32 SendInput            | |
|   | - RemoteDesktop Portal / Evdev   | | - Virtual Display Driver     | |
|   +----------------------------------+ +------------------------------+ |
+-------------------------------------------------------------------------+
```

---

## 2. Workspace Crate Layout & Modular Boundaries

The project is structured as a modular Cargo workspace:

```
remote-desk/
├── Cargo.toml                     # Workspace root manifest
├── crates/
│   ├── remote-desk-proto/         # Wire messages, codecs, event structs
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── messages.rs        # Input, Session, Control messages
│   │       └── codec.rs           # Serialization (bincode / protobuf)
│   ├── remote-desk-core/          # Platform-agnostic state machine & crypto
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── identity.rs        # Ed25519 key generation, sign, verify
│   │       ├── pairing.rs         # SAS generation, mutual authentication
│   │       ├── session.rs         # State machine: Handshake -> Active -> Closed
│   │       ├── storage.rs         # Local encrypted key/config traits
│   │       └── uniffi_api.rs      # Exported bindings for Android/Kotlin
│   ├── remote-desk-net/           # WebRTC and signaling abstraction
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── peer.rs            # WebRTC PeerConnection wrapper
│   │       ├── data_channel.rs    # SCTP reliable/unreliable channels
│   │       ├── signaling.rs       # mDNS, HTTP, WebSocket signaling traits
│   │       └── ice.rs             # STUN / TURN configuration parser
│   ├── remote-desk-linux/         # Linux host capture and input injection
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── wayland_portal.rs  # org.freedesktop.portal.ScreenCast
│   │       ├── pipewire.rs        # PipeWire DMA-BUF buffer listener
│   │       ├── x11_capture.rs     # XShm / XDamage fallback
│   │       └── input_portal.rs    # org.freedesktop.portal.RemoteDesktop & evdev
│   └── remote-desk-windows/       # Windows host capture and input injection
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── dxgi_duplication.rs# IDXGIOutputDuplication implementation
│           └── send_input.rs      # Windows SendInput API
├── android/                       # Native Android mobile controller application
│   ├── app/
│   │   ├── src/main/java/org/remotedesk/
│   │   │   ├── ui/                # Jetpack Compose UI (Devices, Session, Canvas)
│   │   │   ├── input/             # Touch-to-pointer gesture recognizer
│   │   │   ├── webrtc/            # WebRTC SurfaceViewRenderer & MediaCodec
│   │   │   └── core/              # UniFFI Rust bindings bridge
│   └── build.gradle.kts
└── docs/                          # Architectural & specification documents
```

---

## 3. Core Interface Contracts & Traits

### 3.1 Screen Capture Trait (`ScreenCaptureBackend`)
The screen capture trait abstracts frame extraction regardless of the underlying operating system:

```rust
use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub is_primary: bool,
}

#[derive(Debug)]
pub enum FrameBuffer {
    /// Zero-copy Linux DMA-BUF file descriptor with stride and modifier
    DmaBuf {
        fd: std::os::unix::io::RawFd,
        width: u32,
        height: u32,
        stride: u32,
        format_fourcc: u32,
    },
    /// Shared memory buffer (X11 MIT-SHM or Windows staging buffer)
    ShmMem {
        data: Vec<u8>,
        width: u32,
        height: u32,
        stride: u32,
    },
}

#[derive(Error, Debug)]
pub enum CaptureError {
    #[error("Permission denied by system portal")]
    PermissionDenied,
    #[error("Display disconnected")]
    DisplayDisconnected,
    #[error("PipeWire stream error: {0}")]
    PipeWireError(String),
    #[error("DXGI duplication error: {0}")]
    DxgiError(String),
}

#[async_trait]
pub trait ScreenCaptureBackend: Send + Sync {
    /// Discover attached displays
    async fn enumerate_monitors(&self) -> Result<Vec<MonitorInfo>, CaptureError>;
    
    /// Initialize capture session for a specific display target
    async fn start_capture(&mut self, monitor_id: u32) -> Result<(), CaptureError>;
    
    /// Pull the next raw frame (blocks until next VSync or damage region)
    async fn next_frame(&mut self) -> Result<FrameBuffer, CaptureError>;
    
    /// Gracefully stop the capture stream
    async fn stop_capture(&mut self) -> Result<(), CaptureError>;
}
```

### 3.2 Remote Input Trait (`InputInjectorBackend`)
Input injection is abstracted to prevent platform leaks:

```rust
use crate::messages::{KeyAction, MouseButton, MouseWheelDelta};

#[async_trait]
pub trait InputInjectorBackend: Send + Sync {
    /// Injects mouse movement (absolute normalized 0.0..1.0 or relative dx/dy)
    async fn move_pointer(&mut self, x: f32, y: f32, relative: bool) -> Result<(), InputError>;
    
    /// Injects mouse button press or release
    async fn mouse_button(&mut self, button: MouseButton, down: bool) -> Result<(), InputError>;
    
    /// Injects mouse wheel scroll
    async fn mouse_wheel(&mut self, delta: MouseWheelDelta) -> Result<(), InputError>;
    
    /// Injects keyboard key press/release using standardized scancodes
    async fn keyboard_key(&mut self, scancode: u32, down: bool) -> Result<(), InputError>;
    
    /// CRITICAL SECURITY CALL: Instantly release all pressed keys and buttons
    async fn release_all(&mut self) -> Result<(), InputError>;
}
```

---

## 4. End-to-End Data Flow & Media Pipeline

```
[HOST SCREEN]
      │
      ▼ (PipeWire DMA-BUF / DXGI Surface)
[ScreenCaptureBackend]
      │
      ▼ (Raw Frame Buffer)
[Hardware Video Encoder] (H.264 / AV1)
      │
      ▼ (Encoded NAL Units / Annex-B)
[WebRTC VideoTrackSource]
      │
      ▼ (SRTP Encrypted Packets)
================ Network Boundary (P2P / UDP) ================
      │
      ▼ (SRTP Depacketizer)
[Android WebRTC VideoTrack]
      │
      ▼ (MediaCodec Hardware Decoder)
[SurfaceView Presentation Engine]
      │
[CONTROLLER DISPLAY]
```

### Input Backchannel Data Flow:
```
[Android Touch Surface / Keyboard]
      │
      ▼ (Gesture Processing Engine)
[Input Event Serialization] (Proto / Bincode)
      │
      ▼ (SCTP DataChannel: "remote-desk-control")
================ Network Boundary (P2P / UDP) ================
      │
      ▼ (Deserialization & Boundary Check)
[Authorization Filter] (Is Session Active & Allowed?)
      │
      ▼ (Validated Event)
[InputInjectorBackend] (XDG RemoteDesktop / Win32 SendInput)
      │
[HOST OPERATING SYSTEM EVENT LOOP]
```

---

## 5. Concurrency & Asynchronous Architecture

Remote-Desk utilizes **Tokio** as its primary asynchronous runtime in Rust, adhering to strict concurrency guidelines:
1. **Zero Busy-Waiting**: All frame polling is driven by event loops (`epoll` on Linux, `WaitForVBlank` on Windows).
2. **Dedicated Media Threading**:
   - `MediaEncodingThread`: Dedicated high-priority OS thread receiving DMA-BUF frames and feeding hardware encoders to prevent frame-queue bottlenecks.
   - `InputDispatchThread`: Low-latency async task processing SCTP DataChannel packets directly to input injection with zero heap reallocations.
3. **Cancellation Safety**: All asynchronous tasks support clean cooperative cancellation via `tokio::sync::watch` or `CancellationToken`.

---

## 6. Android Client Architecture & Wire Protocol Integration

The Android application uses a clean Model-View-ViewModel (MVVM) architecture with Jetpack Compose:

```
[UI Views: Compose]
      ▲
      │ (StateFlow / UI Events)
[ViewModels: DeviceListViewModel, SessionViewModel]
      ▲
      │ (Kotlin Coroutines / Flow)
[Repository: SessionRepository, KeyStoreManager]
      ▲
      │ (Binary Wire Framing Codec)
[Network Layer: LanSessionClient / WebRtcSessionManager]
```

- **Binary Wire Framing Codec**: Direct 16-byte header packing and little-endian input serialization matching `remote-desk-proto`.
- **Zero-Copy Surface Rendering**: Android's `SurfaceView` handle receives video frames directly, allowing hardware decoders to blit frames directly to display buffers.

---

## 7. Storage Architecture: Local-First & Zero Mandatory Central DB

Remote-Desk explicitly rejects mandatory central databases:
- **Host Configuration**: Stored in standard XDG configuration paths on Linux (`~/.config/remote-desk/config.toml`) and `%APPDATA%\remote-desk\config.toml` on Windows.
- **Cryptographic Secrets**:
  - Linux: Protected via `libsecret` (GNOME Keyring / KWallet).
  - Windows: Protected via DPAPI (`CryptProtectData`) / Windows Credential Manager.
  - Android: Encrypted using Android Keystore System (`MasterKey` with AES-256-GCM).
- **Paired Devices Ledger**: A compact, local JSON/TOML ledger mapping public key fingerprints (`ed25519:8f3c...`) to assigned aliases, last seen addresses, and authorization scopes (Full Control vs View-Only).
