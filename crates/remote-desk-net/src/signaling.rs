use ed25519_dalek::{Signature, VerifyingKey};
use remote_desk_core::identity::DeviceIdentity;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum SignalingError {
    #[error("Serialization failed: {0}")]
    SerializationError(String),
    #[error("Invalid signature on signaling envelope")]
    InvalidSignature,
    #[error("Public key parsing failed: {0}")]
    InvalidPublicKey(String),
    #[error("Message timestamp expired or out of bounds (drift > 60s)")]
    TimestampExpired,
    #[error("Transport error: {0}")]
    TransportError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignalingEnvelope<T> {
    pub protocol_version: String,
    pub sender_device_id: String,
    pub sender_pubkey: String,
    pub timestamp_ms: u64,
    pub payload: T,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum SignalingPayload {
    HandshakeInit {
        client_name: String,
        ephemeral_x25519_pub: String,
        requested_capabilities: Vec<String>,
    },
    AuthChallenge {
        challenge_nonce: String,
        host_ephemeral_x25519_pub: String,
    },
    AuthResponse {
        challenge_response: String,
        sas_confirmation: String,
    },
    SdpOffer {
        sdp: String,
    },
    SdpAnswer {
        sdp: String,
    },
    IceCandidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<u16>,
    },
    SessionReject {
        reason: String,
    },
    SessionClose {
        reason: String,
    },
}

impl<T: Serialize> SignalingEnvelope<T> {
    pub fn sign_and_create(
        identity: &DeviceIdentity,
        payload: T,
        now_ms: u64,
    ) -> Result<Self, SignalingError> {
        let serialized_payload = serde_json::to_vec(&payload)
            .map_err(|e| SignalingError::SerializationError(e.to_string()))?;

        let mut signed_data = Vec::with_capacity(32 + 8 + serialized_payload.len());
        signed_data.extend_from_slice(identity.public_key_hex().as_bytes());
        signed_data.extend_from_slice(&now_ms.to_be_bytes());
        signed_data.extend_from_slice(&serialized_payload);

        let signature_bytes = identity.sign(&signed_data);
        let signature_hex = hex::encode(&signature_bytes);

        Ok(Self {
            protocol_version: "1.0.0".to_string(),
            sender_device_id: identity.device_id().to_string(),
            sender_pubkey: identity.public_key_hex(),
            timestamp_ms: now_ms,
            payload,
            signature: signature_hex,
        })
    }
}

impl<T: Serialize> SignalingEnvelope<T> {
    pub fn verify_signature(&self, current_time_ms: u64) -> Result<(), SignalingError> {
        // Enforce maximum clock drift of 60 seconds (60,000 ms)
        let drift = current_time_ms.abs_diff(self.timestamp_ms);

        if drift > 60_000 {
            return Err(SignalingError::TimestampExpired);
        }

        let pubkey_bytes = hex::decode(&self.sender_pubkey)
            .map_err(|_| SignalingError::InvalidPublicKey("Invalid hex in public key".into()))?;
        if pubkey_bytes.len() != 32 {
            return Err(SignalingError::InvalidPublicKey("Expected 32 bytes".into()));
        }

        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&pubkey_bytes);
        let verifying_key = VerifyingKey::from_bytes(&key_array)
            .map_err(|e| SignalingError::InvalidPublicKey(e.to_string()))?;

        let sig_bytes =
            hex::decode(&self.signature).map_err(|_| SignalingError::InvalidSignature)?;
        if sig_bytes.len() != 64 {
            return Err(SignalingError::InvalidSignature);
        }
        let mut sig_array = [0u8; 64];
        sig_array.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_array);

        let serialized_payload = serde_json::to_vec(&self.payload)
            .map_err(|e| SignalingError::SerializationError(e.to_string()))?;

        let mut signed_data = Vec::with_capacity(32 + 8 + serialized_payload.len());
        signed_data.extend_from_slice(self.sender_pubkey.as_bytes());
        signed_data.extend_from_slice(&self.timestamp_ms.to_be_bytes());
        signed_data.extend_from_slice(&serialized_payload);

        verifying_key
            .verify_strict(&signed_data, &signature)
            .map_err(|_| SignalingError::InvalidSignature)
    }
}

// Minimal hex encoder/decoder to avoid heavy external dependency
mod hex {
    pub fn encode(data: &[u8]) -> String {
        let mut s = String::with_capacity(data.len() * 2);
        for &b in data {
            use std::fmt::Write;
            write!(&mut s, "{:02x}", b).unwrap();
        }
        s
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, ()> {
        if !s.len().is_multiple_of(2) {
            return Err(());
        }
        let mut res = Vec::with_capacity(s.len() / 2);
        for i in (0..s.len()).step_by(2) {
            let byte = u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ())?;
            res.push(byte);
        }
        Ok(res)
    }
}
