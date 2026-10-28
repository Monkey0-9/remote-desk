use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Signature verification failed")]
    SignatureInvalid,
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
}

pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Generate a cryptographically secure fresh Ed25519 identity keypair
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        Self { signing_key }
    }

    /// Construct from raw 32-byte private key seed
    pub fn from_bytes(bytes: &[u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(bytes);
        Self { signing_key }
    }

    /// Export raw 32-byte secret seed for OS-protected keystore persistence
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Obtain public verifying key
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Export public key as raw 32 bytes
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Derive stable 9-digit human-friendly Device ID locator (e.g. "rd-4928-1029")
    pub fn device_id(&self) -> String {
        Self::derive_device_id(&self.verifying_key())
    }

    /// Derive device ID from public key
    pub fn derive_device_id(vk: &VerifyingKey) -> String {
        let hash = Sha256::digest(vk.as_bytes());
        let part1 = u16::from_be_bytes([hash[0], hash[1]]) % 10000;
        let part2 = u16::from_be_bytes([hash[2], hash[3]]) % 10000;
        format!("rd-{:04}-{:04}", part1, part2)
    }

    /// Sign arbitrary payload bytes
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        self.signing_key.sign(message).to_bytes()
    }

    /// Verify signature from remote public key
    pub fn verify(
        remote_vk: &VerifyingKey,
        message: &[u8],
        signature_bytes: &[u8; 64],
    ) -> Result<(), IdentityError> {
        let signature = Signature::from_bytes(signature_bytes);
        remote_vk
            .verify(message, &signature)
            .map_err(|_| IdentityError::SignatureInvalid)
    }
}
