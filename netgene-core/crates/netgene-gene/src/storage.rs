//! Persistent storage for Gene identities.
//!
//! The public identity is saved as JSON in a configurable directory.
//! The private key (PKCS8) is kept in a [`SecretStore`] (by default the OS
//! keystore), never on disk. Legacy `.key` files written by NetGene < 0.4
//! are migrated into the store on first load and then deleted.

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use serde_json;
use base64::Engine as _;

use crate::identity::NetGene;
use crate::crypto::GeneKeyPair;
use crate::secret::SecretStore;

/// Default gene storage directory.
pub fn default_gene_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".netgene").join("genes")
}

/// Name of the secret that holds a gene's private key.
pub fn gene_secret_name(gene_id: &str) -> String {
    format!("gene:{}", gene_id)
}

/// Save a NetGene (public JSON on disk) and its private key (secret store).
pub fn save_gene(gene: &NetGene, kp: &GeneKeyPair, dir: &Path, secrets: &dyn SecretStore) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    secrets.set(&gene_secret_name(&gene.id.to_string()), kp.pkcs8_bytes())?;

    let gene_path = dir.join(format!("{}.gene.json", gene.id));
    let json = serde_json::to_string_pretty(gene)?;
    std::fs::write(&gene_path, json)?;

    Ok(())
}

/// Load a NetGene and its private key.
pub fn load_gene(gene_id: &str, dir: &Path, secrets: &dyn SecretStore) -> Result<(NetGene, GeneKeyPair)> {
    let gene_path = dir.join(format!("{}.gene.json", gene_id));
    let json = std::fs::read_to_string(&gene_path)
        .with_context(|| format!("Gene '{}' not found in {}", gene_id, dir.display()))?;
    let gene: NetGene = serde_json::from_str(&json)?;

    let secret_name = gene_secret_name(gene_id);
    let pkcs8 = match secrets.get(&secret_name)? {
        Some(bytes) => bytes,
        None => migrate_legacy_key(gene_id, dir, secrets)?
            .with_context(|| format!("Private key for gene '{}' not found in the secret store", gene_id))?,
    };
    let kp = GeneKeyPair::from_pkcs8(pkcs8)?;

    if kp.public_key_b64() != gene.public_key_b64 {
        anyhow::bail!("Private key for gene '{}' does not match its public key", gene_id);
    }

    Ok((gene, kp))
}

/// Move a legacy plain-text `<id>.key` file into the secret store.
fn migrate_legacy_key(gene_id: &str, dir: &Path, secrets: &dyn SecretStore) -> Result<Option<Vec<u8>>> {
    let key_path = dir.join(format!("{}.key", gene_id));
    if !key_path.exists() {
        return Ok(None);
    }

    let key_b64 = std::fs::read_to_string(&key_path)?;
    let key_bytes = base64::engine::general_purpose::STANDARD.decode(key_b64.trim())?;
    GeneKeyPair::from_pkcs8(key_bytes.clone())?;

    secrets.set(&gene_secret_name(gene_id), &key_bytes)?;
    std::fs::remove_file(&key_path)?;
    tracing::info!(gene_id, "Migrated legacy plain-text gene key into the secret store");

    Ok(Some(key_bytes))
}

/// List all stored genes in a directory.
pub fn list_genes(dir: &Path) -> Result<Vec<NetGene>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut genes = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            let json = std::fs::read_to_string(&path)?;
            if let Ok(gene) = serde_json::from_str::<NetGene>(&json) {
                genes.push(gene);
            }
        }
    }

    genes.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(genes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::MemorySecretStore;

    #[test]
    fn test_save_and_load_keeps_key_off_disk() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let (gene, kp) = NetGene::generate_master("Storage Test").unwrap();

        save_gene(&gene, &kp, dir.path(), &secrets).unwrap();

        assert!(!dir.path().join(format!("{}.key", gene.id)).exists());
        let (loaded, loaded_kp) = load_gene(&gene.id.to_string(), dir.path(), &secrets).unwrap();
        assert_eq!(loaded.id, gene.id);
        assert_eq!(loaded_kp.public_key_bytes(), kp.public_key_bytes());
    }

    #[test]
    fn test_legacy_key_file_is_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let (gene, kp) = NetGene::generate_master("Legacy").unwrap();

        std::fs::write(dir.path().join(format!("{}.gene.json", gene.id)), serde_json::to_string(&gene).unwrap()).unwrap();
        let key_path = dir.path().join(format!("{}.key", gene.id));
        std::fs::write(&key_path, kp.pkcs8_b64()).unwrap();

        let (_, loaded_kp) = load_gene(&gene.id.to_string(), dir.path(), &secrets).unwrap();
        assert_eq!(loaded_kp.public_key_bytes(), kp.public_key_bytes());
        assert!(!key_path.exists(), "legacy key file must be deleted after migration");
        assert!(secrets.get(&gene_secret_name(&gene.id.to_string())).unwrap().is_some());
    }

    #[test]
    fn test_mismatched_key_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = MemorySecretStore::new();
        let (gene, kp) = NetGene::generate_master("A").unwrap();
        let (_, other_kp) = NetGene::generate_master("B").unwrap();

        save_gene(&gene, &kp, dir.path(), &secrets).unwrap();
        secrets.set(&gene_secret_name(&gene.id.to_string()), other_kp.pkcs8_bytes()).unwrap();

        assert!(load_gene(&gene.id.to_string(), dir.path(), &secrets).is_err());
    }
}
