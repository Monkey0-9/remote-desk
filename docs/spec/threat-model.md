# Security Threat Model & Cryptographic Specification: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-SEC-005 |
| **Author / Agent** | Security & Cryptography Engineering Agent |
| **Methodology** | STRIDE Threat Modeling, NIST SSDF SP 800-218, OWASP MASVS v2.0 |
| **Target Platforms** | Linux, Windows, Android |
| **Status** | Approved Security Baseline |

---

## 1. System Scope & Security Objectives

Remote-Desk enables remote visual monitoring and interactive hardware input injection across disparate network perimeters. Because remote desktop solutions possess powerful system privileges, security must be built into the architectural foundation rather than layered on as an afterthought.

### Core Security Objectives:
1. **End-to-End Cryptographic Confidentiality**: No intermediate node (signaling broker, TURN relay, ISP) can decrypt screen frames, keystrokes, clipboard data, or control messages.
2. **Strict Cryptographic Mutual Authentication**: Knowing a host's IP address or 9-digit Device ID grants ZERO access. Every controller must possess explicit, user-approved cryptographic authorization.
3. **Fail-Safe Authorization Revocation**: Termination of a session, network partition, or revocation immediately revokes all system control capabilities and releases all active hardware inputs.
4. **Zero Ambient Logging of Sensitive Artifacts**: Keystrokes, passwords, clipboard fragments, and unencrypted frame buffers are never written to disk, swap, or diagnostic telemetry.

---

## 2. Cryptographic Architecture & Key Lifecycle

```
[CONTROLLER IDENTITY]                                [HOST IDENTITY]
Ed25519 Keypair (Long-lived)                         Ed25519 Keypair (Long-lived)
      │                                                    │
      │ 1. Ephemeral X25519 Key Exchange                   │
      ▼                                                    ▼
(e_controller, e_controller_pub)                     (e_host, e_host_pub)
      │                                                    │
      └────────────── Noise_XX Handshake / DTLS ───────────┘
                               │
                               ▼
                    Shared Secret (K_shared)
                 = X25519(e_priv, e_remote_pub)
                               │
               ┌───────────────┴───────────────┐
               ▼                               ▼
       Session Encryption Keys             SAS Verification
       (ChaCha20-Poly1305 / DTLS)     HMAC-SHA256(K_shared, "SAS")
                                              │
                                              ▼
                                    6-Digit Matching Code
                                    (Displayed on both screens)
```

### 2.1 Cryptographic Primitives Register
- **Digital Signatures**: `Ed25519` (RFC 8032) for permanent device identity and signing authentication tokens.
- **Key Agreement**: `X25519` (RFC 7748) for ephemeral, forward-secret session key establishment.
- **Symmetric Encryption (AEAD)**: `ChaCha20-Poly1305` (RFC 8439) and `AES-256-GCM` for authenticated, tamper-evident payload encryption.
- **Key Derivation (KDF)**: `HKDF-SHA256` (RFC 5869) and `BLAKE3` for key expansion and domain separation.
- **Fingerprinting & Hashing**: `SHA-256` and `BLAKE3`.

### 2.2 Short Authentication String (SAS) Generation
To protect against active Man-in-the-Middle attacks during the initial unauthenticated signaling exchange:
$$\text{SAS}_{\text{raw}} = \text{HMAC-SHA256}(K_{\text{handshake}}, \text{ControllerPub} \mathbin{\Vert} \text{HostPub} \mathbin{\Vert} \text{"RemoteDesk-SAS-v1"})$$
$$\text{SAS}_{\text{numeric}} = (\text{SAS}_{\text{raw}}[0..4] \bmod 1{,}000{,}000)$$
Displayed to both users as a 6-digit formatted code (`XXX-XXX`) that must be visually verified prior to saving the device as trusted.

---

## 3. STRIDE Threat Analysis & Mitigations

The system boundaries are evaluated across all six STRIDE categories:

### 3.1 Spoofing (Identity Threats)
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-SPOOF-001` | Unauthorized Controller Impersonation | Attacker sends malicious input commands claiming to be a trusted controller. | **Critical** | Host rejects any control packet not verified by the established session AEAD cipher key negotiated via mutual Ed25519 authentication. |
| `THR-SPOOF-002` | Rogue Host Spoofing | Attacker positions a fake host answering the same Device ID. | **High** | Controller pins the host's Ed25519 public key. If the key changes, controller flags a prominent warning (`REQ-UI-003`). |

### 3.2 Tampering (Data Modification)
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-TAMP-001` | Modification of Input Stream by TURN Relay | Compromised TURN relay modifies mouse coordinates or keystrokes in flight. | **Critical** | SCTP DataChannels operate inside DTLS 1.3 encryption. TURN servers only see opaque UDP payloads; tampering causes AEAD tag verification failure and packet drop. |
| `THR-TAMP-002` | Replay of Recorded Input Packets | Attacker replays captured mouse clicks or password keystrokes. | **High** | Every control message contains a monotonically increasing 64-bit sequence number and session epoch. Stale or duplicated sequences are dropped immediately. |

