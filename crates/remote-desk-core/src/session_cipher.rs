use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;

use crate::pairing::{derive_session_keys, SessionKeys};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum CryptoError {
    #[error("Ciphertext authentication or MAC tag verification failed (possible tampering or forged packet)")]
    AuthenticationFailed,
    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),
    #[error(
        "Replay attack or duplicate sequence number detected: expected > {expected}, got {actual}"
    )]
    ReplayDetected { expected: u64, actual: u64 },
    #[error("Invalid packet format: {0}")]
    InvalidPacket(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherRole {
    Host,
    Client,
}

/// Direction-specific nonce prefixes to guarantee unique nonces even if sequence numbers coincide
pub const HOST_TO_CLIENT_TAG: [u8; 4] = [0x48, 0x32, 0x43, 0x00]; // "H2C\0"
pub const CLIENT_TO_HOST_TAG: [u8; 4] = [0x43, 0x32, 0x48, 0x00]; // "C2H\0"

/// Authenticated symmetric encryption context for Remote-Desk sessions.
///
/// Implements RFC 8439 ChaCha20-Poly1305 AEAD with session keys derived via HKDF-SHA256
/// from the mutual X25519 Diffie-Hellman shared secret and handshake transcript hash.
pub struct SessionCrypto {
    encrypt_cipher: ChaCha20Poly1305,
    decrypt_cipher: ChaCha20Poly1305,
    encrypt_tag: [u8; 4],
    decrypt_tag: [u8; 4],
    send_seq: AtomicU64,
    recv_seq: Arc<AtomicU64>,
    role: CipherRole,
}

impl SessionCrypto {
    /// Construct authenticated session cipher context from shared secret, transcript, and role
    pub fn new(shared_secret: &[u8; 32], transcript_hash: &[u8; 32], role: CipherRole) -> Self {
        let SessionKeys {
            media_key,
            input_key,
            ..
        } = derive_session_keys(shared_secret, transcript_hash);

        let (enc_key, dec_key, enc_tag, dec_tag) = match role {
            CipherRole::Host => (media_key, input_key, HOST_TO_CLIENT_TAG, CLIENT_TO_HOST_TAG),
            CipherRole::Client => (input_key, media_key, CLIENT_TO_HOST_TAG, HOST_TO_CLIENT_TAG),
        };

        let encrypt_cipher = ChaCha20Poly1305::new(Key::from_slice(&enc_key));
        let decrypt_cipher = ChaCha20Poly1305::new(Key::from_slice(&dec_key));

        Self {
            encrypt_cipher,
            decrypt_cipher,
            encrypt_tag: enc_tag,
            decrypt_tag: dec_tag,
            send_seq: AtomicU64::new(0),
            recv_seq: Arc::new(AtomicU64::new(0)),
            role,
        }
    }

    pub fn role(&self) -> CipherRole {
        self.role
    }

    /// Construct 12-byte Nonce from 4-byte role tag and 8-byte sequence number
    fn build_nonce(tag: &[u8; 4], seq: u64) -> Nonce {
        let mut nonce_bytes = [0u8; 12];
        nonce_bytes[0..4].copy_from_slice(tag);
        nonce_bytes[4..12].copy_from_slice(&seq.to_be_bytes());
        *Nonce::from_slice(&nonce_bytes)
    }

    /// Encrypt plaintext with next monotonic sequence number and role tag
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(u64, Vec<u8>), CryptoError> {
        let seq = self.send_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let nonce = Self::build_nonce(&self.encrypt_tag, seq);
        let ciphertext = self
            .encrypt_cipher
            .encrypt(&nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;
        Ok((seq, ciphertext))
    }

    /// Decrypt ciphertext with provided sequence number and role tag, verifying MAC
    pub fn decrypt(&self, seq: u64, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let prev = self.recv_seq.load(Ordering::SeqCst);
        if seq <= prev {
            return Err(CryptoError::ReplayDetected {
                expected: prev + 1,
                actual: seq,
            });
        }

        let nonce = Self::build_nonce(&self.decrypt_tag, seq);
        let plaintext = self
            .decrypt_cipher
            .decrypt(&nonce, ciphertext)
            .map_err(|_| CryptoError::AuthenticationFailed)?;

        self.recv_seq.store(seq, Ordering::SeqCst);
        Ok(plaintext)
    }

    /// Packs plaintext into a length-delimited wire packet:
    /// `[4 bytes: total packet len] + [8 bytes: seq] + [ciphertext + 16B Poly1305 tag]`
    pub fn pack_encrypted_packet(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let (seq, ciphertext) = self.encrypt(plaintext)?;
        let packet_len = 8 + ciphertext.len(); // 8 bytes sequence + ciphertext
        let mut buf = Vec::with_capacity(4 + packet_len);
        buf.extend_from_slice(&(packet_len as u32).to_be_bytes());
        buf.extend_from_slice(&seq.to_be_bytes());
        buf.extend_from_slice(&ciphertext);
        Ok(buf)
    }

    /// Unpacks and decrypts wire packet payload: `[8 bytes: seq] + [ciphertext + tag]`
    pub fn unpack_encrypted_payload(&self, payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        if payload.len() < 8 + 16 {
            // At least 8-byte sequence + 16-byte Poly1305 tag
            return Err(CryptoError::InvalidPacket(format!(
                "Packet payload too short: {} bytes, expected at least 24 bytes",
                payload.len()
            )));
        }

        let seq = u64::from_be_bytes(payload[0..8].try_into().unwrap());
        let ciphertext = &payload[8..];
        self.decrypt(seq, ciphertext)
    }
}
