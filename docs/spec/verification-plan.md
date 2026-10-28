# Verification & Quality Assurance Plan: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-QA-007 |
| **Author / Agent** | Quality Assurance & Verification Engineering Agent |
| **Standards Compliance** | NIST SSDF SP 800-218, IEEE 730:2026, OWASP MASVS v2.0 |
| **Test Matrix** | Unit, Integration, Mock E2E, Fuzzing, Regression |
| **Status** | Approved Verification Plan |

---

## 1. Quality Assurance Strategy & Guiding Principles

Remote-Desk enforces the principle: **No unverified claims of completion.** A task or milestone cannot be marked complete without reproducible execution evidence and automated test verification.

```
       +-----------------------------------------------------------+
       |                     TESTING PYRAMID                       |
       +-----------------------------------------------------------+
       |               [ End-to-End System Tests ]                 |
       |             Loopback Linux-to-Android Stream              |
       |                                                           |
       |          [ Cross-Module Integration Tests ]               |
       |       Capture -> Encoder -> Net -> Decoder -> Input       |
       |                                                           |
       |         [ Security & Adversarial Fuzz Tests ]             |
       |    Replay attacks, Malformed NALs, Deserializer Fuzzing   |
       |                                                           |
       |            [ Automated Core Unit Tests ]                  |
       |   Crypto Handshake, Bincode Serialization, State Machine  |
       +-----------------------------------------------------------+
```

---

## 2. Standardized Task Acceptance Checklist

Every single implementation task submitted to the repository MUST satisfy this checklist before merging:

```markdown
### Task Acceptance Verification
- [ ] Requirement ID referenced (e.g. `REQ-CAP-001`, `REQ-INP-003`)
- [ ] Implementation matches approved design specification
- [ ] Error and timeout paths handled safely with typed errors
- [ ] Automated unit tests written and passing
- [ ] Integration tests pass where applicable
- [ ] Security impact reviewed against STRIDE threat model
- [ ] Resource usage verified (zero memory leaks, bounded queues)
- [ ] Documentation updated
- [ ] Code formatted (`cargo fmt`) and linted (`cargo clippy -- -D warnings`)
- [ ] Execution output evidence documented
```

---

## 3. Test Suites & Verification Matrix

### 3.1 Unit Testing (`remote-desk-proto`, `remote-desk-core`)
| Test ID | Test Target | Verification Description | Execution Command |
|---|---|---|---|
| `TEST-PROTO-001` | Message Serialization | Verifies round-trip Bincode/Binary serialization of `MouseMoveAbs`, `MouseButton`, `KeyboardKey`. | `cargo test -p remote-desk-proto --test codec` |
| `TEST-PROTO-002` | Framing Alignment | Validates magic bytes `0x5244` and rejection of corrupt packet headers. | `cargo test -p remote-desk-proto --test framing` |
| `TEST-CORE-001` | Ed25519 Identity | Asserts key generation, signature, and verification against known test vectors. | `cargo test -p remote-desk-core --test identity` |
| `TEST-CORE-002` | SAS Derivation | Asserts deterministic 6-digit Short Authentication String generation from ephemeral secrets. | `cargo test -p remote-desk-core --test sas` |
| `TEST-CORE-003` | Session State Machine | Verifies state progression: `Disconnected -> Handshake -> Active -> Closed`. | `cargo test -p remote-desk-core --test session` |

### 3.2 Linux Capture & Input Integration (`remote-desk-linux`)
| Test ID | Test Target | Verification Description | Execution Command |
|---|---|---|---|
| `TEST-CAP-001` | Portal Permission Failure | Validates that portal permission denial returns `CaptureError::PermissionDenied` gracefully. | `cargo test -p remote-desk-linux --test portal_rejection` |
| `TEST-CAP-002` | X11 Shm Capture | Validates X11 screen capture against headless Xvfb virtual display. | `cargo test -p remote-desk-linux --test x11_capture` |
| `TEST-INP-001` | Input Coordinate Normalization | Asserts mapping of float `(0.5, 0.5)` to exact display resolution `(960, 540)` on 1080p target. | `cargo test -p remote-desk-linux --test input_coords` |
| `TEST-INP-002` | Stuck-Key Revocation | Simulates abrupt connection loss while keys are held; verifies immediate synthetic release. | `cargo test -p remote-desk-linux --test key_revocation` |

