# Design Reference Study: Remote Desktop Systems & Interaction Benchmarks

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-REF-002 |
| **Author / Agent** | UI/UX & Systems Research Agent |
| **Target Platforms** | Android Controller, Linux Host/Controller, Windows Host/Controller |
| **Status** | Approved Reference Baseline |

---

## 1. Objective & Scope

The purpose of this study is to ground Remote-Desk's interface and architectural interaction patterns in observable, documented behaviors of leading reference platforms:
1. **Microsoft Windows App / Remote Desktop Client (Android v10.0.19+, Windows v1.2.5+)**
2. **RustDesk (v1.2.3 - v1.3.0)**
3. **GNOME Remote Desktop (v44 - v46) & XDG Desktop Portal (v1.16 - v1.18)**
4. **OWASP Mobile Application Security Verification Standard (MASVS v2.0)**

Rather than designing from speculative imagination or copying proprietary visual assets, Remote-Desk synthesizes the ergonomic strengths of Microsoft's touch interaction patterns with RustDesk's decentralized peer-to-peer connection paradigm, while strictly adhering to open standards for desktop integration and privacy.

---

## 2. Microsoft Windows App / Remote Desktop Reference Analysis

### 2.1 Evaluated Systems & Official Documentation
- **Reference Application**: Microsoft Remote Desktop (Android Package `com.microsoft.rdc.android`) / Windows App.
- **Official Specification Source**: *Microsoft Learn: "Get started with the Android client" & "Remote Desktop touch gestures and mouse modes"*.

### 2.2 Navigation & Visual Hierarchy
```
+-----------------------------------------------------------+
|               MICROSOFT RD ANDROID WORKFLOW               |
+-----------------------------------------------------------+
|  [Tabs: PCs | Workspaces]                                 |
|  - Saved Desktop Tiles (Host Name, IP/FQDN, Status)       |
|  - FAB (+) -> Add PC / Add Workspace                      |
|       |                                                   |
|       v                                                   |
|  [Connect Trigger] -> Progress Modal ("Configuring...")   |
|       |                                                   |
|       v                                                   |
|  [Active Canvas] Full screen + Floating Connection Bar    |
|  - Top-docked expandable bar: [Pan] [Keyboard] [Menu ⋮]   |
+-----------------------------------------------------------+
```

### 2.3 Interaction Modes: Direct-Touch vs. Mouse-Pointer Mode
Microsoft Remote Desktop establishes the industry standard for mapping mobile touchscreens to desktop mouse/pointer operating systems:

| Interaction Action | Direct-Touch Mode (Observed) | Mouse-Pointer Mode (Observed) | Remote-Desk Adoption Decision |
|---|---|---|---|
| **Single-finger tap** | Left-click at exact touched coordinates | Left-click at current position of visible pointer cursor | **Adopt**: Identical dual-mode model. |
| **Single-finger drag** | Direct manipulation (touch-scrolling or window drag if target supports touch) | Moves the visible cursor relatively across the remote canvas | **Adopt**: Crucial for small UI targets (Windows/Linux taskbars). |
| **Single-finger tap and hold** | Right-click (context menu) at touched location | Left-click down and hold (drag-and-drop mode) | **Adopt**: Consistent with touch OS expectations. |
| **Two-finger tap** | Right-click at position | Right-click at visible cursor location | **Adopt**: Familiar secondary click gesture. |
| **Two-finger drag (vertical)** | Scroll content vertically | Scroll mouse wheel at cursor position | **Adopt**: Standard smooth scrolling gesture. |
| **Pinch-in / Pinch-out** | Zoom canvas in/out | Zoom canvas in/out | **Adopt**: Smooth zooming with viewport boundary clamping. |

### 2.4 Ergonomic Deficiencies & Privacy Issues in Microsoft RD
1. **Connection Bar Intrusion**: The top-docked connection bar frequently obscures top menus in Linux (GNOME top bar) and Windows (browser tabs, window title bars).
   - *Remote-Desk Improvement*: Introduce a floating, edge-docking pill that automatically collapses into a subtle 40% translucent edge tab after 3 seconds of cursor/touch idle.
