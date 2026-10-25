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

/// Operating system keystore backend.
pub struct OsKeyring {
    service: String,
}

impl OsKeyring {
    pub fn new() -> Self {
        Self { service: KEYRING_SERVICE.to_string() }
    }

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
