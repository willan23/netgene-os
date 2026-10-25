//! Secret storage backends.
//!
//! Private key material never touches disk in plain text: it lives in the
//! operating system keystore (Windows Credential Manager, macOS Keychain,
//! Linux Secret Service). `MemorySecretStore` exists for tests and
//! ephemeral nodes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};

/// Service name under which NetGene secrets are registered in the OS keystore.
pub const KEYRING_SERVICE: &str = "netgene-os";

/// A named-secret store.
pub trait SecretStore: Send + Sync {
    /// Read a secret, `Ok(None)` if it does not exist.
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>>;
    /// Create or replace a secret.
    fn set(&self, name: &str, secret: &[u8]) -> Result<()>;
    /// Delete a secret (no error if it does not exist).
    fn delete(&self, name: &str) -> Result<()>;
}

impl<T: SecretStore + ?Sized> SecretStore for Box<T> {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        (**self).get(name)
    }
    fn set(&self, name: &str, secret: &[u8]) -> Result<()> {
        (**self).set(name, secret)
    }
    fn delete(&self, name: &str) -> Result<()> {
        (**self).delete(name)
    }
}

/// Operating system keystore backend.
pub struct OsKeyring {
    service: String,
}

impl OsKeyring {
    pub fn new() -> Self {
        Self { service: KEYRING_SERVICE.to_string() }
    }

    #[cfg(any(windows, target_os = "macos", target_os = "linux"))]
    fn entry(&self, name: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(&self.service, name)
            .with_context(|| format!("Cannot open OS keystore entry '{}'", name))
    }
}

impl Default for OsKeyring {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(windows, target_os = "macos", target_os = "linux"))]
impl SecretStore for OsKeyring {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match self.entry(name)?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e).with_context(|| format!("Cannot read secret '{}' from OS keystore", name)),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<()> {
        self.entry(name)?
            .set_secret(secret)
            .with_context(|| format!("Cannot write secret '{}' to OS keystore", name))
    }

    fn delete(&self, name: &str) -> Result<()> {
        match self.entry(name)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e).with_context(|| format!("Cannot delete secret '{}' from OS keystore", name)),
        }
    }
}

/// Platforms without a supported keystore backend yet (iOS…):
/// fail loudly instead of silently keeping keys in memory, which would lose
/// them on restart.
#[cfg(target_os = "android")]
fn android_entry(service: &str, name: &str) -> Result<keyring_core::Entry> {
    use keyring_core::api::CredentialStoreApi;
    static STORE: std::sync::OnceLock<std::sync::Arc<android_native_keyring_store::Store>> = std::sync::OnceLock::new();
    let store = match STORE.get() {
        Some(store) => store.clone(),
        None => {
            let store = android_native_keyring_store::Store::new()
                .map_err(|e| anyhow::anyhow!("Cannot open the Android Keystore: {}", e))?;
            STORE.get_or_init(|| store).clone()
        }
    };
    store
        .build(service, name, None)
        .map_err(|e| anyhow::anyhow!("Cannot open Android Keystore entry '{}': {}", name, e))
}

