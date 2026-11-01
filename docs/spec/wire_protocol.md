# Remote-Desk Normative Wire Protocol Specification

Version: 1.0  
Status: Normative  
Date: November 1, 2026  

## 1. Frame Structure

Every protocol packet transmitted across data channels or network sockets begins with a strict 16-byte header in network byte order (Big-Endian), followed by the binary payload.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Magic ('RD')         |    Version    |  Message Type |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                    Sequence Number (u64)                      +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                      Payload Length (u32)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         Payload Bytes...                      |
+                                                               +
```

### 1.1 Header Fields

| Field | Offset | Type | Byte Order | Description |
|---|---|---|---|---|
| `magic` | 0..2 | `[u8; 2]` | N/A | Must equal `0x52, 0x44` (ASCII `"RD"`). Packets with invalid magic MUST be dropped. |
| `version` | 2 | `u8` | N/A | Protocol version. Current version is `0x01`. Unsupported versions MUST be rejected. |
| `msg_type` | 3 | `u8` | N/A | Message type discriminant (see Table 1.2). |
| `sequence` | 4..12 | `u64` | Big-Endian | Strictly monotonically increasing sequence counter for replay defense. |
| `payload_len` | 12..16 | `u32` | Big-Endian | Exact byte length of the payload following this header. Boundary max: 16 MB. |

### 1.2 Message Type Discriminants

| Type ID | Message Name | Delivery Guarantee | Description |
|---|---|---|---|
| `0x01` | `MouseMoveAbs` | Unreliable / Coalesced | Normalized absolute pointer coordinates (0.0 .. 1.0). |
| `0x02` | `MouseMoveRel` | Unreliable / Coalesced | Relative pointer motion deltas (dx, dy). |
| `0x03` | `MouseButton` | Reliable / In-Order | Mouse button press or release state transition. |
| `0x04` | `MouseWheel` | Reliable / In-Order | Mouse wheel scroll delta steps (horizontal & vertical). |
| `0x05` | `KeyboardKey` | Reliable / In-Order | Keyboard scancode press/release with modifier bitmask. |
| `0x06` | `TouchGesture` | Unreliable / Coalesced | Multi-touch contact points, phase, coordinates, pressure. |
| `0x10` | `DisplayTopology` | Reliable / In-Order | Host display outputs, resolutions, and active monitor ID. |
| `0x11` | `SelectMonitor` | Reliable / In-Order | Request host switch capture to designated monitor ID. |
| `0x12` | `QualityConfig` | Reliable / In-Order | Dynamic FPS, bitrate, and codec negotiation. |
| `0x13` | `ClipboardData` | Reliable / In-Order | Text or image MIME data payload for remote clipboard. |
| `0x20` | `Ping` | Unreliable | Keepalive heartbeat with sender millisecond timestamp. |
| `0x21` | `Pong` | Unreliable | Heartbeat response echoing client timestamp and host timestamp. |
| `0xFF` | `Disconnect` | Reliable / In-Order | Explicit session termination request with reason code. |

---

## 2. Payload Binary Layouts

All payload fields are encoded in **Little-Endian** byte order.

### 2.1 MouseMoveAbs (`0x01`)
Total payload size: **16 bytes**
- `0..4`: Variant Discriminant: `0x00000000` (`u32` LE)
- `4..8`: Monitor ID: `u32` LE
- `8..12`: Normalized X: `f32` LE (must be finite, 0.0 <= x <= 1.0)
- `12..16`: Normalized Y: `f32` LE (must be finite, 0.0 <= y <= 1.0)

### 2.2 MouseButton (`0x03`)
Total payload size: **12 bytes**
- `0..4`: Variant Discriminant: `0x00000002` (`u32` LE)
- `4..8`: Button Code: `u32` LE (0=Left, 1=Right, 2=Middle, 3=Back, 4=Forward)
- `8..12`: State: `u32` LE (0=Released, 1=Pressed)

### 2.3 MouseWheel (`0x04`)
Total payload size: **8 bytes**
- `0..4`: Variant Discriminant: `0x00000003` (`u32` LE)
- `4..6`: Delta X: `i16` LE
- `6..8`: Delta Y: `i16` LE

### 2.4 KeyboardKey (`0x05`)
Total payload size: **16 bytes**
- `0..4`: Variant Discriminant: `0x00000004` (`u32` LE)
- `4..8`: Scancode: `u32` LE (Linux evdev / Win32 hardware scancode)
- `8..12`: State: `u32` LE (0=Released, 1=Pressed)
- `12..16`: Modifiers: `u32` LE (Bitmask: Shift=1, Ctrl=2, Alt=4, Meta=8)

---

## 3. Strict Decoder Bounds & Invariants

1. **Exact Framing**: `buf.len() == HEADER_SIZE + payload_len`. Extra trailing bytes MUST be rejected with `PayloadLengthMismatch`.
2. **Coordinate Sanitization**: Coordinate floating point values MUST NOT be NaN or Infinity. Out-of-range values MUST be rejected.
3. **Replay Defense**: Every input message sequence counter MUST be checked against a sliding replay window of size >= 64. Duplicate or backwards sequence numbers MUST be discarded.
4. **Fail-Safe Revocation**: On reception of `Disconnect (0xFF)` or transport disruption, the host MUST immediately invoke `release_all()` to clear all held keys and mouse buttons within 10ms.
