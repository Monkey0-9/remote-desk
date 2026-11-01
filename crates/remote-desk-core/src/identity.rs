use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum IdentityError {
    #[error("Signature verification failed")]
    SignatureInvalid,
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Insecure file permissions: {0}")]
    InsecurePermissions(String),
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

    /// Load identity from file or generate and persist securely (0600 on Unix)
    pub fn load_or_generate(path: &Path) -> Result<Self, IdentityError> {
        if path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = fs::metadata(path)?.permissions();
                let mode = perms.mode() & 0o777;
                // If group or world has any permissions, tighten it to 0600
                if (mode & 0o077) != 0 {
                    let mut new_perms = perms;
                    new_perms.set_mode(0o600);
                    fs::set_permissions(path, new_perms)?;
                }
            }

            let data = fs::read(path)?;
            if data.len() != 32 {
                return Err(IdentityError::InvalidKeyFormat(format!(
                    "Expected 32 bytes seed, got {} bytes",
                    data.len()
                )));
            }
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&data);
            Ok(Self::from_bytes(&seed))
        } else {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let identity = Self::generate();
            let seed = identity.to_bytes();

            #[cfg(unix)]
            {
                use std::fs::OpenOptions;
                use std::io::Write;
                use std::os::unix::fs::OpenOptionsExt;

                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(path)?;
                file.write_all(&seed)?;
            }

            #[cfg(not(unix))]
            {
                fs::write(path, &seed)?;
            }

            Ok(identity)
        }
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

    /// Derive full 32-byte SHA-256 fingerprint of the public key
    pub fn fingerprint_bytes(&self) -> [u8; 32] {
        let hash = Sha256::digest(self.public_key_bytes());
        let mut out = [0u8; 32];
        out.copy_from_slice(&hash);
        out
    }

    /// Export public key fingerprint as lowercase 64-character hexadecimal string
    pub fn fingerprint_hex(&self) -> String {
        let fp = self.fingerprint_bytes();
        let mut s = String::with_capacity(64);
        for &b in &fp {
            use std::fmt::Write;
            write!(&mut s, "{:02x}", b).unwrap();
        }
        s
    }

    /// Export public key as lowercase 64-character hexadecimal string
    pub fn public_key_hex(&self) -> String {
        let bytes = self.public_key_bytes();
        let mut s = String::with_capacity(64);
        for &b in &bytes {
            use std::fmt::Write;
            write!(&mut s, "{:02x}", b).unwrap();
        }
        s
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

    /// Check if remote public key is in the trusted allowlist
    pub fn is_trusted(&self, remote_vk: &VerifyingKey, allowlist: &[[u8; 32]]) -> bool {
        let remote_bytes = remote_vk.to_bytes();
        allowlist.contains(&remote_bytes)
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
