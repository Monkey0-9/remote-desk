# Security Policy: Remote-Desk Platform

Remote-Desk enforces a privacy-first, zero-knowledge architecture. We welcome responsible disclosure of security vulnerabilities.

---

## 1. Supported Versions

| Version | Supported | Security Update Commitment |
|---|---|---|
| **1.0.x (Current)** | :white_check_mark: | Immediate security patches within 48h for High/Critical |
| **< 1.0.0 (Pre-release)** | :x: | Not supported; upgrade to latest release |

---

## 2. Core Security Guarantees

1. **Confidentiality**: End-to-end encryption across all video, audio, input, and clipboard channels via DTLS 1.3 and SRTP.
2. **Authentication**: Cryptographic mutual pairing via Ed25519 identity keys and 6-digit Short Authentication String (SAS) matching.
3. **No Ambient Data Exposure**: Screen recording and cloud session retention are disabled by default.
4. **Immediate Revocation**: Disconnection, timeout, or authorization revocation triggers immediate synthetic release of all hardware inputs (`release_all()`).

---

## 3. Reporting a Vulnerability

If you discover a security vulnerability or cryptographic flaw:
1. **DO NOT** open a public GitHub issue.
2. Email encrypted details to `security@remote-desk.org` or submit via GitHub Private Vulnerability Reporting.
3. Include:
   - Affected crate/component (`remote-desk-core`, `remote-desk-linux`, `remote-desk-windows`, `android/app`, etc.)
   - Reproducible proof-of-concept steps or adversarial payload.
   - Severity assessment against STRIDE threat model (`docs/spec/threat-model.md`).

### Response Timeline
- **Initial Acknowledgement**: Within 24 hours.
- **Triage & Patch Formulation**: Within 72 hours.
- **CVE Assignment & Advisory**: Coordinated public release upon patch availability.
