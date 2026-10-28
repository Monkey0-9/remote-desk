# Release Runbook & Rollback Procedures: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-OPS-REL-011 |
| **Standards Compliance** | ISO/IEC/IEEE 12207:2026 Clause 6.4.11, IEEE 730:2026 Clause 5.2 |
| **Target Audience** | Release Engineers, DevOps Automators, Integration Owners |
| **Status** | Production Release Baseline |

---

## 1. Pre-Release Verification Gates

A release candidate (RC) cannot be promoted to production without passing all automated quality gates:

```
[ Gate 1: Workspace Tests ] ──> cargo test --workspace --all-targets (100% pass)
             │
             ▼
[ Gate 2: Code Quality ]    ──> cargo fmt --check && cargo clippy --workspace -- -D warnings
             │
             ▼
[ Gate 3: Spec Validation ] ──> openspec validate --all
             │
             ▼
[ Gate 4: Empirical Bench ] ──> cargo run -p remote-desk-bench (Median < 35ms, Idle CPU < 0.5%)
             │
             ▼
[ Gate 5: Security Review ] ──> OWASP MASVS KeyStore check, zero hardcoded secrets
             │
             ▼
[ PROCEED TO RELEASE ARTIFACT PACKAGING ]
```

---

## 2. Release Packaging Pipeline

1. **Linux Artifacts**:
   - Tarball containing `remote-desk-host`, `remote-desk-client`, systemd service, and desktop entry.
   - Flatpak & AppImage containerized bundles for Wayland/X11 distribution.
2. **Windows Artifacts**:
   - MSI installer containing DXGI capture service and Windows controller executable.
3. **Android Client**:
   - Signed APK & Android App Bundle (`.aab`) with ProGuard/R8 obfuscation.
4. **Signaling Server**:
   - Multi-arch Docker container (`linux/amd64`, `linux/arm64`) pushed to container registry.

---

## 3. Rollback & Disaster Recovery Procedures

### 3.1 Immediate Rollback Triggers
- Any critical authorization bypass or remote code execution vulnerability reported.
- Crash rate exceeding 0.1% of active remote sessions.
- Inability to release held keyboard/mouse inputs on session drop.

### 3.2 Rollback Execution Steps
1. **GitHub Releases**: Mark current release as deprecated; re-tag previous stable release as latest.
2. **Signaling Server**:
   ```bash
   # Revert to previous immutable Docker image digest
   docker compose -f deploy/docker/docker-compose.yml down
   docker tag remotedesk/signal:v1.0.0-stable remotedesk/signal:latest
   docker compose -f deploy/docker/docker-compose.yml up -d
   ```
3. **Android Play Store**: Issue immediate staged rollback or expedited hotfix build via Google Play Console.
4. **Client Notification**: Broadcast security advisory requiring clients to update before establishing new peer sessions.
