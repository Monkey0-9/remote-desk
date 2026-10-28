# User Interface Specification: Remote-Desk Controller & Host

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-UI-003 |
| **Author / Agent** | UI/UX & Interaction Design Agent |
| **Target Clients** | Android 8.0+ (Jetpack Compose / Material 3), Linux Desktop (GTK4 / Adwaita), Windows (WinUI3 / Native) |
| **Design Principles** | Familiarity, Maximum Screen Real Estate, Instant Mode Switching, Zero Clutter |
| **Status** | Approved Baseline Specification |

---

## 1. Android Information Architecture & Navigation

The Android mobile client is organized around four core functional surfaces:
1. **Devices** (Default Home Surface): Fast access to saved, paired computers and discovery of local network hosts.
2. **Add Computer** (Pairing Modal / Flow): Secure pairing, address/ID entry, QR scanning, and SAS verification.
3. **Active Session** (Immersive Full-Screen Canvas): Pure remote desktop viewport with floating edge-dockable toolbar.
4. **Settings & Privacy** (Configuration Surface): Streaming preferences, gesture mappings, trusted device revocation, and privacy logs.

```
                  +-----------------------------------+
                  |           MAIN ROUTER             |
                  +-----------------------------------+
                                    |
            +-----------------------+-----------------------+
            |                                               |
            v                                               v
  +--------------------+                         +--------------------+
  |   Devices Screen   |                         |  Settings & Privacy|
  +--------------------+                         +--------------------+
            |
    +-------+-------+
    |               |
    v               v
+-------+       +----------------------+
| Add   |       | Connection Progress  |
| Host  |       +----------------------+
+-------+                   |
                            v
                +----------------------+
                | Active Session Canvas|
                | (Floating Overlay)   |
                +----------------------+
```

---

## 2. Screen A: Devices (Main View)

### 2.1 Wireframe Layout
```
+-------------------------------------------------------------+
| Remote-Desk                                        [Search] |
|                                                    [Settings]|
+-------------------------------------------------------------+
| YOUR COMPUTERS                                              |
|                                                             |
| +---------------------------------------------------------+ |
| | [ICON] Desktop-Workstation                      [Online] | |
| |        Linux (Wayland) · 192.168.1.120                   | |
| |        Last connected: 2 hours ago                      | |
| |                                          [ CONNECT ]    | |
| +---------------------------------------------------------+ |
|                                                             |
| +---------------------------------------------------------+ |
| | [ICON] Studio-Laptop                            [Offline]| |
| |        Windows 11 · 10.0.0.45                           | |
| |        Last connected: Yesterday                         | |
| |                                          [ DETAILS ]    | |
| +---------------------------------------------------------+ |
|                                                             |
| DISCOVERED ON LOCAL NETWORK                                 |
|                                                             |
| +---------------------------------------------------------+ |
| | [ICON] Media-Server (New)                        [LAN]   | |
| |        Linux (X11) · 192.168.1.55                        | |
| |                                          [ + PAIR ]     | |
| +---------------------------------------------------------+ |
|                                                             |
|                           [ + Add Computer ]                |
+-------------------------------------------------------------+
| [Devices (active)]                            [Settings]    |
+-------------------------------------------------------------+
```

### 2.2 Functional Behavior & Edge Cases
1. **Device Identification**: Displays human-readable name, OS badge (Linux penguin, Windows logo), IP address or truncated Device ID (`rd-9481-2201`), and status badge.
2. **Status Determination**:
   - Status is verified via passive mDNS listener on LAN and low-overhead heartbeat ping when user opens the screen.
   - **No aggressive polling**: Devices are NOT continuously probed in the background when app is unfocused.
   - If a host cannot be contacted within 2.0s, status shows `Offline`. Never display a device as `Available` simply because its record is cached in local storage.
3. **Card Context Actions (Long-press / Swipe)**:
   - **Edit / Rename**: Modify alias or update connection endpoint.
   - **Manage Credentials**: View fingerprint, update stored password/PIN.
   - **Delete / Revoke**: Remove device from trusted list and erase local shared keys.

