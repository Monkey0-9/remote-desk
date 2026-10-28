# Deployment & Administration Guide: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-DOC-DEP-012 |
| **Target Audience** | System Administrators, Self-Hosters, Power Users |
| **Status** | Approved Deployment Manual |

---

## 1. Local Network Deployment (Direct P2P, Zero Cloud)

Remote-Desk operates out-of-the-box over local area networks (LAN) without needing any internet connection, account registration, or central server:

1. **Host Setup (Linux)**:
   ```bash
   # Run host service directly
   remote-desk-host --display :0
   ```
2. **Controller Pairing (Android)**:
   - Open Remote-Desk on Android.
   - Tap `+ Add Computer`.
   - Select the discovered host or enter its local IP (e.g. `192.168.1.120`).
   - Match the 6-digit Short Authentication String (SAS) code displayed on both screens.
   - Tap `Approve` on the host desktop.

---

## 2. Worldwide Self-Hosting (Rendezvous & TURN Relay)

To connect across different networks and enterprise firewalls worldwide:

### Step 1: Deploy Signaling & TURN Server
On your public VPS / server (Ubuntu / Debian):

```bash
cd deploy/docker
docker compose up -d
```

### Step 2: Configure Controller & Host
In Remote-Desk Settings:
- **Rendezvous Server**: `wss://signal.yourdomain.com:4443`
- **STUN/TURN Server**: `turn:turn.yourdomain.com:3478`
- **TURN Username / Credential**: Entered securely in local settings.

---

## 3. Privacy & Telemetry Statement

- **Screen Recording**: Strictly DISABLED by default. Remote-Desk does not record or archive screen frames.
- **Data Minimization**: Signaling envelopes only transmit encrypted ephemeral public keys and ICE candidates.
- **Key Storage**: All device identity seeds are generated locally and stored using OS-protected hardware keychains.