/// Android: secrets in SharedPreferences, encrypted with an AES key that
/// never leaves the Android Keystore.
#[cfg(target_os = "android")]
impl SecretStore for OsKeyring {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        match android_entry(&self.service, name)?.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Cannot read secret '{}' from the Android Keystore: {}", name, e)),
        }
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<()> {
        android_entry(&self.service, name)?
            .set_secret(secret)
            .map_err(|e| anyhow::anyhow!("Cannot write secret '{}' to the Android Keystore: {}", name, e))
    }

    fn delete(&self, name: &str) -> Result<()> {
        match android_entry(&self.service, name)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(anyhow::anyhow!("Cannot delete secret '{}' from the Android Keystore: {}", name, e)),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux", target_os = "android")))]
impl SecretStore for OsKeyring {
    fn get(&self, _name: &str) -> Result<Option<Vec<u8>>> {
        anyhow::bail!("No OS keystore backend on this platform yet (use NETGENE_SECRET_STORE=file)")
    }
    fn set(&self, _name: &str, _secret: &[u8]) -> Result<()> {
        anyhow::bail!("No OS keystore backend on this platform yet (use NETGENE_SECRET_STORE=file)")
    }
    fn delete(&self, _name: &str) -> Result<()> {
        anyhow::bail!("No OS keystore backend on this platform yet (use NETGENE_SECRET_STORE=file)")
    }
}

/// Passphrase-encrypted file store for headless servers and containers,
/// where no OS keystore (Secret Service) is running.
///
/// Key: PBKDF2-HMAC-SHA256 (600 000 iterations, random salt) of the
/// passphrase. Each secret: AES-256-GCM with a random nonce, the secret name
/// as associated data. The file is rewritten atomically on every change.
pub struct EncryptedFileStore {
    path: std::path::PathBuf,
    key: zeroize::Zeroizing<[u8; 32]>,
    salt: Vec<u8>,
    entries: Mutex<HashMap<String, (Vec<u8>, Vec<u8>)>>,
}

const FILE_STORE_ITERATIONS: u32 = 600_000;

#[derive(serde::Serialize, serde::Deserialize)]
struct FileStoreDoc {
    version: u32,
    kdf: String,
    iterations: u32,
    salt_b64: String,
    /// name → (nonce, ciphertext), base64.
    entries: std::collections::BTreeMap<String, (String, String)>,
}

impl EncryptedFileStore {
    /// Open (or create) the store at `path` with `passphrase`.
    pub fn open(path: std::path::PathBuf, passphrase: &str) -> Result<Self> {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        use ring::rand::SecureRandom;

        if passphrase.chars().count() < 12 {
            anyhow::bail!("Secret store passphrase must have at least 12 characters");
        }
        let (salt, iterations, entries) = if path.exists() {
            let doc: FileStoreDoc = serde_json::from_str(&std::fs::read_to_string(&path)?)
                .with_context(|| format!("Invalid secret store {}", path.display()))?;
            if doc.version != 1 || doc.kdf != "pbkdf2-hmac-sha256" {
                anyhow::bail!("Unsupported secret store format in {}", path.display());
            }
            let mut entries = HashMap::new();
            for (name, (nonce, ct)) in doc.entries {
                entries.insert(name, (B64.decode(nonce)?, B64.decode(ct)?));
            }
            (B64.decode(doc.salt_b64)?, doc.iterations, entries)
        } else {
            let mut salt = vec![0u8; 16];
            ring::rand::SystemRandom::new().fill(&mut salt).map_err(|_| anyhow::anyhow!("RNG failure"))?;
            (salt, FILE_STORE_ITERATIONS, HashMap::new())
        };

        let mut key = zeroize::Zeroizing::new([0u8; 32]);
        let iterations = std::num::NonZeroU32::new(iterations).context("Invalid iteration count")?;
        ring::pbkdf2::derive(ring::pbkdf2::PBKDF2_HMAC_SHA256, iterations, &salt, passphrase.as_bytes(), key.as_mut());

        let store = Self { path, key, salt, entries: Mutex::new(entries) };
        // Wrong passphrase: the first existing entry fails to decrypt.
        let first = store.entries.lock().unwrap_or_else(|e| e.into_inner()).keys().next().cloned();
        if let Some(name) = first {
            store.get(&name).context("Wrong passphrase for the secret store")?;
        }
        Ok(store)
    }

    fn cipher(&self) -> Result<ring::aead::LessSafeKey> {
        let unbound = ring::aead::UnboundKey::new(&ring::aead::AES_256_GCM, self.key.as_ref())
            .map_err(|_| anyhow::anyhow!("Key error"))?;
        Ok(ring::aead::LessSafeKey::new(unbound))
    }

    fn persist(&self, entries: &HashMap<String, (Vec<u8>, Vec<u8>)>) -> Result<()> {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let doc = FileStoreDoc {
            version: 1,
            kdf: "pbkdf2-hmac-sha256".into(),
            iterations: FILE_STORE_ITERATIONS,
            salt_b64: B64.encode(&self.salt),
            entries: entries.iter().map(|(k, (n, c))| (k.clone(), (B64.encode(n), B64.encode(c)))).collect(),
        };
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("tmp");
        let _ = std::fs::remove_file(&tmp);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        // Owner-only: the ciphertext + salt would otherwise allow offline
        // passphrase guessing by any local user.
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
        {
            use std::io::Write;
            let mut f = opts.open(&tmp)?;
            f.write_all(serde_json::to_string_pretty(&doc)?.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl SecretStore for EncryptedFileStore {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let Some((nonce, ct)) = entries.get(name) else { return Ok(None) };
        let nonce = ring::aead::Nonce::try_assume_unique_for_key(nonce).map_err(|_| anyhow::anyhow!("Corrupted nonce"))?;
        let mut buf = ct.clone();
        let plain = self
            .cipher()?
            .open_in_place(nonce, ring::aead::Aad::from(name.as_bytes()), &mut buf)
            .map_err(|_| anyhow::anyhow!("Cannot decrypt secret '{}' (wrong passphrase or tampered file)", name))?;
        Ok(Some(plain.to_vec()))
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<()> {
        use ring::rand::SecureRandom;
        let mut nonce = [0u8; 12];
        ring::rand::SystemRandom::new().fill(&mut nonce).map_err(|_| anyhow::anyhow!("RNG failure"))?;
        let mut buf = secret.to_vec();
        self.cipher()?
            .seal_in_place_append_tag(ring::aead::Nonce::assume_unique_for_key(nonce), ring::aead::Aad::from(name.as_bytes()), &mut buf)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.insert(name.to_string(), (nonce.to_vec(), buf));
        self.persist(&entries)
    }

    fn delete(&self, name: &str) -> Result<()> {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if entries.remove(name).is_some() {
            self.persist(&entries)?;
        }
        Ok(())
    }
}

/// The secret store selected by the environment:
/// - `NETGENE_SECRET_STORE=file` → [`EncryptedFileStore`] at `NETGENE_SECRET_FILE`
///   (default `~/.netgene/secrets.enc`), passphrase from `NETGENE_SECRET_PASSPHRASE`
///   or the file named by `NETGENE_SECRET_PASSPHRASE_FILE` (e.g. a Docker secret);
/// - otherwise the OS keystore.
pub fn default_store() -> Result<Box<dyn SecretStore>> {
    match std::env::var("NETGENE_SECRET_STORE").as_deref() {
        Ok("file") => {
            let path = std::env::var("NETGENE_SECRET_FILE").map(std::path::PathBuf::from).unwrap_or_else(|_| {
                crate::storage::default_gene_dir().parent().map(|p| p.join("secrets.enc")).unwrap_or_else(|| "secrets.enc".into())
            });
            let passphrase = match std::env::var("NETGENE_SECRET_PASSPHRASE") {
                Ok(p) => p,
                Err(_) => {
                    let file = std::env::var("NETGENE_SECRET_PASSPHRASE_FILE")
                        .context("Set NETGENE_SECRET_PASSPHRASE or NETGENE_SECRET_PASSPHRASE_FILE")?;
                    std::fs::read_to_string(file)?.trim_end().to_string()
                }
            };
            Ok(Box::new(EncryptedFileStore::open(path, &passphrase)?))
        }
        Ok("os") | Err(_) => Ok(Box::new(OsKeyring::new())),
        Ok(other) => anyhow::bail!("Unknown NETGENE_SECRET_STORE '{}' (use os or file)", other),
    }
}

/// In-process store; secrets vanish when it is dropped.
#[derive(Default, Clone)]
pub struct MemorySecretStore {
    secrets: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn get(&self, name: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.secrets.lock().unwrap_or_else(|e| e.into_inner()).get(name).cloned())
    }

    fn set(&self, name: &str, secret: &[u8]) -> Result<()> {
        self.secrets.lock().unwrap_or_else(|e| e.into_inner()).insert(name.to_string(), secret.to_vec());
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<()> {
        self.secrets.lock().unwrap_or_else(|e| e.into_inner()).remove(name);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypted_file_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.enc");
        let store = EncryptedFileStore::open(path.clone(), "correct horse battery").unwrap();
        store.set("gene:a", b"private key bytes").unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("private key bytes"), "secrets are not stored in clear");

        let reopened = EncryptedFileStore::open(path.clone(), "correct horse battery").unwrap();
        assert_eq!(reopened.get("gene:a").unwrap().as_deref(), Some(&b"private key bytes"[..]));
        assert!(EncryptedFileStore::open(path.clone(), "wrong passphrase!!").is_err());
        assert!(EncryptedFileStore::open(dir.path().join("x.enc"), "short").is_err());

        // Moving a ciphertext to another name fails (the name is associated data).
        std::fs::write(&path, text.replace("gene:a", "gene:b")).unwrap();
        assert!(EncryptedFileStore::open(path, "correct horse battery").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_encrypted_file_store_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secrets.enc");
        let store = EncryptedFileStore::open(path.clone(), "correct horse battery").unwrap();
        store.set("gene:a", b"k").unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn test_memory_store_roundtrip() {
        let store = MemorySecretStore::new();
        assert!(store.get("a").unwrap().is_none());
        store.set("a", b"secret").unwrap();
        assert_eq!(store.get("a").unwrap().as_deref(), Some(&b"secret"[..]));
        store.delete("a").unwrap();
        assert!(store.get("a").unwrap().is_none());
        store.delete("a").unwrap();
    }
}