2. **Account Requirement & Cloud Lock-in**: Windows App is heavily biased toward Microsoft Azure/Entra ID workspaces, making ad-hoc local connections cumbersome.
   - *Remote-Desk Improvement*: Zero cloud dependencies; direct device pairing via local discovery (mDNS) or cryptographic device ID.

---

## 3. RustDesk Architecture & UX Reference Analysis

### 3.1 Evaluated Systems & Documentation
- **Reference Application**: RustDesk (Open Source, v1.2.3 - v1.3.0).
- **Documentation Sources**: *RustDesk Documentation: "Self-hosting", "Linux Wayland support", and "Permissions"*.

### 3.2 Observed Strengths
```
+-----------------------------------------------------------+
|                   RUSTDESK PEER-TO-PEER FLOW              |
+-----------------------------------------------------------+
|  Host UI: "Your Desk" [ID: 9-digit] [One-time Password]   |
|  Controller UI: "Control Remote Desk" [Enter Remote ID]   |
|       |                                                   |
|       v                                                   |
|  Rendezvous Server (hbbs/hbbr)                            |
|  - Facilitates NAT punch-through via UDP hole punching     |
|  - Relays encrypted packets if symmetric NAT prevents P2P |
+-----------------------------------------------------------+
```
- **Strengths Adopted**:
  - Distinct numerical/alphanumeric device ID for locator routing.
  - Zero mandatory account registration.
  - Architecture cleanly separated into Core, Platform Capture, and UI.

### 3.3 Critical Architectural Limitations & Security Pitfalls Observed in RustDesk
1. **Device ID Treated as Credential**: In default RustDesk setups, attackers scanning numerical IDs with brute-force passwords can probe hosts.
   - *Remote-Desk Decision*: Device ID is strictly a **locator / routing handle**, NEVER an authentication credential. Mutual pairing requires Ed25519 public key pinning and explicit interactive approval on the host desktop.
2. **Wayland & Linux Fragmentation Workarounds**: RustDesk historically relied on launching a custom background service or falling back to root-privileged X11 emulations.
   - *Remote-Desk Decision*: Strict alignment with user-space standards: `xdg-desktop-portal` ScreenCast and RemoteDesktop APIs via PipeWire, eliminating setuid binaries or raw `/dev/uinput` root execution wherever possible.

---

## 4. Linux Desktop Integration: GNOME Remote Desktop & XDG Portals

### 4.1 Evaluated Specifications
- **Freedesktop Specifications**:
  - `org.freedesktop.portal.ScreenCast`
  - `org.freedesktop.portal.RemoteDesktop`
  - `PipeWire SPA (Simple Plugin API)` for zero-copy DMA-BUF buffers.

### 4.2 Portal Session Lifecycle & Permission Model
```
Controller                         Host Portal                 PipeWire Daemon
    |                                   |                             |
    |---- CreateSession() ------------->|                             |
    |                                   |                             |
    |---- SelectSources(Screen/Window)->|                             |
    |                                   |-- Prompt User for Access -->|
    |                                   |<-- User selects Display ----|
    |                                   |                             |
    |---- StartSession() -------------->|                             |
    |                                   |-- OpenPipeWireRemote() ---->|
    |                                   |<-- Return PipeWire Node ID -|
    |<--- Session Started (Node ID) ----|                             |
    |                                                                 |
    |---- Connect to PipeWire Stream (Node ID) ---------------------->|
    |<--- Receive Zero-Copy DMA-BUF Video Frames ---------------------|
```

### 4.3 Why Portal Integration is Mandatory for Modern Linux
- On Wayland (GNOME/Mutter, KDE/KWin, Sway/wlroots), direct screen reading via `/dev/fb0` or raw X11 calls is blocked by compositor security boundaries.
- The portal enforces transparent user consent: the host operating system shows a system-native dialog detailing which displays are shared.
- Remote input must be requested via `RemoteDesktop` portal with explicit flags (`Pointer`, `Keyboard`, `Touch`).

