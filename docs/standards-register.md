# Standards Compliance Register: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-REG-STD-010 |
| **Lifecycle Standards** | ISO/IEC/IEEE 12207:2026, IEEE 730:2026 |
| **Security Standards** | OWASP MASVS v2.0, NIST SSDF SP 800-218 |
| **Status** | Verified Baseline & Audit Trail |

---

## 1. ISO/IEC/IEEE 12207:2026 Software Life Cycle Process Traceability

| Process Clause | Clause Description | Project Implementation Control | Evidence & Verification Reference |
|---|---|---|---|
| **6.4.1** | Business or Mission Analysis | Definition of problem statement, privacy-first tenet, no mandatory cloud database. | `docs/spec/requirements.md` (§1), `openspec/changes/init-remote-desk-core/proposal.md` |
| **6.4.2** | Stakeholder Needs & Requirements Definition | Reference study of Microsoft Windows App, RustDesk, and GNOME Remote Desktop. | `docs/spec/reference-study.md` (RD-SPEC-REF-002) |
| **6.4.3** | System Requirements Definition | Formal numbered requirements (`REQ-SYS-*`, `REQ-CAP-*`, `REQ-INP-*`, `REQ-NET-*`, `REQ-UI-*`, `REQ-SEC-*`, `REQ-PERF-*`). | `docs/spec/requirements.md` (RD-SPEC-REQ-001) |
| **6.4.4** | System Architecture Definition | Modular Cargo workspace: Core, Proto, Linux, Net, Signal, Bench, and Android client. | `docs/spec/architecture.md` (RD-SPEC-ARCH-004) |
| **6.4.5** | Design Definition | UI interaction specifications, dual touch/pointer modes, wireframes, STRIDE threat model. | `docs/spec/ui-spec.md`, `docs/spec/threat-model.md` |
| **6.4.6** | System Analysis Process | Empirical workload definitions (WL-01 to WL-10), latency probe, comparative benchmarks. | `docs/spec/benchmark-plan.md`, `crates/remote-desk-bench` |
| **6.4.7** | Implementation Process | Rust 2021 shared core, Kotlin/Compose Android controller, native Linux portal/SHM capture. | `crates/`, `android/app` |
| **6.4.8** | Integration Process | Verified loopback prototype demonstrating identity, DH handshake, SAS code, capture, input, and revocation. | `crates/remote-desk-linux/examples/loopback_prototype.rs` |
| **6.4.9** | Verification Process | Automated unit & integration tests (`TEST-PROTO-*`, `TEST-CORE-*`, `TEST-CAP-*`, `TEST-INP-*`). | `docs/spec/verification-plan.md`, `cargo test --workspace` |
| **6.4.11** | Transition & Hardening | Phased milestone roadmap with strict exit criteria (Phases 0 through 9). | `docs/spec/roadmap.md` (RD-SPEC-ROADMAP-009) |

---

## 2. IEEE 730:2026 Software Quality Assurance (SQA) Compliance

| SQA Requirement | Quality Gate Control | Enforcement Mechanism |
|---|---|---|
| **Clause 5.1: SQA Process Planning** | Milestone roadmap with explicit entry/exit criteria and zero unverified completion claims. | `docs/spec/roadmap.md`, `docs/spec/verification-plan.md` |
| **Clause 5.2: Product Quality Evaluation** | Mandatory Task Acceptance Checklist referencing requirement IDs, unit tests, and performance data. | Every PR/Commit verified against checklist; zero clippy/fmt warnings. |
| **Clause 5.3: Process Quality Assurance** | OpenSpec capability-driven change tracking and delta specifications. | `openspec validate --all` executed before changes are committed. |
| **Clause 5.4: Anomaly Resolution** | Immediate fail-safe input revocation on disconnection (`release_all()`). | `crates/remote-desk-linux/tests/linux_tests.rs::test_stuck_key_revocation_on_disconnect` |

---

## 3. OWASP Mobile Application Security Verification Standard (MASVS v2.0)

| MASVS Control ID | Control Category | Mobile Implementation Control | Verification Status |
|---|---|---|---|
| **MASVS-STORAGE-1** | Sensitive Data Storage | Cryptographic keys and peer credentials stored in Android Keystore / `SecureKeyStore` with secure memory wipe. | `android/app/src/main/java/org/remotedesk/security/SecureKeyStore.kt` [VERIFIED] |
| **MASVS-STORAGE-2** | Ambient Data Leakage | Zero ambient logging of screen buffers or keystrokes; no screen recording by default. | `docs/spec/ui-spec.md` (Screen E), `crates/remote-desk-proto` [VERIFIED] |
| **MASVS-CRYPTO-1** | Cryptographic Algorithms | Industry-standard primitives: Ed25519 (RFC 8032), X25519 (RFC 7748), ChaCha20-Poly1305 (RFC 8439), SHA-256. | `crates/remote-desk-core/src/identity.rs`, `pairing.rs` [VERIFIED] |
| **MASVS-NETWORK-1** | Network Communication | All media encrypted with SRTP; control and input encrypted with DTLS 1.3; authenticated signaling envelopes. | `crates/remote-desk-net/src/signaling.rs`, `peer.rs` [VERIFIED] |
| **MASVS-PLATFORM-1** | Platform Permissions | Explicit user permission dialogs on desktop portal; minimal required Android permissions (INTERNET only). | `android/app/src/main/AndroidManifest.xml` [VERIFIED] |

---

## 4. NIST SSDF (SP 800-218) Practices Register

| SSDF Practice | Description | Remote-Desk Engineering Practice |
|---|---|---|
| **PO.1** | Define Security Requirements | STRIDE threat model completed and mapped to mitigations prior to implementation. |
| **PS.1** | Protect Software Integrity | Git version control, signed commits, automated framing validation (`0x5244`). |
| **PW.1** | Design Software Architecture | Layered architecture separating platform-agnostic core from native host capture and client UI. |
| **PW.4** | Reuse Secure Software | Standard cryptographic libraries (`ed25519-dalek`, `x25519-dalek`, `sha2`, `hmac`). |
| **RV.1** | Review Software Architecture | Multi-agent review protocol, OpenSpec validation, and loopback verification. |

---

## 5. Milestone Phase 7, 8 & 9 Engineering Verification Register

| Phase & Domain | Requirement IDs | Implementation Control & Deliverable | Automated Verification Reference | Status |
|---|---|---|---|---|
| **Phase 7: Windows Host Engine** | `REQ-CAP-003`, `REQ-INP-001` | DXGI Desktop Duplication, Windows Graphics Capture, Win32 `SendInput`, 0..65535 coordinate normalization, fail-safe release. | `crates/remote-desk-windows/tests/windows_tests.rs` | **VERIFIED** |
| **Phase 8: Worldwide TURN & Host** | `REQ-NET-001`, `REQ-NET-002`, `REQ-NET-003` | ICE candidate priority (host/srflx/relay), TURN fallback, in-memory rendezvous broker, and unified host daemon (`remote-desk-host`). | `crates/remote-desk-host/tests/host_tests.rs`, `crates/remote-desk-signal/tests/signal_tests.rs` | **VERIFIED** |
| **Phase 9: Production Packaging** | `REQ-SEC-004`, ISO 12207 §6.4.11 | Linux Flatpak, Debian, RPM, Arch, AppImage, systemd services, Windows WiX MSI, Android ProGuard, `SECURITY.md`, Release Runbook. | `packaging/`, `SECURITY.md`, `docs/release-runbook.md`, `docs/deployment-guide.md` | **VERIFIED** |

