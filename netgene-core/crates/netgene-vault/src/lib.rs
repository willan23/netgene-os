//! NetGene Personal Vault - local encrypted file store.
//!
//! Files are split into 1 MB chunks, each sealed with AES-256-GCM. The vault
//! key is generated once and kept in a [`SecretStore`] (normally the OS
//! keystore). Every chunk is bound to its file id, position and chunk count
//! through the AEAD associated data, so chunks cannot be swapped between
//! files, reordered or truncated without detection.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use ring::aead::{self, BoundKey, Nonce, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use std::path::PathBuf;
use std::fs;
use zeroize::Zeroizing;
use netgene_gene::crypto::short_fingerprint;
use netgene_gene::SecretStore;

/// On-disk chunk format version (first byte of every chunk file).
const CHUNK_FORMAT_V1: u8 = 1;
const CHUNK_SIZE: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultFile {
    pub file_id: Uuid,
    pub filename: String,
    pub size_bytes: u64,
    pub chunk_ids: Vec<Uuid>,
    pub created_at: i64,
}

pub struct NetGeneVault {
    storage_dir: PathBuf,
    encryption_key: Zeroizing<[u8; 32]>,
}

struct OneNonceSequence(Option<aead::Nonce>);

impl OneNonceSequence {
    fn new(nonce: aead::Nonce) -> Self {
        Self(Some(nonce))
    }
}

impl aead::NonceSequence for OneNonceSequence {
    fn advance(&mut self) -> Result<aead::Nonce, ring::error::Unspecified> {
        self.0.take().ok_or(ring::error::Unspecified)
    }
}

/// Associated data binding a chunk to (file id, index, total chunks).
fn chunk_aad(file_id: Uuid, index: u32, total: u32) -> [u8; 24] {
    let mut aad = [0u8; 24];
    aad[..16].copy_from_slice(file_id.as_bytes());
    aad[16..20].copy_from_slice(&index.to_be_bytes());
    aad[20..].copy_from_slice(&total.to_be_bytes());
    aad
}

impl NetGeneVault {
    /// Open (or create) the vault at `storage_dir`. Its key is created on
    /// first use and stored in `secrets`, so files survive restarts.
    pub fn open(storage_dir: PathBuf, secrets: &dyn SecretStore) -> Result<Self> {
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        let canonical = fs::canonicalize(&storage_dir)?;
        let secret_name = format!("vault:{}", short_fingerprint(canonical.to_string_lossy().as_bytes()));

        let mut key = Zeroizing::new([0u8; 32]);
        match secrets.get(&secret_name)? {
            Some(stored) if stored.len() == 32 => key.copy_from_slice(&stored),
            Some(_) => anyhow::bail!("Vault key '{}' in the secret store is corrupted", secret_name),
            None => {
                SystemRandom::new()
                    .fill(key.as_mut())
                    .map_err(|_| anyhow::anyhow!("Failed to generate random key"))?;
                secrets.set(&secret_name, key.as_ref())?;
                info!("🔐 New vault key created in the secret store ({})", secret_name);
            }
        }

        info!("🛡️ NetGene Vault opened at {:?}", storage_dir);
        Ok(Self { storage_dir, encryption_key: key })
    }