---

## 5. WebRTC Transport & Signaling Reference Analysis

### 5.1 Standards Evaluated
- **IETF RFC 8825**: *Overview: WebRTC in Real-Time Communications*.
- **IETF RFC 8831**: *WebRTC Data Channels*.
- **IETF RFC 8835**: *Transports for WebRTC*.

### 5.2 Latency Optimization Pipeline
```
Raw Frame (VRAM)
   │ (Zero-copy DMA-BUF / MemFd)
   ▼
Hardware Encoder (VA-API / NVENC / MediaCodec)
   │ (Low-latency preset, zero B-frames, CBR)
   ▼
RTP Packetizer / SRTP Encryption (DTLS 1.3)
   │ (TWCC congestion control, paced queue)
   ▼
UDP Socket (Direct P2P or TURN Relay)
   │
   ▼
RTP Depacketizer / Jitter Buffer (Minimal 0-1 frame buffer)
   │
   ▼
Hardware Decoder (MediaCodec on Android / Vulkan Video / VA-API)
   │ (Direct surface presentation)
   ▼
Display (SurfaceView / 60-120Hz refresh)
```

### 5.3 Key Design Decisions
- **Video Transport**: Standard WebRTC SRTP media track. WebRTC handles jitter buffering, network pacing, packet retransmissions (NACK), and congestion adaptation (TWCC).
- **Control & Input**: SCTP DataChannel configured with `maxRetransmits: 0` and `ordered: false` for mouse cursor movement (loss-tolerant, latency-critical), while keyboard and system commands use `ordered: true` (loss-intolerant).

---

## 6. OWASP MASVS Mobile Security Benchmark

The Android controller implementation is evaluated against the **OWASP Mobile Application Security Verification Standard (MASVS v2.0)**:

| Category | Reference Requirement | Remote-Desk Architecture Implementation |
|---|---|---|
| **MASVS-STORAGE** | System credential storage must use hardware-backed keystore. | Host pairing certificates and private identity keys are stored in Android Keystore with `KeyGenParameterSpec.PURPOSE_SIGN` and `setUserAuthenticationRequired(false)` for session keys, with optional biometric prompt for stored trusted hosts. |
| **MASVS-CRYPTO** | Never use broken or deprecated ciphers (e.g. SHA-1, DES, ECB mode). | Cryptographic primitives locked to Ed25519 (signatures), X25519 (Diffie-Hellman), ChaCha20-Poly1305 / AES-256-GCM (AEAD), and SHA-256 / BLAKE3 (hashing). |
| **MASVS-NETWORK** | Data in transit must be protected by state-of-the-art TLS/DTLS. | Signaling channels require TLS 1.3 with certificate fingerprint verification. Media/Data channels encrypted via DTLS 1.3 / SRTP. |
| **MASVS-PLATFORM**| Proper permission hygiene and IPC security. | No exported activities without explicit permissions. Tapjacking protection via `filterTouchesWhenObscured = true` on sensitive pairing dialogs. |

---

## 7. Synthesis: Concrete Design Decisions for Remote-Desk

1. **Retain Microsoft's Intuitive Input Gestures**: Implement distinct Direct-Touch and Mouse-Pointer modes with identical tap, two-finger, and pinch mappings.
2. **Overcome Microsoft's UI Clutter**: Replace the rigid top connection bar with a sleek, draggable floating pill that docks unobtrusively to screen perimeters.
3. **Adopt RustDesk's Decentralization without its Security Weaknesses**: Support direct local IP/mDNS connections and optional self-hosted relay signaling, but require cryptographic public-key pairing (SAS fingerprint) before granting desktop access.
4. **Standardize on Freedesktop Portals for Linux**: Full Wayland first-class support via `xdg-desktop-portal` ScreenCast and PipeWire DMA-BUF buffer sharing.
5. **No Proprietary Assets**: Use 100% original icons, SVG glyphs, and typography (Material 3 / Jetpack Compose on Android; Adwaita / native system fonts on Linux/Windows).
