//! Local agent marketplace.
//!
//! Every manifest is signed by the publisher's Gene with a hybrid
//! (Ed25519 + ML-DSA-65) signature. Manifests are
//! verified when published, listed and installed; unsigned or tampered files
//! in the registry directory are ignored.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::warn;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature, NetGene};

const SIGNING_DOMAIN: &[u8] = b"netgene-agent-manifest-v2\0";
const MAX_ID_LEN: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<String>,
    /// Gene that signed this manifest.
    pub publisher_gene_id: String,
    /// Publisher's public key (Ed25519 + optional ML-DSA-65).
    pub publisher_key: HybridPublicKey,
    /// Hybrid signature over [`AgentManifest::signing_payload`].
    pub signature: HybridSignature,
}

/// Agent ids are used as file names: 1–64 chars of `[a-z0-9-]`, no leading `-`.
pub fn validate_agent_id(id: &str) -> Result<()> {
    let valid = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && !id.starts_with('-')
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        anyhow::bail!("Invalid agent id '{}': use 1-{} characters of a-z, 0-9 and '-'", id, MAX_ID_LEN);
    }
    Ok(())
}

impl AgentManifest {
    /// Build and sign a manifest with the publisher's Gene key.
    pub fn new_signed(
        id: &str,
        name: &str,
        version: &str,
        description: &str,
        capabilities: Vec<String>,
        publisher: &NetGene,
        publisher_kp: &GeneKeyPair,
    ) -> Result<Self> {
        validate_agent_id(id)?;
        if publisher_kp.public_key_b64() != publisher.public_key_b64 {
            anyhow::bail!("Signing key does not belong to gene {}", publisher.id);
        }

        let mut manifest = Self {
            id: id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            description: description.to_string(),
            author: publisher.name.clone(),
            capabilities,
            publisher_gene_id: publisher.id.to_string(),
            publisher_key: HybridPublicKey::from_gene(publisher),
            signature: HybridSignature { ed25519_b64: String::new(), ml_dsa65_b64: None },
        };
        manifest.signature = publisher_kp.sign_hybrid(&manifest.signing_payload())?;
        Ok(manifest)
    }

    /// Length-prefixed encoding of every field except the key and signature
    /// (the key is bound by [`verify_hybrid`]).
    pub fn signing_payload(&self) -> Vec<u8> {
        fn push(buf: &mut Vec<u8>, field: &str) {
            buf.extend_from_slice(&(field.len() as u32).to_be_bytes());
            buf.extend_from_slice(field.as_bytes());
        }

        let mut payload = SIGNING_DOMAIN.to_vec();
        for field in [&self.id, &self.name, &self.version, &self.description, &self.author,
                      &self.publisher_gene_id] {
            push(&mut payload, field);
        }
        payload.extend_from_slice(&(self.capabilities.len() as u32).to_be_bytes());
        for cap in &self.capabilities {
            push(&mut payload, cap);
        }
        payload
    }

    /// Check the id and the publisher's hybrid signature.
    pub fn verify(&self) -> Result<()> {
        validate_agent_id(&self.id)?;
        if !verify_hybrid(&self.publisher_key, &self.signing_payload(), &self.signature) {
            anyhow::bail!("Invalid signature on agent manifest '{}'", self.id);
        }
        Ok(())
    }

    /// Short fingerprint of the publisher key, for display.
    pub fn publisher_fingerprint(&self) -> String {
        self.publisher_key.short_fingerprint()
    }
}

pub struct Marketplace {
    registry_path: PathBuf,
}

impl Marketplace {
    /// Registry at `~/.netgene/marketplace`.
    pub fn new() -> Self {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::at(home.join(".netgene").join("marketplace"))
    }

    /// Registry at an explicit directory.
    pub fn at(registry_path: PathBuf) -> Self {
        if let Err(e) = fs::create_dir_all(&registry_path) {
            warn!("Cannot create marketplace registry {:?}: {}", registry_path, e);
        }
        Self { registry_path }
    }