    /// Encrypts and chunks a file payload
    pub fn store_file(&self, filename: String, data: &[u8]) -> Result<VaultFile> {
        info!("📦 Storing file '{}' ({} bytes) in Vault", filename, data.len());

        let file_id = Uuid::new_v4();
        // An empty file still gets one (empty) authenticated chunk.
        let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[]] } else { data.chunks(CHUNK_SIZE).collect() };
        let total = u32::try_from(chunks.len())?;
        let mut chunk_ids = Vec::with_capacity(chunks.len());

        for (i, chunk) in chunks.into_iter().enumerate() {
            let chunk_id = Uuid::new_v4();
            let encrypted = self.encrypt_chunk(chunk, &chunk_aad(file_id, i as u32, total))?;
            fs::write(self.storage_dir.join(chunk_id.to_string()), encrypted)?;
            chunk_ids.push(chunk_id);
            info!("   -> Stored chunk {}/{} ({})", i + 1, total, chunk_id);
        }

        let meta = VaultFile {
            file_id,
            filename,
            size_bytes: data.len() as u64,
            chunk_ids,
            created_at: chrono::Utc::now().timestamp(),
        };

        let meta_path = self.storage_dir.join(format!("{}.meta", file_id));
        fs::write(&meta_path, serde_json::to_string(&meta)?)?;

        Ok(meta)
    }

    /// Retrieves and decrypts a file
    pub fn retrieve_file(&self, file_id: Uuid) -> Result<(String, Vec<u8>)> {
        let meta_path = self.storage_dir.join(format!("{}.meta", file_id));
        let meta: VaultFile = serde_json::from_str(&fs::read_to_string(&meta_path)?)?;
        if meta.file_id != file_id {
            anyhow::bail!("Vault metadata does not match file {}", file_id);
        }

        let total = u32::try_from(meta.chunk_ids.len())?;
        let mut data = Vec::new();
        for (i, chunk_id) in meta.chunk_ids.iter().enumerate() {
            let encrypted = fs::read(self.storage_dir.join(chunk_id.to_string()))?;
            let decrypted = self.decrypt_chunk(&encrypted, &chunk_aad(file_id, i as u32, total))?;
            data.extend_from_slice(&decrypted);
        }
        if data.len() as u64 != meta.size_bytes {
            anyhow::bail!("Vault file {} has an unexpected size", file_id);
        }

        info!("🔓 Retrieved and decrypted file '{}'", meta.filename);
        Ok((meta.filename, data))
    }

    pub fn list_files(&self) -> Result<Vec<VaultFile>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "meta") {
                if let Ok(meta_str) = fs::read_to_string(&path) {
                    if let Ok(meta) = serde_json::from_str(&meta_str) {
                        files.push(meta);
                    }
                }
            }
        }
        Ok(files)
    }

    fn encrypt_chunk(&self, data: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, self.encryption_key.as_ref())
            .map_err(|_| anyhow::anyhow!("Key error"))?;
        let mut nonce_bytes = [0u8; 12];
        SystemRandom::new().fill(&mut nonce_bytes).map_err(|_| anyhow::anyhow!("Nonce error"))?;

        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut sealing_key = SealingKey::new(unbound_key, OneNonceSequence::new(nonce));

        let mut in_out = data.to_vec();
        sealing_key
            .seal_in_place_append_tag(aead::Aad::from(aad), &mut in_out)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        let mut result = Vec::with_capacity(1 + 12 + in_out.len());
        result.push(CHUNK_FORMAT_V1);
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&in_out);
        Ok(result)
    }

    fn decrypt_chunk(&self, data: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 1 + 12 || data[0] != CHUNK_FORMAT_V1 {
            anyhow::bail!("Unsupported or corrupted vault chunk");
        }
        let (nonce_bytes, ciphertext) = data[1..].split_at(12);

        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(nonce_bytes);

        let unbound_key = UnboundKey::new(&AES_256_GCM, self.encryption_key.as_ref())
            .map_err(|_| anyhow::anyhow!("Key error"))?;
        let nonce = Nonce::assume_unique_for_key(nonce_arr);
        let mut opening_key = OpeningKey::new(unbound_key, OneNonceSequence::new(nonce));

        let mut in_out = ciphertext.to_vec();
        let decrypted = opening_key
            .open_in_place(aead::Aad::from(aad), &mut in_out)
            .map_err(|_| anyhow::anyhow!("Decryption failed (wrong key or tampered chunk)"))?;

        Ok(decrypted.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netgene_gene::MemorySecretStore;
    use tempfile::tempdir;

    fn open(dir: &std::path::Path, secrets: &MemorySecretStore) -> NetGeneVault {
        NetGeneVault::open(dir.to_path_buf(), secrets).unwrap()
    }

    #[test]
    fn test_vault_store_and_retrieve() {
        let dir = tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let vault = open(dir.path(), &secrets);

        let data = b"Hello NetGene Vault! This is a test file.";
        let stored_meta = vault.store_file("test.txt".to_string(), data).unwrap();

        assert_eq!(stored_meta.filename, "test.txt");
        assert_eq!(stored_meta.size_bytes, data.len() as u64);
        assert!(!stored_meta.chunk_ids.is_empty());

        let (filename, retrieved_data) = vault.retrieve_file(stored_meta.file_id).unwrap();
        assert_eq!(filename, "test.txt");
        assert_eq!(retrieved_data, data);
    }

    #[test]
    fn test_vault_survives_reopen() {
        let dir = tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let meta = open(dir.path(), &secrets).store_file("persist.txt".to_string(), b"still here").unwrap();

        let (_, data) = open(dir.path(), &secrets).retrieve_file(meta.file_id).unwrap();
        assert_eq!(data, b"still here");
    }

    #[test]
    fn test_vault_rejects_wrong_key() {
        let dir = tempdir().unwrap();
        let meta = open(dir.path(), &MemorySecretStore::new()).store_file("x".to_string(), b"secret").unwrap();

        // A different secret store means a different key.
        assert!(open(dir.path(), &MemorySecretStore::new()).retrieve_file(meta.file_id).is_err());
    }

    #[test]
    fn test_vault_rejects_swapped_chunks() {
        let dir = tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let vault = open(dir.path(), &secrets);

        let a = vault.store_file("a".to_string(), b"file A").unwrap();
        let b = vault.store_file("b".to_string(), b"file B").unwrap();

        // Point file A's metadata at file B's chunk.
        let mut forged = a.clone();
        forged.chunk_ids = b.chunk_ids.clone();
        fs::write(dir.path().join(format!("{}.meta", a.file_id)), serde_json::to_string(&forged).unwrap()).unwrap();

        assert!(vault.retrieve_file(a.file_id).is_err());
    }

    #[test]
    fn test_vault_multi_chunk_and_empty() {
        let dir = tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let vault = open(dir.path(), &secrets);

        let big = vec![7u8; CHUNK_SIZE * 2 + 10];
        let meta = vault.store_file("big".to_string(), &big).unwrap();
        assert_eq!(meta.chunk_ids.len(), 3);
        assert_eq!(vault.retrieve_file(meta.file_id).unwrap().1, big);

        let empty = vault.store_file("empty".to_string(), b"").unwrap();
        assert!(vault.retrieve_file(empty.file_id).unwrap().1.is_empty());
    }

    #[test]
    fn test_vault_list_files() {
        let dir = tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let vault = open(dir.path(), &secrets);

        vault.store_file("file1.txt".to_string(), b"data1").unwrap();
        vault.store_file("file2.txt".to_string(), b"data2").unwrap();

        assert_eq!(vault.list_files().unwrap().len(), 2);
    }
}
