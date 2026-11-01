# Wire Protocol & Message Specification: Remote-Desk Platform

| Document Metadata | Details |
|---|---|
| **Document ID** | RD-SPEC-PROTO-006 |
| **Author / Agent** | Networking & Transport Protocol Agent |
| **Protocol Version** | 1.0.0 (Magic Bytes: `0x52, 0x44` ["RD"], Version: `0x01`) |
| **Serialization** | JSON/CBOR (Signaling), Bincode/Binary (Control DataChannel) |
| **Status** | Approved Protocol Specification |

---

## 1. Protocol Architecture & Channel Separation

Remote-Desk protocol traffic is segregated into three independent network layers:

1. **Signaling Layer (Out-of-Band)**:
   - Exchanges session initialization, cryptographic authorization challenges, SDP offers/answers, and ICE candidates.
   - Operates over mDNS + local HTTP (LAN) or secure WebSocket / HTTPS rendezvous servers (WAN).
2. **Media Transport Layer (In-Band WebRTC MediaTracks)**:
   - High-throughput, real-time video streaming (H.264 / AV1) protected by SRTP (DTLS 1.3).
3. **Control & Input Layer (In-Band WebRTC DataChannels)**:
   - Bi-directional, multiplexed binary channels over SCTP:
     - `remote-desk-control`: Ordered, reliable control channel (topology, quality, clipboard, pings).
     - `remote-desk-input`: Low-latency, unordered/unreliable or time-bounded channel for high-frequency pointer movements.

```
+-------------------------------------------------------------------------+
|                            SIGNALING CHANNEL                            |
|             (JSON/CBOR over Local HTTP/mDNS or WebSocket Rendezvous)    |
|   - HandshakeInit  - AuthChallenge  - SdpOffer  - SdpAnswer  - IceCand  |
+-------------------------------------------------------------------------+
                                    │
                                    ▼ (ICE Traversal -> WebRTC Connected)
+-------------------------------------------------------------------------+
|                        WEBRTC PEER CONNECTION                           |
|  +-----------------------------------+  +----------------------------+  |
|  | SRTP Encrypted Video Track        |  | SCTP DataChannels          |  |
|  | - H.264 / AV1 NAL Units           |  | - remote-desk-control      |  |
|  | - TWCC Feedback / RTCP            |  | - remote-desk-input        |  |
|  +-----------------------------------+  +----------------------------+  |
+-------------------------------------------------------------------------+
```

---

## 2. Signaling Message Schema

Signaling messages are formatted as typed JSON or CBOR objects containing a common envelope:

```typescript
interface SignalingEnvelope<T> {
  protocol_version: "1.0.0";
  sender_device_id: string;      // e.g. "rd-4928-1029"
  sender_pubkey: string;         // Base64-encoded Ed25519 public key
  timestamp_ms: number;          // Monotonic UTC timestamp
  payload: T;
  signature: string;             // Ed25519 signature over serialized payload
}
```

### 2.1 Signaling Payloads

#### `HandshakeInit`
Controller initiates connection to host:
```json
{
  "type": "HandshakeInit",
  "client_name": "Pixel 8 Pro",
  "ephemeral_x25519_pub": "dGhpcyBpcyBhIHZhbGlkIHg...",
  "requested_capabilities": ["video:h264", "video:av1", "input:pointer", "input:keyboard"]
}
```

#### `AuthChallenge` & `AuthResponse`
Host issues cryptographic challenge to verify controller holds the private key of its registered identity:
```json
{
  "type": "AuthChallenge",
  "challenge_nonce": "a8f3b194c92019e4812a...",
  "host_ephemeral_x25519_pub": "eW91ciBwdWJrZXkgaGVy..."
}
```
Controller signs `challenge_nonce` with its Ed25519 key:
```json
{
  "type": "AuthResponse",
  "challenge_response": "c2lnbmF0dXJlX29mX25vbmNl...",
  "sas_confirmation": "839204"
}
```

#### `SdpOffer` & `SdpAnswer`
```json
{
  "type": "SdpOffer",
  "sdp": "v=0\r\no=- 4820194 2 IN IP4 127.0.0.1...\r\n"
}
```

#### `IceCandidate`
```json
{
  "type": "IceCandidate",
  "candidate": "candidate:842163049 1 udp 1677729535 192.168.1.120 54128 typ host...",
  "sdp_mid": "0",
  "sdp_mline_index": 0
}
```

---

## 3. Control & Input Binary Framing