---

## 3. Screen B: Add Computer Workflow

### 3.1 Step-by-Step Flow
```
      [+ Add Computer]
             │
             ▼
   Select Discovery Mode
   ├── [1] Direct LAN / IP (Port 8443)
   ├── [2] Device ID (Signaling Locator)
   └── [3] Scan QR Code on Host Screen
             │
             ▼
   Enter Target Endpoint & Optional Port
   (Validation: Valid IPv4/IPv6, FQDN, or 9-digit Device ID)
             │
             ▼
   Initiate Cryptographic Handshake
             │
             ▼
   Display Short Authentication String (SAS)
   Controller Screen: "Verify matching code on host: 839-204"
             │
             ▼
   Host Approves Pairing Request
             │
             ▼
   Save Device into Encrypted Local Storage
```

### 3.2 Security Validation Rules
- **Device ID is a Locator, Not a Secret**: Knowing a 9-digit Device ID only allows routing signaling packets to the host. It DOES NOT authenticate the session.
- **Short Authentication String (SAS)**: Generated from the SHA-256 hash of the concatenated controller and host ephemeral public keys (`HMAC-SHA256(K_shared, "RemoteDesk-Pairing-SAS")`). Displayed as 6 digits or 4 phonetic words.
- Both parties must confirm the SAS match to defeat Man-in-the-Middle (MitM) attacks.

---

## 4. Screen C: Connection Progress & State Machine

### 4.1 Connection Lifecycle State Machine
```
[DISCONNECTED]
      │ Connect button tapped
      ▼
[FINDING_HOST] ───────────── Timeout (5s) ──────────────> [ERROR_HOST_UNREACHABLE]
      │ Host answers signaling
      ▼
[VERIFYING_IDENTITY] ────── Fingerprint mismatch ───────> [ERROR_KEY_CHANGED]
      │ Key matches trusted database
      ▼
[WAITING_HOST_APPROVAL] ─── User rejected on host ──────> [ERROR_PERMISSION_DENIED]
      │ Host approves session
      ▼
[ESTABLISHING_TRANSPORT] ── ICE negotiation fails ──────> [RETRY_TURN_FALLBACK]
      │ Direct P2P or TURN bound
      ▼
[NEGOTIATING_MEDIA] ─────── Codec negotiation failure ──> [ERROR_CODEC_UNSUPPORTED]
      │ Video stream decodable
      ▼
[CONNECTED] (Active Session View)
```

### 4.2 Actionable Error Messages
| Failure Condition | User-Facing Message | Actionable Button |
|---|---|---|
| Host unreachable on LAN | *"Could not reach Desktop at 192.168.1.120. Ensure the host application is running and on the same Wi-Fi network."* | `[Check LAN Settings]` `[Retry]` |
| Host rejection | *"Desktop rejected the remote connection request."* | `[Dismiss]` |
| Cryptographic identity change | *"WARNING: The host's cryptographic identity has changed. This could indicate a Man-in-the-Middle attack or a fresh re-install."* | `[View Fingerprints]` `[Abort]` |
| Timeout | *"Connection timed out while negotiating video transport."* | `[Try TURN Relay]` `[Cancel]` |

---

## 5. Screen D: Active Remote Session Canvas & Floating Pill

### 5.1 Screen Layout & Maximized Display
```
+-------------------------------------------------------------+
|                                                             |
|                                                             |
|                                                             |
|                                                             |
|                  LIVE REMOTE DESKTOP CANVAS                 |
|                   (Aspect ratio preserved)                  |
|                                                             |
|                                                             |
|                                                             |
|                                 +-------------------------+ |
|                                 | [Mouse] [Kbd] [Fit] [⋮] | |
|                                 +-------------------------+ |
+-------------------------------------------------------------+
```

