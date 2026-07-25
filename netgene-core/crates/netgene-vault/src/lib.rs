//! NetGene Personal Vault - P2P Encrypted File System
//! 
//! Handles chunking, encryption (Gene Cryptography / AES-GCM), and distributed storage metadata.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;
use uuid::Uuid;
use ring::aead::{self, BoundKey, Nonce, OpeningKey, SealingKey, UnboundKey, AES_256_GCM};
use ring::rand::{SecureRandom, SystemRandom};
use std::path::PathBuf;
use std::fs;

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
    encryption_key: [u8; 32], // Simulates the Master Gene Key
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

impl NetGeneVault {
    pub fn new(storage_dir: PathBuf) -> Result<Self> {
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir)?;
        }
        
        let mut key = [0u8; 32];
        SystemRandom::new().fill(&mut key).map_err(|_| anyhow::anyhow!("Failed to generate random key"))?;

        info!("🛡️ NetGene Vault initialized at {:?}", storage_dir);
        Ok(Self { storage_dir, encryption_key: key })
    }

    /// Encrypts and chunks a file payload
    pub fn store_file(&self, filename: String, data: &[u8]) -> Result<VaultFile> {
        info!("📦 Storing file '{}' ({} bytes) in Vault", filename, data.len());
        
        let file_id = Uuid::new_v4();
        let chunk_size = 1024 * 1024; // 1MB chunks
        let mut chunk_ids = Vec::new();

        for (i, chunk) in data.chunks(chunk_size).enumerate() {
            let chunk_id = Uuid::new_v4();
            let encrypted = self.encrypt_chunk(chunk)?;
            let chunk_path = self.storage_dir.join(chunk_id.to_string());
            fs::write(&chunk_path, encrypted)?;
            chunk_ids.push(chunk_id);
            info!("   -> Stored chunk {}/{} ({})", i + 1, (data.len() as f64 / chunk_size as f64).ceil(), chunk_id);
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
        let meta_str = fs::read_to_string(&meta_path)?;
        let meta: VaultFile = serde_json::from_str(&meta_str)?;

        let mut data = Vec::new();
        for chunk_id in meta.chunk_ids {
            let chunk_path = self.storage_dir.join(chunk_id.to_string());
            let encrypted = fs::read(&chunk_path)?;
            let decrypted = self.decrypt_chunk(&encrypted)?;
            data.extend_from_slice(&decrypted);
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

    fn encrypt_chunk(&self, data: &[u8]) -> Result<Vec<u8>> {
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.encryption_key).map_err(|_| anyhow::anyhow!("Key error"))?;
        let mut nonce_bytes = [0u8; 12];
        SystemRandom::new().fill(&mut nonce_bytes).map_err(|_| anyhow::anyhow!("Nonce error"))?;
        
        let nonce = Nonce::assume_unique_for_key(nonce_bytes);
        let mut sealing_key = SealingKey::new(unbound_key, OneNonceSequence::new(nonce));
        
        let mut in_out = data.to_vec();
        sealing_key.seal_in_place_append_tag(aead::Aad::empty(), &mut in_out).map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        
        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&in_out);
        Ok(result)
    }

    fn decrypt_chunk(&self, data: &[u8]) -> Result<Vec<u8>> {
        if data.len() < 12 {
            anyhow::bail!("Invalid chunk size");
        }
        let (nonce_bytes, ciphertext) = data.split_at(12);
        
        let mut nonce_arr = [0u8; 12];
        nonce_arr.copy_from_slice(nonce_bytes);
        
        let unbound_key = UnboundKey::new(&AES_256_GCM, &self.encryption_key).map_err(|_| anyhow::anyhow!("Key error"))?;
        let nonce = Nonce::assume_unique_for_key(nonce_arr);
        let mut opening_key = OpeningKey::new(unbound_key, OneNonceSequence::new(nonce));
        
        let mut in_out = ciphertext.to_vec();
        let decrypted = opening_key.open_in_place(aead::Aad::empty(), &mut in_out).map_err(|_| anyhow::anyhow!("Decryption failed"))?;
        
        Ok(decrypted.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_vault_store_and_retrieve() {
        let dir = tempdir().unwrap();
        let vault = NetGeneVault::new(dir.path().to_path_buf()).unwrap();
        
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
    fn test_vault_list_files() {
        let dir = tempdir().unwrap();
        let vault = NetGeneVault::new(dir.path().to_path_buf()).unwrap();
        
        vault.store_file("file1.txt".to_string(), b"data1").unwrap();
        vault.store_file("file2.txt".to_string(), b"data2").unwrap();
        
        let files = vault.list_files().unwrap();
        assert_eq!(files.len(), 2);
    }
}
