use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use sha2::Sha256;
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
