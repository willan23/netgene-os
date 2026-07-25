//! Persistent storage for Gene identities.
//!
//! Saves/loads NetGene structs as JSON files in a configurable directory.
//! The private key (PKCS8) is stored separately in a `.key` file.

use std::path::{Path, PathBuf};
use anyhow::Result;
use serde_json;
use base64::Engine as _;

use crate::identity::NetGene;
use crate::crypto::GeneKeyPair;

/// Default gene storage directory.
pub fn default_gene_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".netgene").join("genes")
}

/// Save a NetGene and its keypair to disk.
pub fn save_gene(gene: &NetGene, kp: &GeneKeyPair, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;

    // Save identity JSON
    let gene_path = dir.join(format!("{}.gene.json", gene.id));
    let json = serde_json::to_string_pretty(gene)?;
    std::fs::write(&gene_path, json)?;

    // Save private key (PKCS8 base64)
    let key_path = dir.join(format!("{}.key", gene.id));
    std::fs::write(&key_path, kp.pkcs8_b64())?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

/// Load a NetGene from disk.
pub fn load_gene(gene_id: &str, dir: &Path) -> Result<(NetGene, GeneKeyPair)> {
    let gene_path = dir.join(format!("{}.gene.json", gene_id));
    let key_path = dir.join(format!("{}.key", gene_id));

    let json = std::fs::read_to_string(&gene_path)?;
    let gene: NetGene = serde_json::from_str(&json)?;

    let key_b64 = std::fs::read_to_string(&key_path)?;
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64.trim())?;
    let kp = GeneKeyPair::from_pkcs8(key_bytes)?;

    Ok((gene, kp))
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