### 5.2 Floating Toolbar Specifications
- **Positioning**: Default docked at bottom-right corner, 16dp above screen edge.
- **Drag Behavior**: Can be dragged and docked to any of the 4 screen perimeters (Left, Right, Top, Bottom).
- **Auto-Dimming**: After 3 seconds of user inactivity, the toolbar contracts into a small semi-transparent pill (40% alpha) to prevent screen burn-in and obstruction. Touching the pill restores full controls.
- **Controls**:
  1. **Input Mode Toggle**: Toggle between `[Mouse Pointer]` and `[Direct Touch]`.
  2. **Soft Keyboard**: Toggles Android soft keyboard + modifier ribbon (`Ctrl`, `Alt`, `Shift`, `Super/Win`, `Esc`, `Tab`, `F1-F12`, `Arrow Keys`).
  3. **Display Scaling**: `Fit to Screen` (letterboxed), `100% Native 1:1` (scrollable), or `Stretch`.
  4. **Monitor Switcher**: Displays monitor icons (`[1]`, `[2]`) when host has multiple physical displays attached.
  5. **Session Overflow (⋮)**:
     - Quality Preset (`Low Latency`, `Balanced`, `High Fidelity`)
     - Clipboard Sync (`Send Clipboard`, `Get Clipboard`)
     - Disconnect (`End Session Now`)

---

## 6. Touch Interaction Specification

### 6.1 Gesture Mapping Matrix
The Android controller provides two discrete input operational modes:

| User Physical Gesture | Direct-Touch Mode Action | Mouse-Pointer Mode Action |
|---|---|---|
| **Single-finger tap** | Left-click at touched coordinate | Left-click at visible cursor location |
| **Single-finger double tap** | Double left-click at touched coordinate | Double left-click at visible cursor |
| **Tap and hold (> 400ms)** | Right-click at touched coordinate | Drag-and-drop initiation (Left mouse down & hold) |
| **Single-finger drag** | Emulates touch-drag / selection | Moves visible cursor relatively (`dx * sensitivity, dy * sensitivity`) |
| **Two-finger tap** | Right-click at touched location | Right-click at visible cursor location |
| **Two-finger drag (vertical)** | Remote page / window vertical scroll | Emulates mouse wheel vertical scroll (`dy`) |
| **Two-finger drag (horizontal)**| Remote horizontal scroll | Emulates mouse wheel horizontal scroll (`dx`) |
| **Pinch-to-zoom (two fingers)**| Zooms local canvas viewport | Zooms local canvas viewport |
| **Three-finger swipe down** | Minimize remote window / Show Desktop | Toggles floating toolbar visibility |
| **Three-finger tap** | Shows soft keyboard | Shows soft keyboard |

### 6.2 External Keyboard & Mouse Support
- **Bluetooth / USB Mouse**:
  - When an external mouse is attached to the Android controller, Pointer Mode automatically captures the hardware mouse via Android Pointer Capture API (`view.requestPointerCapture()`).
  - Native Right-Click, Middle-Click, and Scroll Wheel bypass touch translation and map 1:1 to remote mouse events.
- **Hardware Keyboard**:
  - Raw `KeyEvent` codes captured and mapped via standard USB HID scancodes to remote X11/Linux keysyms and Windows Virtual Key (VK) codes.

---

## 7. Screen E: Settings & Privacy View

### 7.1 Configuration Structure
```
+-------------------------------------------------------------+
| Settings & Privacy                                 [Done]   |
+-------------------------------------------------------------+
| CONNECTION & PERFORMANCE                                    |
|   Display Quality Preset            [Balanced (1080p60) >]  |
|   Preferred Video Codec             [AV1 with H.264 Fallback]|
|   Max Frame Rate                    [60 FPS >]             |
|   Direct P2P LAN Only Mode          [ [Toggle: OFF] ]      |
|                                                             |
| INPUT & CONTROLS                                            |
|   Default Input Mode                [Mouse-Pointer Mode >]  |
|   Cursor Tracking Sensitivity       [====●========] 1.2x    |
|   Invert Mouse Wheel Direction      [ [Toggle: OFF] ]      |
|                                                             |
| PRIVACY & SECURITY                                          |
|   Biometric App Lock                [ [Toggle: ON] ]       |
|   Clipboard Auto-Synchronization    [ [Toggle: OFF] ]      |
|   Block Remote Screen Recording     [ [Toggle: ON] ]       |
|   Manage Paired Trusted Devices     [ 3 Devices > ]         |
|   Revoke All Trusted Keys           [ REVOKE ALL ]         |
|                                                             |
| DIAGNOSTICS & SYSTEM INFO                                   |
|   Real-Time Overlay (FPS / Latency) [ [Toggle: ON] ]       |
|   Export Redacted Diagnostic Logs   [ EXPORT ]             |
|   Version 0.1.0-alpha (Core: 0.1.0)                         |
+-------------------------------------------------------------+
```