### 3.3 Repudiation
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-REP-001` | Denying Initiated Input Actions | Malicious operator denies executing remote commands. | **Medium** | Host maintains a cryptographically signed local audit log of session start, session end, paired key ID, and connection endpoint IP. |

### 3.4 Information Disclosure (Privacy & Eavesdropping)
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-INFO-001` | Eavesdropping on Video Stream | Unencrypted interception of desktop screen contents on public Wi-Fi. | **Critical** | Video is encrypted with SRTP using ephemeral keys negotiated via DTLS 1.3 (`AEAD_AES_128_GCM` or `AEAD_CHACHA20_POLY1305`). |
| `THR-INFO-002` | Keystroke & Password Leakage in Logs | Passwords typed remotely recorded in debug trace logs. | **High** | Logging subsystem strictly excludes `keyboard_key` and `clipboard_data` payloads. Automated CI tests enforce zero occurrences of sensitive structs in logs (`REQ-SEC-004`). |
| `THR-INFO-003` | Screen Buffer Leakage via Crash Dumps | OS core dump or Android minidump reveals unencrypted screen memory. | **Medium** | Frame buffers use protected memory allocations zeroed on deallocation. Android sets `FLAG_SECURE` on sensitive surfaces when enabled. |

### 3.5 Denial of Service (Availability Threats)
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-DOS-001` | Signaling Flooding / Resource Exhaustion | Attacker floods host with connection requests to exhaust memory. | **High** | Host enforces token-bucket rate limiting on incoming signaling offers (max 5 per second per source IP) and immediately rejects offers without valid cryptographic timestamps. |
| `THR-DOS-002` | Malicious Codec Packet Crash | Sending malformed H.264 NAL units to trigger a decoder crash in controller. | **High** | Android hardware decoders run in isolated Android media sandbox. Fuzzing harnesses test decoder resilience against corrupted bitstreams. |

### 3.6 Elevation of Privilege
| Threat ID | Threat Description | Attack Vector | Severity | Mitigation Strategy |
|---|---|---|---|---|
| `THR-ELEV-001` | Unauthorized Desktop Control / Privilege Escalation | Malicious client attempts to execute code or bypass lock screen. | **Critical** | Host operates strictly in user-space context. Linux Wayland uses `xdg-desktop-portal` without setuid binaries. Input cannot be injected when the host session is locked unless explicit elevated agent mode is configured. |
| `THR-ELEV-002` | Stuck Input Key Exploitation | Attacker terminates connection while `Super` or `Ctrl` key is held down to leave host compromised. | **High** | Fail-safe release engine: on any session closure, the host OS immediately dispatches synthetic key-up and button-up events for all 256 scancodes (`REQ-INP-003`). |

---

## 4. OWASP MASVS v2.0 Android Security Checklist

The Android controller architecture is aligned with the following MASVS controls:

```
[MASVS-STORAGE]
  ├── Store private identity key in Android Keystore (Hardware TEE / StrongBox)
  ├── Encrypt device database with AES-256-GCM (EncryptedSharedPreferences)
  └── Wipe temporary video frame buffers from RAM after rendering

[MASVS-CRYPTO]
  ├── Prohibit DES, 3DES, RC4, MD5, SHA-1
  ├── Enforce forward secrecy via ephemeral X25519 per session
  └── Constant-time comparison for all HMAC and token checks

[MASVS-NETWORK]
  ├── TLS 1.3 mandatory for signaling connections
  ├── Strict hostname verification and optional certificate pinning
  └── WebRTC DTLS-SRTP end-to-end encryption for all media tracks

[MASVS-PLATFORM]
  ├── Set android:exported="false" on all activities except MainActivity
  ├── Filter obscured touches (prevent tapjacking on pairing confirmation)
  └── Implement FLAG_SECURE option to prevent Android OS screenshot/recording
```

---

## 5. Security Incident Response & Key Revocation Protocol

1. **Local Key Revocation**:
   - The user can click `"Revoke All Devices"` on either host or controller.
   - This purges the local trust database and rotates the node's primary Ed25519 identity key.
   - Any future connection attempts from previously paired controllers will be rejected with `Error::UntrustedPeer`.
2. **Immediate Disconnect Kill-Switch**:
   - The host desktop UI includes an omnipresent, un-dismissible floating indicator: `"Remote Controller Connected [Disconnect Now]"`.
   - Clicking disconnect immediately cancels the Tokio cancellation token, shuts down WebRTC peer connections, zeroes session keys in memory, and triggers `release_all()` on the input backend.
