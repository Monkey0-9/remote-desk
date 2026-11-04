use serde::{Deserialize, Serialize};

pub const PROTOCOL_MAGIC: [u8; 2] = [0x52, 0x44]; // "RD"
pub const PROTOCOL_VERSION: u8 = 1;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    MouseMoveAbs = 0x01,
    MouseMoveRel = 0x02,
    MouseButton = 0x03,
    MouseWheel = 0x04,
    KeyboardKey = 0x05,
    TouchGesture = 0x06,

    DisplayTopology = 0x10,
    SelectMonitor = 0x11,
    QualityConfig = 0x12,
    ClipboardData = 0x13,

    Ping = 0x20,
    Pong = 0x21,

    EncryptedFrame = 0x30,

    Disconnect = 0xFF,
}

impl TryFrom<u8> for MessageType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(MessageType::MouseMoveAbs),
            0x02 => Ok(MessageType::MouseMoveRel),
            0x03 => Ok(MessageType::MouseButton),
            0x04 => Ok(MessageType::MouseWheel),
            0x05 => Ok(MessageType::KeyboardKey),
            0x06 => Ok(MessageType::TouchGesture),
            0x10 => Ok(MessageType::DisplayTopology),
            0x11 => Ok(MessageType::SelectMonitor),
            0x12 => Ok(MessageType::QualityConfig),
            0x13 => Ok(MessageType::ClipboardData),
            0x20 => Ok(MessageType::Ping),
            0x21 => Ok(MessageType::Pong),
            0x30 => Ok(MessageType::EncryptedFrame),
            0xFF => Ok(MessageType::Disconnect),
            other => Err(other),
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MouseButtonCode {
    Left = 0,
    Right = 1,
    Middle = 2,
    Back = 3,
    Forward = 4,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyState {
    Released = 0,
    Pressed = 1,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseMoveAbsPayload {
    pub monitor_id: u32,
    pub x: f32, // Normalized 0.0 .. 1.0
    pub y: f32, // Normalized 0.0 .. 1.0
}

impl MouseMoveAbsPayload {
    /// Validates that coordinates are finite and bounded in 0.0..=1.0
    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
    }

    /// Clamps coordinates safely into 0.0..=1.0
    pub fn clamped(&self) -> Self {
        let x = if self.x.is_nan() {
            0.0
        } else {
            self.x.clamp(0.0, 1.0)
        };
        let y = if self.y.is_nan() {
            0.0
        } else {
            self.y.clamp(0.0, 1.0)
        };
        Self {
            monitor_id: self.monitor_id,
            x,
            y,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseMoveRelPayload {
    pub dx: i16,
    pub dy: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseButtonPayload {
    pub button: MouseButtonCode,
    pub state: KeyState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseWheelPayload {
    pub delta_x: i16,
    pub delta_y: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyboardKeyPayload {
    pub scancode: u32,
    pub state: KeyState,
    pub modifiers: u32, // Bitmask: Shift=1, Ctrl=2, Alt=4, Meta=8
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchGesturePayload {
    pub touch_id: u32,
    pub phase: u8, // 0=Down, 1=Move, 2=Up, 3=Cancel
    pub x: f32,    // Normalized 0.0 .. 1.0
    pub y: f32,    // Normalized 0.0 .. 1.0
    pub pressure: f32,
}

impl TouchGesturePayload {
    pub fn is_valid(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && (0.0..=1.0).contains(&self.x)
            && (0.0..=1.0).contains(&self.y)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardDataPayload {
    pub mime_type: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MonitorDescriptor {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub refresh_rate: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayTopologyPayload {
    pub monitors: Vec<MonitorDescriptor>,
    pub active_monitor_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectMonitorPayload {
    pub monitor_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityConfigPayload {
    pub max_fps: u32,
    pub max_bitrate_kbps: u32,
    pub preferred_codec: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PingPayload {
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PongPayload {
    pub client_timestamp_ms: u64,
    pub host_timestamp_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisconnectPayload {
    pub reason_code: u32,
    pub reason_text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RemoteDeskMessage {
    MouseMoveAbs(MouseMoveAbsPayload),
    MouseMoveRel(MouseMoveRelPayload),
    MouseButton(MouseButtonPayload),
    MouseWheel(MouseWheelPayload),
    KeyboardKey(KeyboardKeyPayload),
    TouchGesture(TouchGesturePayload),
    DisplayTopology(DisplayTopologyPayload),
    SelectMonitor(SelectMonitorPayload),
    QualityConfig(QualityConfigPayload),
    ClipboardData(ClipboardDataPayload),
    Ping(PingPayload),
    Pong(PongPayload),
    Disconnect(DisconnectPayload),
    EncryptedFrame(Vec<u8>),
}

impl RemoteDeskMessage {
    pub fn message_type(&self) -> MessageType {
        match self {
            RemoteDeskMessage::MouseMoveAbs(_) => MessageType::MouseMoveAbs,
            RemoteDeskMessage::MouseMoveRel(_) => MessageType::MouseMoveRel,
            RemoteDeskMessage::MouseButton(_) => MessageType::MouseButton,
            RemoteDeskMessage::MouseWheel(_) => MessageType::MouseWheel,
            RemoteDeskMessage::KeyboardKey(_) => MessageType::KeyboardKey,
            RemoteDeskMessage::TouchGesture(_) => MessageType::TouchGesture,
            RemoteDeskMessage::DisplayTopology(_) => MessageType::DisplayTopology,
            RemoteDeskMessage::SelectMonitor(_) => MessageType::SelectMonitor,
            RemoteDeskMessage::QualityConfig(_) => MessageType::QualityConfig,
            RemoteDeskMessage::ClipboardData(_) => MessageType::ClipboardData,
            RemoteDeskMessage::Ping(_) => MessageType::Ping,
            RemoteDeskMessage::Pong(_) => MessageType::Pong,
            RemoteDeskMessage::EncryptedFrame(_) => MessageType::EncryptedFrame,
            RemoteDeskMessage::Disconnect(_) => MessageType::Disconnect,
        }
    }
}
