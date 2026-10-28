use crate::messages::{MessageType, RemoteDeskMessage, PROTOCOL_MAGIC, PROTOCOL_VERSION};
use byteorder::{BigEndian, ByteOrder};
use thiserror::Error;

pub const HEADER_SIZE: usize = 16;
pub const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024; // 16 MB boundary limit

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ProtoError {
    #[error("Invalid protocol magic: expected {expected:?}, got {actual:?}")]
    InvalidMagic { expected: [u8; 2], actual: [u8; 2] },

    #[error("Unsupported protocol version: expected {expected}, got {actual}")]
    UnsupportedVersion { expected: u8, actual: u8 },

    #[error("Unknown message type identifier: {0}")]
    UnknownMessageType(u8),

    #[error("Buffer too short: expected at least {expected} bytes, got {actual}")]
    BufferTooShort { expected: usize, actual: usize },

    #[error(
        "Payload length mismatch: header specifies {header} bytes, actual buffer has {actual}"
    )]
    PayloadLengthMismatch { header: usize, actual: usize },

    #[error("Payload too large: {0} bytes exceeds safety limit")]
    PayloadTooLarge(usize),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub magic: [u8; 2],
    pub version: u8,
    pub msg_type: MessageType,
    pub sequence: u64,
    pub payload_len: u32,
}

impl FrameHeader {
    pub fn parse(buf: &[u8]) -> Result<Self, ProtoError> {
        if buf.len() < HEADER_SIZE {
            return Err(ProtoError::BufferTooShort {
                expected: HEADER_SIZE,
                actual: buf.len(),
            });
        }

        let magic = [buf[0], buf[1]];
        if magic != PROTOCOL_MAGIC {
            return Err(ProtoError::InvalidMagic {
                expected: PROTOCOL_MAGIC,
                actual: magic,
            });
        }

        let version = buf[2];
        if version != PROTOCOL_VERSION {
            return Err(ProtoError::UnsupportedVersion {
                expected: PROTOCOL_VERSION,
                actual: version,
            });
        }

        let raw_type = buf[3];
        let msg_type = MessageType::try_from(raw_type)
            .map_err(|_| ProtoError::UnknownMessageType(raw_type))?;

        let sequence = BigEndian::read_u64(&buf[4..12]);
        let payload_len = BigEndian::read_u32(&buf[12..16]);

        if (payload_len as usize) > MAX_PAYLOAD_SIZE {
            return Err(ProtoError::PayloadTooLarge(payload_len as usize));
        }

        Ok(FrameHeader {
            magic,
            version,
            msg_type,
            sequence,
            payload_len,
        })
    }

    pub fn write_to(&self, buf: &mut [u8]) {
        assert!(buf.len() >= HEADER_SIZE);
        buf[0] = self.magic[0];
        buf[1] = self.magic[1];
        buf[2] = self.version;
        buf[3] = self.msg_type as u8;
        BigEndian::write_u64(&mut buf[4..12], self.sequence);
        BigEndian::write_u32(&mut buf[12..16], self.payload_len);
    }
}

pub fn pack_frame(sequence: u64, msg: &RemoteDeskMessage) -> Result<Vec<u8>, ProtoError> {
    let payload =
        bincode::serialize(msg).map_err(|e| ProtoError::SerializationError(e.to_string()))?;

    if payload.len() > MAX_PAYLOAD_SIZE {
        return Err(ProtoError::PayloadTooLarge(payload.len()));
    }

    let header = FrameHeader {
        magic: PROTOCOL_MAGIC,
        version: PROTOCOL_VERSION,
        msg_type: msg.message_type(),
        sequence,
        payload_len: payload.len() as u32,
    };

    let mut out = vec![0u8; HEADER_SIZE + payload.len()];
    header.write_to(&mut out[..HEADER_SIZE]);
    out[HEADER_SIZE..].copy_from_slice(&payload);

    Ok(out)
}

pub fn unpack_frame(buf: &[u8]) -> Result<(u64, RemoteDeskMessage), ProtoError> {
    let header = FrameHeader::parse(buf)?;
    let total_len = HEADER_SIZE + header.payload_len as usize;

    if buf.len() < total_len {
        return Err(ProtoError::PayloadLengthMismatch {
            header: header.payload_len as usize,
            actual: buf.len() - HEADER_SIZE,
        });
    }

    let payload_slice = &buf[HEADER_SIZE..total_len];
    let msg: RemoteDeskMessage = bincode::deserialize(payload_slice)
        .map_err(|e| ProtoError::SerializationError(e.to_string()))?;

    // Verify consistency between header message type and deserialized enum variant
    if msg.message_type() != header.msg_type {
        return Err(ProtoError::SerializationError(format!(
            "Header message type {:?} does not match payload {:?}",
            header.msg_type,
            msg.message_type()
        )));
    }

    Ok((header.sequence, msg))
}