    /// Every manifest in the registry whose signature verifies.
    pub fn list_available_agents(&self) -> Vec<AgentManifest> {
        let mut agents = Vec::new();
        let Ok(entries) = fs::read_dir(&self.registry_path) else { return agents };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let parsed = fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|c| serde_json::from_str::<AgentManifest>(&c).map_err(anyhow::Error::from));
            match parsed.and_then(|m| m.verify().map(|_| m)) {
                Ok(manifest) => agents.push(manifest),
                Err(e) => warn!("Ignoring marketplace entry {:?}: {}", path, e),
            }
        }
        agents.sort_by(|a, b| a.id.cmp(&b.id));
        agents
    }

    pub fn install_agent(&self, agent_id: &str) -> Result<AgentManifest> {
        validate_agent_id(agent_id)?;
        self.list_available_agents()
            .into_iter()
            .find(|a| a.id == agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent {} not found in marketplace (or its signature is invalid)", agent_id))
    }

    /// Store a signed manifest given as JSON.
    pub fn publish_agent(&self, json_content: &str) -> Result<AgentManifest> {
        let manifest: AgentManifest = serde_json::from_str(json_content).context("Invalid agent manifest JSON")?;
        self.publish_manifest(&manifest)?;
        Ok(manifest)
    }

    /// Store a signed manifest.
    pub fn publish_manifest(&self, manifest: &AgentManifest) -> Result<()> {
        manifest.verify()?;
        let path = self.registry_path.join(format!("{}.json", manifest.id));
        fs::write(path, serde_json::to_string_pretty(manifest)?)?;
        Ok(())
    }
}

impl Default for Marketplace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn publisher() -> (NetGene, GeneKeyPair) {
        NetGene::generate_master("publisher").unwrap()
    }

    fn signed(id: &str) -> AgentManifest {
        let (gene, kp) = publisher();
        AgentManifest::new_signed(id, "Agent", "1.0.0", "desc", vec!["read".into()], &gene, &kp).unwrap()
    }

    #[test]
    fn test_agent_id_validation() {
        assert!(validate_agent_id("sentinel-guard-v2").is_ok());
        for bad in ["", "../evil", "a/b", "Upper", "-lead", "dot.json", &"a".repeat(65)] {
            assert!(validate_agent_id(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn test_publish_and_install_signed_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let market = Marketplace::at(dir.path().to_path_buf());
        let manifest = signed("my-agent");

        market.publish_agent(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(market.install_agent("my-agent").unwrap().name, "Agent");
    }

    #[test]
    fn test_path_traversal_id_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let market = Marketplace::at(dir.path().join("registry"));
        let mut manifest = signed("ok");
        manifest.id = "../escape".to_string();

        assert!(market.publish_agent(&serde_json::to_string(&manifest).unwrap()).is_err());
        assert!(!dir.path().join("escape.json").exists());
    }

    #[test]
    fn test_tampered_or_unsigned_manifest_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let market = Marketplace::at(dir.path().to_path_buf());

        let mut tampered = signed("agent-a");
        tampered.capabilities.push("network.admin".into());
        assert!(market.publish_agent(&serde_json::to_string(&tampered).unwrap()).is_err());

        let unsigned = r#"{"id":"x","name":"x","version":"1","description":"","author":"","capabilities":[]}"#;
        assert!(market.publish_agent(unsigned).is_err());
    }

    #[test]
    fn test_manifest_is_post_quantum_signed() {
        let manifest = signed("pq-agent");
        assert!(manifest.publisher_key.is_post_quantum());
        manifest.verify().unwrap();

        let mut stripped = manifest.clone();
        stripped.signature.ml_dsa65_b64 = None;
        assert!(stripped.verify().is_err());

        let mut downgraded = manifest.clone();
        downgraded.publisher_key.ml_dsa65_b64 = None;
        assert!(downgraded.verify().is_err());
    }

    #[test]
    fn test_tampered_file_on_disk_is_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let market = Marketplace::at(dir.path().to_path_buf());
        market.publish_manifest(&signed("agent-b")).unwrap();

        let path = dir.path().join("agent-b.json");
        let edited = fs::read_to_string(&path).unwrap().replace("\"read\"", "\"network.admin\"");
        fs::write(&path, edited).unwrap();

        assert!(market.list_available_agents().is_empty());
        assert!(market.install_agent("agent-b").is_err());
    }
}
