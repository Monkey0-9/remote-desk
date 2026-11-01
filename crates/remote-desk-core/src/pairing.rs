use crate::identity::DeviceIdentity;
use ed25519_dalek::VerifyingKey;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

type HmacSha256 = Hmac<Sha256>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PairingError {
    #[error("Weak or low-order Diffie-Hellman public key rejected")]
    WeakPublicKey,
    #[error("Invalid SAS code")]
    InvalidSas,
    #[error("Handshake transcript verification failed")]
    TranscriptVerificationFailed,
}

pub struct EphemeralKeyExchange {
    secret: StaticSecret,
    public: X25519PublicKey,
}

impl EphemeralKeyExchange {
    pub fn new() -> Self {
        let csprng = OsRng;
        let secret = StaticSecret::random_from_rng(csprng);
        let public = X25519PublicKey::from(&secret);
        Self { secret, public }
    }

    pub fn public_key(&self) -> X25519PublicKey {
        self.public
    }

    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.public.as_bytes()
    }

    /// Compute shared secret and validate against weak/low-order point attacks
    pub fn compute_shared_secret(
        self,
        peer_public_bytes: &[u8; 32],
    ) -> Result<[u8; 32], PairingError> {
        // Disallow all-zero point
        if *peer_public_bytes == [0u8; 32] {
            return Err(PairingError::WeakPublicKey);
        }

        let peer_public = X25519PublicKey::from(*peer_public_bytes);
        let shared = self.secret.diffie_hellman(&peer_public);
        let shared_bytes = *shared.as_bytes();

        // An all-zero shared secret is produced if the point is a small-order element
        if shared_bytes == [0u8; 32] {
            return Err(PairingError::WeakPublicKey);
        }

        Ok(shared_bytes)
    }
}

impl Default for EphemeralKeyExchange {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute 6-digit Short Authentication String (SAS) for visual verification
pub fn compute_sas_code(
    shared_secret: &[u8; 32],
    controller_pubkey: &[u8; 32],
    host_pubkey: &[u8; 32],
) -> String {
    let mut mac = HmacSha256::new_from_slice(shared_secret).expect("HMAC can take key of any size");

    mac.update(b"RemoteDesk-SAS-v1");
    // Lexicographical ordering to ensure both sides produce identical input regardless of role
    if controller_pubkey <= host_pubkey {
        mac.update(controller_pubkey);
        mac.update(host_pubkey);
    } else {
        mac.update(host_pubkey);
        mac.update(controller_pubkey);
    }

    let result = mac.finalize().into_bytes();
    let num = u32::from_be_bytes([result[0], result[1], result[2], result[3]]) % 1_000_000;
    format!("{:03}-{:03}", num / 1000, num % 1000)
}

/// Handshake Transcript Binding for Authenticated Key Exchange
///
/// Binds persistent identity keys, ephemeral Diffie-Hellman keys, session identifier,
/// protocol version, and negotiated capabilities into a single cryptographic transcript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HandshakeTranscript {
    pub protocol_version: u8,
    pub host_identity: [u8; 32],
    pub controller_identity: [u8; 32],
    pub host_ephemeral: [u8; 32],
    pub controller_ephemeral: [u8; 32],
    pub session_id: String,
    pub capabilities: Vec<String>,
}

impl HandshakeTranscript {
    pub fn new(
        protocol_version: u8,
        host_identity: [u8; 32],
        controller_identity: [u8; 32],
        host_ephemeral: [u8; 32],
        controller_ephemeral: [u8; 32],
        session_id: String,
        capabilities: Vec<String>,
    ) -> Self {
        Self {
            protocol_version,
            host_identity,
            controller_identity,
            host_ephemeral,
            controller_ephemeral,
            session_id,
            capabilities,
        }
    }

    /// Compute SHA-256 hash over the entire canonical transcript
    pub fn compute_transcript_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update([self.protocol_version]);
        hasher.update(self.host_identity);
        hasher.update(self.controller_identity);
        hasher.update(self.host_ephemeral);
        hasher.update(self.controller_ephemeral);
        hasher.update(self.session_id.as_bytes());
        for cap in &self.capabilities {
            hasher.update(cap.as_bytes());
        }
        let res = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&res);
        out
    }

    /// Sign transcript hash using local device persistent identity key
    pub fn sign(&self, identity: &DeviceIdentity) -> [u8; 64] {
        let hash = self.compute_transcript_hash();
        identity.sign(&hash)
    }

    /// Verify signature from remote device verifying key over the transcript hash
    pub fn verify(
        &self,
        verifying_key: &VerifyingKey,
        signature: &[u8; 64],
    ) -> Result<(), PairingError> {
        let hash = self.compute_transcript_hash();
        DeviceIdentity::verify(verifying_key, &hash, signature)
            .map_err(|_| PairingError::TranscriptVerificationFailed)
    }
}

/// Keys derived via HKDF (RFC 5869) from the authenticated ephemeral shared secret
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKeys {
    pub media_key: [u8; 32],
    pub control_mac_key: [u8; 32],
    pub input_key: [u8; 32],
}

/// Derive separate, purpose-specific symmetric keys from ephemeral shared secret and transcript
pub fn derive_session_keys(shared_secret: &[u8; 32], transcript_hash: &[u8; 32]) -> SessionKeys {
    // HKDF-Extract: PRK = HMAC-SHA256(salt, IKM)
    let salt = b"RemoteDesk-HKDF-Salt-v1";
    let mut prk_mac = HmacSha256::new_from_slice(salt).expect("HMAC can take any salt");
    prk_mac.update(shared_secret);
    let prk = prk_mac.finalize().into_bytes();

    // HKDF-Expand: OKM = HMAC-SHA256(PRK, transcript_hash || info || 0x01)
    let derive_sub_key = |info: &[u8]| -> [u8; 32] {
        let mut exp_mac = HmacSha256::new_from_slice(&prk).expect("HMAC valid key");
        exp_mac.update(transcript_hash);
        exp_mac.update(info);
        exp_mac.update(&[0x01]);
        let res = exp_mac.finalize().into_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(&res);
        out
    };

    SessionKeys {
        media_key: derive_sub_key(b"media-encryption"),
        control_mac_key: derive_sub_key(b"control-mac"),
        input_key: derive_sub_key(b"input-channel"),
    }
}