Messages over `remote-desk-control` and `remote-desk-input` use compact, memory-aligned binary framing with zero variable-length allocations wherever possible:

### Binary Frame Header:
```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic (0x52, 0x44)   |    Version    |  Message Type |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Sequence Number                       |
|                            (64-bit)                           |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Length                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                        Payload Data...                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```
- `Magic`: `0x5244` (`"RD"`)
- `Version`: `0x01`
- `Message Type`: 8-bit identifier
- `Sequence Number`: Monotonically increasing counter per session
- `Payload Length`: 32-bit unsigned integer

---

## 4. Message Type Definitions

| ID | Message Name | Channel | Description |
|---|---|---|---|
| `0x01` | `MouseMoveAbs` | `input` | Absolute pointer move (`x: f32, y: f32` normalized 0.0..1.0) |
| `0x02` | `MouseMoveRel` | `input` | Relative pointer delta (`dx: i16, dy: i16`) |
| `0x03` | `MouseButton` | `input` | Mouse button press/release (`button: u8, state: u8`) |
| `0x04` | `MouseWheel` | `input` | Scroll delta (`delta_x: i16, delta_y: i16`) |
| `0x05` | `KeyboardKey` | `input` | Key down/up (`scancode: u32, key_state: u8`) |
| `0x06` | `TouchGesture` | `input` | Multi-touch gesture payload |
| `0x10` | `DisplayTopology`| `control` | Host announces list of connected monitors |
| `0x11` | `SelectMonitor` | `control` | Controller requests switching active monitor |
| `0x12` | `QualityConfig` | `control` | Controller requests bitrate / FPS / resolution limit |
| `0x13` | `ClipboardData` | `control` | UTF-8 clipboard synchronization payload |
| `0x20` | `Ping` | `control` | Round-trip latency probe (`client_timestamp: u64`) |
| `0x21` | `Pong` | `control` | Probe response (`client_timestamp: u64, host_timestamp: u64`) |
| `0xFF` | `Disconnect` | `control` | Graceful session termination with reason code |

### 4.1 Rust Struct Layouts for Input Serialization

```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonCode {
    Left = 0,
    Right = 1,
    Middle = 2,
    Back = 3,
    Forward = 4,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Released = 0,
    Pressed = 1,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseMoveAbsPayload {
    pub monitor_id: u32,
    pub x: f32, // 0.0 .. 1.0
    pub y: f32, // 0.0 .. 1.0
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseButtonPayload {
    pub button: MouseButtonCode,
    pub state: KeyState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KeyboardKeyPayload {
    pub scancode: u32,
    pub state: KeyState,
    pub modifiers: u32, // Bitmask: Ctrl, Alt, Shift, Meta
}
```

---

## 5. Session State Transitions & Error Codes

```
                    +--------------------+
                    |    DISCONNECTED    |
                    +--------------------+
                              │ HandshakeInit
                              ▼
                    +--------------------+
                    |   AUTHENTICATING   |
                    +--------------------+
                              │ AuthChallenge / Response Success
                              ▼
                    +--------------------+
                    |    ICE_CONNECTING  |
                    +--------------------+
                              │ PeerConnection Connected
                              ▼
                    +--------------------+
                    |   SESSION_ACTIVE   |
                    +--------------------+
                      │                │
     Disconnect Packet│                │ Network Timeout (5s)
                      ▼                ▼
             +----------------+  +-------------------+
             | CLOSED_GRACEFUL|  | RECONNECTING_ICE  |
             +----------------+  +-------------------+
```

### 5.1 Protocol Error Codes
| Code | Error Identifier | Description | Actionable Controller Handling |
|---|---|---|---|
| `0x01` | `ERR_UNSUPPORTED_VERSION` | Protocol version incompatible | Prompt user to update app |
| `0x02` | `ERR_UNAUTHORIZED_PEER` | Public key not in host's trusted ledger | Trigger Add Computer pairing flow |
| `0x03` | `ERR_HOST_REJECTED` | User on host desktop clicked "Reject" | Display rejection banner; stop |
| `0x04` | `ERR_SESSION_OCCUPIED` | Another controller is actively controlling host | Inform user host is busy |
| `0x05` | `ERR_CAPTURE_FAILED` | Screen capture backend failed to acquire stream | Prompt host configuration check |
| `0x06` | `ERR_TIMEOUT` | No heartbeat received for > 5000ms | Trigger ICE restart / TURN fallback |
