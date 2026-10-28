# Proposal: Initial Core Platform Specification & Prototype Architecture

## Why
Users and organizations requiring low-latency remote desktop capabilities currently face a trade-off between proprietary, privacy-invasive commercial platforms (such as Microsoft Remote Desktop with mandatory cloud accounts or AnyDesk/TeamViewer) and complex open-source tools with fragmented desktop integration and security assumptions. Remote-Desk delivers an original, open-source, low-latency, and privacy-first remote desktop solution combining the familiar touch/mouse ergonomics of Microsoft Remote Desktop with the decentralized P2P and self-hosting design of RustDesk.

## What Changes
- Establish standard specifications conforming to ISO/IEC/IEEE 12207:2026, IEEE 730:2026, and OWASP MASVS v2.0.
- Implement shared cross-platform core in Rust (`remote-desk-core`, `remote-desk-proto`, `remote-desk-net`).
- Implement Linux screen capture via modern `xdg-desktop-portal` ScreenCast and PipeWire DMA-BUF buffer exchange with X11 MIT-SHM fallback (`remote-desk-linux`).
- Implement Android controller architecture using Kotlin and Jetpack Compose (`android/`).
- Enforce strict cryptographic mutual pairing via Ed25519 identity keys and 6-digit SAS verification, with no default screen recording and immediate fail-safe input revocation.

## Capabilities

### New Capabilities
- `remote-core-proto`: Cross-platform binary wire protocol, message framing, serialization, and input/control types.
- `session-orchestration`: State machine managing cryptographic handshake, pairing, ICE connection, and session revocation.
- `linux-capture-engine`: Wayland portal/PipeWire and X11 screen capture abstraction with monitor enumeration.
- `android-controller-ui`: Jetpack Compose mobile viewer with dual touch/pointer modes and floating toolbar.

### Modified Capabilities
*(None - initial baseline capability creation)*

## Impact
- New Cargo workspace supporting Linux and Android builds.
- Native system dependencies: `libdbus`, `libpipewire-0.3`, `libx11`, `libxshm`.
- Zero database requirements; purely local encrypted key storage.
