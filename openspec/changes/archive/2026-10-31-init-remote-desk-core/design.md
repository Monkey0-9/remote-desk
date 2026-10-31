# Design: Core Platform Architecture & Prototype Implementation

## Context
See `proposal.md` and `docs/spec/architecture.md`. Remote-Desk requires a multi-crate Rust workspace decoupling shared protocols, crypto identities, and session orchestration from Linux/Windows native desktop capture and Android mobile client layers.

## Goals / Non-Goals

**Goals:**
- Provide a clean Cargo workspace containing `remote-desk-proto`, `remote-desk-core`, `remote-desk-net`, and `remote-desk-linux`.
- Implement binary protocol serialization and framing with zero-copy deserialization where feasible.
- Abstract Linux screen capture behind a uniform `ScreenCaptureBackend` trait supporting Wayland Portal and X11 MIT-SHM.
- Structure Android controller with Jetpack Compose and UniFFI JNI bridge.

**Non-Goals:**
- Centralized user database or cloud authentication account servers.
- Unverified screen recording or cloud session storage.
- iOS or macOS platform backends in initial milestone.

## Decisions

### 1. Rust for Shared Core and Protocol
- **Rationale**: Rust guarantees memory safety, zero-cost abstractions, deterministic performance, and cross-compilation across Linux and Android (`cargo-ndk`).
- **Alternatives Considered**: C++20 (rejected due to memory safety risks and tooling complexity), Go (rejected due to garbage collection jitter impacting 60 FPS video pipelines).

### 2. WebRTC for Real-Time Transport with Dual DataChannels
- **Rationale**: WebRTC provides proven SRTP media encryption, TWCC network congestion adaptation, and NAT hole punching via ICE/STUN/TURN. DataChannels allow separating loss-tolerant mouse movement (unreliable/unordered) from reliable control packets.
- **Alternatives Considered**: Custom UDP protocol (rejected due to lack of standard STUN/TURN traversal and encryption audit).

### 3. Freedesktop Portal over Raw X11/uinput for Modern Linux
- **Rationale**: Wayland compositors (GNOME, KDE) enforce strict isolation. The `xdg-desktop-portal` ScreenCast and RemoteDesktop APIs provide legitimate, permission-aware access.
- **Alternatives Considered**: Direct `/dev/uinput` daemon (retained only as optional headless fallback; portal preferred for desktop sessions).

## Risks / Trade-offs

- **[Risk: Wayland Portal User Dialog Overhead]** → The initial ScreenCast session requires interactive user confirmation on the host. *Mitigation*: Re-use existing portal session tokens for reconnecting paired controllers where supported by compositors.
- **[Risk: Android Hardware Video Decoder Fragmentation]** → Diverse Android SoCs handle H.264/AV1 hardware decoding differently. *Mitigation*: Leverage Google WebRTC Android SDK's `DefaultVideoDecoderFactory` with automatic software fallback (`libvpx` / `openh264`).
- **[Risk: Stuck Input Keys on Sudden Network Drop]** → Controller disconnection while keys are held down could lock the host. *Mitigation*: Mandatory `release_all()` invocation upon session cancellation token trigger.