### 7.2 Diagnostic Privacy Redaction Rule
Exported logs MUST filter and redact:
- IP addresses outside private RFC 1918 blocks.
- Hostnames and user account names.
- Public/private cryptographic key strings.
- Clipboard text fragments.
- Keystroke scancodes and input coordinates.

---

## 8. Desktop UI Specification (Linux & Windows Host/Controller)

### 8.1 Desktop UI Layout
```
+-------------------------------------------------------------------------+
| Remote-Desk                                                [-] [口] [X] |
+-------------------------------------------------------------------------+
| [Connections] [My Shared Desktop] [Security & Logs] [Settings]          |
+-------------------------------------------------------------------------+
| MY DESKTOP (HOST STATUS: READY)                                         |
|                                                                         |
|  Device ID: 492-810-394              [ Copy ]                           |
|  Status: Listening on 192.168.1.120:8443                                |
|  Sharing Screen: eDP-1 (1920x1080 @ 60Hz)                               |
|                                                                         |
|  Active Controller: None                                                |
|                                                                         |
|  [ Stop Host Service ]      [ Change Screen ]     [ Security Settings ] |
|                                                                         |
+-------------------------------------------------------------------------+
| SAVED REMOTE COMPUTERS                                                  |
|                                                                         |
|  +----------------------------+   +----------------------------+        |
|  | [PC] Office Workstation    |   | [PC] Cloud Linux Server    |        |
|  | Linux (Wayland) · Online   |   | Linux (Headless) · Offline |        |
|  | [ Connect ]  [ Configure ] |   | [ Retry ]    [ Configure ] |        |
|  +----------------------------+   +----------------------------+        |
|                                                                         |
|  [ + Connect to Remote Computer ]                                       |
+-------------------------------------------------------------------------+
```

### 8.2 Host Active Notification Bar
When a remote controller is actively connected, the host OS MUST display an un-dismissible notification and persistent status bar indicator:
- **Visual Alert**: Bright green or amber indicator pill at the top-center of the host monitor: `"Remote controller 'Pixel 8' connected. [Disconnect Immediately]"`.
- **Session Revocation**: Clicking the banner immediately severs the WebRTC transport and zeroes active virtual input devices.

---

## 9. Design System: Colors, Typography & Accessibility

### 9.1 Color Tokens (Dark & Light Theme Support)
| Token Name | Light Value | Dark Value | Purpose |
|---|---|---|---|
| `color-primary` | `#0D6EFD` | `#3B82F6` | Primary CTAs, active badges |
| `color-surface` | `#FFFFFF` | `#121212` | Screen background |
| `color-surface-container` | `#F3F4F6` | `#1E1E1E` | Device cards, modal backgrounds |
| `color-text-primary` | `#111827` | `#F9FAFB` | Primary headings, computer titles |
| `color-text-secondary` | `#6B7280` | `#9CA3AF` | Subtitles, last connected time |
| `color-success` | `#10B981` | `#34D399` | Online status, paired confirmation |
| `color-danger` | `#EF4444` | `#F87171` | Offline status, disconnect, revoke |

### 9.2 Accessibility & WCAG 2.1 AA Compliance
- **Contrast**: All text elements maintain minimum 4.5:1 contrast ratio against container surfaces.
- **Touch Target Sizing**: All interactive buttons, tabs, and floating pill icons have a minimum bounding box of 48x48 dp.
- **Screen Reader Support**: All icons and canvas controls include explicit `contentDescription` / accessibility labels.