### 3.3 Android Controller Verification (`android/app`)
| Test ID | Test Target | Verification Description | Execution Command |
|---|---|---|---|
| `TEST-ANDR-001` | Navigation Flow | Asserts correct navigation between Devices, Add Computer, and Session views. | `./gradlew testDebugUnitTest --tests "*NavigationTest*"` |
| `TEST-ANDR-002` | Touch Mode Switching | Verifies gesture interpreter toggling between Direct-Touch and Pointer mode. | `./gradlew testDebugUnitTest --tests "*GestureModeTest*"` |
| `TEST-ANDR-003` | MASVS Storage Security | Verifies keys are stored in `EncryptedSharedPreferences` backed by Android Keystore. | `./gradlew connectedAndroidTest --tests "*MasvsStorageTest*"` |

---

## 4. Security & Robustness Verification Plan

### 4.1 Fuzz Testing Harness (`cargo-fuzz` / `libfuzzer`)
- **Fuzz Target 1: `fuzz_protocol_deserializer`**:
  - Feeds arbitrary corrupted byte streams into the binary frame parser.
  - Assert: Must never panic, crash, or perform out-of-bounds memory accesses.
- **Fuzz Target 2: `fuzz_signaling_json`**:
  - Fuzzes JSON signaling payloads with malformed UTF-8, excessive nesting, and type confusion.

### 4.2 Adversarial Network Simulation
- **Packet Drop & Reordering**: Tested using Linux NetEm (`tc qdisc add dev lo root netem loss 5% delay 20ms reorder 10%`).
- **Assert**: WebRTC TWCC adapts bitrate gracefully; control channel SCTP preserves message ordering.

---

## 5. Release Quality Gates (Phase Milestone Verification)

Each milestone phase defined in the roadmap MUST satisfy its corresponding gate before proceeding:

| Phase Gate | Gate Criteria | Verification Evidence Required |
|---|---|---|
| **Gate 0: Specifications** | All 9 spec documents approved and cross-reconciled. | Document signatures; zero conflicting interface traits. |
| **Gate 1: Core Foundation** | Rust workspace builds clean; zero clippy warnings; 100% protocol unit tests pass. | `cargo test --workspace` logs in CI; `cargo clippy` clean. |
| **Gate 2: Linux Capture** | Linux capture pipeline acquires frames on Wayland and X11; permission rejection safe. | Automated headless Xvfb / portal mock test log. |
| **Gate 3: Android Viewer** | Android client compiles, connects to local mock stream, and presents video canvas. | Instrumented test video frame display verification. |
| **Gate 4: Remote Input** | Touch and mouse gestures map to remote host inputs without stuck keys on disconnect. | Automated stuck-key revocation test evidence. |
| **Gate 5: Security Review** | Threat model mitigations verified; zero unauthenticated input injection; MASVS-L1 pass. | Security test report + static analyzer scan output. |
| **Gate 6: Performance** | Latency < 40ms median on LAN; CPU < 0.5% idle; zero memory leaks. | Criterion benchmark run and Massif heap profiler logs. |

---

## 6. Standards Compliance Register

| Standard | Clause / Control | System Implementation | Audit / Verification Method |
|---|---|---|---|
| **NIST SSDF SP 800-218** | PW.1.1 (Use secure coding standards) | Rust memory safety; `clippy` pedantic rules; Kotlin type-safety. | CI static analysis gate |
| **NIST SSDF SP 800-218** | PS.1.1 (Maintain software component integrity) | `Cargo.lock` audited via `cargo-audit`; no unpinned dependencies. | `cargo audit` in CI pipeline |
| **OWASP MASVS v2.0** | MASVS-STORAGE-1 | Android Keystore used for Ed25519 private keys. | Unit tests on KeyStore provider |
| **OWASP MASVS v2.0** | MASVS-CRYPTO-1 | Modern ciphers only (Ed25519, X25519, ChaCha20-Poly1305). | Code inspection & unit test assertions |
| **IEEE 730:2026** | Clause 6.2 (Verification Traceability) | Requirement-to-Test mapping maintained in `verification-plan.md`. | Traceability audit matrix |
