use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
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

#[derive(Clone)]
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

    /// Atomically writes data to path using a tempfile and rename, setting 0600 permissions on Unix.
    pub fn save_atomic(path: &Path, data: &[u8]) -> Result<(), IdentityError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file_stem = path.file_name().and_then(|s| s.to_str()).unwrap_or("key");
        let tmp_path = path.with_file_name(format!(".{}.tmp.{}", file_stem, rand::random::<u32>()));

        #[cfg(unix)]
        {
            use std::fs::OpenOptions;
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;

            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&tmp_path)?;
            file.write_all(data)?;
            file.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            fs::write(&tmp_path, data)?;
        }

        fs::rename(&tmp_path, path)?;
        Ok(())
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
            let identity = Self::generate();
            let seed = identity.to_bytes();
            Self::save_atomic(path, &seed)?;
            Ok(identity)
        }
    }

    /// Rotates identity: generates a fresh key, atomically saves previous key as `.bak`, and writes new key securely
    pub fn rotate_identity(path: &Path) -> Result<(Self, Self), IdentityError> {
        let old_identity = Self::load_or_generate(path)?;

        // Atomically backup old key
        let backup_path = path.with_extension("key.bak");
        let old_seed = old_identity.to_bytes();
        Self::save_atomic(&backup_path, &old_seed)?;

        // Generate fresh identity and atomically overwrite main key path
        let new_identity = Self::generate();
        let new_seed = new_identity.to_bytes();
        Self::save_atomic(path, &new_seed)?;

        Ok((new_identity, old_identity))
    }

    /// Recovers identity from backup `.bak` file if main key is lost or corrupted
    pub fn recover_from_backup(path: &Path) -> Result<Self, IdentityError> {
        let backup_path = path.with_extension("key.bak");
        if !backup_path.exists() {
            return Err(IdentityError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Backup file not found at {:?}", backup_path),
            )));
        }
        let identity = Self::load_or_generate(&backup_path)?;
        Self::save_atomic(path, &identity.to_bytes())?;
        Ok(identity)
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
        Self::derive_device_id_from_bytes(vk.as_bytes())
    }

    /// Derive stable 9-digit human-friendly Device ID locator from raw 32-byte public key
    pub fn derive_device_id_from_bytes(public_key: &[u8; 32]) -> String {
        let hash = Sha256::digest(public_key);
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

/// Record of a trusted paired peer device
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustedDevice {
    pub device_id: String,
    pub public_key_hex: String,
    pub fingerprint_hex: String,
    pub friendly_name: String,
    pub paired_at_secs: u64,
    pub last_seen_secs: u64,
    pub is_revoked: bool,
}

/// Secure persistent repository for paired devices and trust relationships
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrustedDeviceStore {
    devices: std::collections::HashMap<String, TrustedDevice>,
}

fn decode_hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

impl TrustedDeviceStore {
    pub fn new() -> Self {
        Self {
            devices: std::collections::HashMap::new(),
        }
    }

    /// Load trusted device store from JSON file on disk
    pub fn load_from_file(path: &Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Ok(Self::new());
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(path) {
                let mode = meta.permissions().mode() & 0o777;
                if (mode & 0o077) != 0 {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o600);
                    let _ = fs::set_permissions(path, perms);
                }
            }
        }

        let content = fs::read_to_string(path)?;
        let store: Self = serde_json::from_str(&content).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to parse trusted device store: {}", e),
            )
        })?;
        Ok(store)
    }

    /// Persist trusted device store to disk with 0600 permissions atomically
    pub fn save_to_file(&self, path: &Path) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(self).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Failed to serialize trusted devices: {}", e),
            )
        })?;

        DeviceIdentity::save_atomic(path, json.as_bytes()).map_err(|e| match e {
            IdentityError::Io(io_err) => io_err,
            other => std::io::Error::other(other.to_string()),
        })
    }

    /// Add or update a trusted device
    pub fn trust_device(
        &mut self,
        device_id: String,
        public_key_hex: String,
        friendly_name: String,
        now_secs: u64,
    ) {
        // Calculate SHA-256 fingerprint from public key hex
        let fp = if let Some(bytes) = decode_hex_bytes(&public_key_hex) {
            let hash = Sha256::digest(&bytes);
            let mut s = String::with_capacity(64);
            for b in hash {
                use std::fmt::Write;
                write!(&mut s, "{:02x}", b).unwrap();
            }
            s
        } else {
            String::new()
        };

        self.devices.insert(
            device_id.clone(),
            TrustedDevice {
                device_id,
                public_key_hex,
                fingerprint_hex: fp,
                friendly_name,
                paired_at_secs: now_secs,
                last_seen_secs: now_secs,
                is_revoked: false,
            },
        );
    }

    /// Revoke trust for a device
    pub fn revoke_device(&mut self, device_id: &str) -> bool {
        if let Some(device) = self.devices.get_mut(device_id) {
            device.is_revoked = true;
            true
        } else {
            false
        }
    }

    /// Check if device ID and public key are trusted and not revoked
    pub fn is_trusted(&self, device_id: &str, public_key_hex: &str) -> bool {
        if let Some(device) = self.devices.get(device_id) {
            !device.is_revoked && device.public_key_hex.eq_ignore_ascii_case(public_key_hex)
        } else {
            false
        }
    }

    /// Get list of all trusted devices
    pub fn list_devices(&self) -> Vec<TrustedDevice> {
        self.devices.values().cloned().collect()
    }

    /// Count active trusted devices
    pub fn count(&self) -> usize {
        self.devices.values().filter(|d| !d.is_revoked).count()
    }
}
