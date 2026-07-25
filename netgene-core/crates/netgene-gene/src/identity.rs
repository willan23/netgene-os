//! NetGene identity model.
//!
//! Defines the `NetGene` struct — the fundamental identity unit of the system.
//! A NetGene carries a cryptographic keypair, metadata, role, and lineage
//! (parent gene ID for hierarchy tracing).

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::{info, debug};

use crate::crypto::{fingerprint, short_fingerprint, GeneKeyPair};
use crate::error::GeneError;

/// Role of a NetGene in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneRole {
    /// Root identity — system owner, full authority.
    Master,
    /// Node identity — an autonomous network node.
    Node,
    /// Agent identity — an AI agent in the Netsphere Kernel.
    Agent,
    /// Observer — read-only monitoring access.
    Observer,
    /// Custom role with label.
    Custom(String),
}

impl std::fmt::Display for GeneRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeneRole::Master => write!(f, "MASTER"),
            GeneRole::Node => write!(f, "NODE"),
            GeneRole::Agent => write!(f, "AGENT"),
            GeneRole::Observer => write!(f, "OBSERVER"),
            GeneRole::Custom(s) => write!(f, "CUSTOM:{}", s),
        }
    }
}

/// Lifecycle status of a Gene.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeneStatus {
    Active,
    Suspended,
    Revoked,
    Pending,
}

impl std::fmt::Display for GeneStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GeneStatus::Active => write!(f, "ACTIVE"),
            GeneStatus::Suspended => write!(f, "SUSPENDED"),
            GeneStatus::Revoked => write!(f, "REVOKED"),
            GeneStatus::Pending => write!(f, "PENDING"),
        }
    }
}

/// Cryptographic key algorithm (Classical vs Post-Quantum)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CryptoAlgorithm {
    Ed25519,
    MlKem768,  // Kyber Post-Quantum KEM
    MlDsa65,   // Dilithium Post-Quantum DSA
}

impl std::fmt::Display for CryptoAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoAlgorithm::Ed25519 => write!(f, "Ed25519"),
            CryptoAlgorithm::MlKem768 => write!(f, "ML-KEM-768 (Kyber PQC)"),
            CryptoAlgorithm::MlDsa65 => write!(f, "ML-DSA-65 (Dilithium PQC)"),
        }
    }
}

/// The core identity unit of NetGene OS.
///
/// Contains all metadata and the public key. The private key is held
/// separately (in `GeneKeyPair`) and never serialized into this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetGene {
    /// Unique identifier for this gene.
    pub id: Uuid,
    /// Human-readable name.
    pub name: String,
    /// Role in the hierarchy.
    pub role: GeneRole,
    /// Lifecycle status.
    pub status: GeneStatus,
    /// Cryptographic algorithm used (Ed25519 vs Post-Quantum ML-DSA).
    pub algorithm: CryptoAlgorithm,
    /// Public key (base64-encoded Ed25519/PQC).
    pub public_key_b64: String,
    /// SHA3-256 fingerprint of the public key.
    pub fingerprint: String,
    /// Short fingerprint for display (8 bytes / 16 hex chars).
    pub short_fp: String,
    /// Optional parent gene ID (None for Master Gene).
    pub parent_id: Option<Uuid>,
    /// Timestamp of creation.
    pub created_at: DateTime<Utc>,
    /// Optional description / metadata.
    pub description: Option<String>,
    /// Capabilities granted to this gene.
    pub capabilities: Vec<String>,
}

impl NetGene {
    /// Generate a new Master Gene (root identity).
    pub fn generate_master(name: impl Into<String>) -> Result<(Self, GeneKeyPair)> {
        let name = name.into();
        let kp = GeneKeyPair::generate()?;
        let pk_b64 = kp.public_key_b64();
        let fp = fingerprint(kp.public_key_bytes());
        let sfp = short_fingerprint(kp.public_key_bytes());

        let gene = NetGene {
            id: Uuid::new_v4(),
            name: name.clone(),
            role: GeneRole::Master,
            status: GeneStatus::Active,
            algorithm: CryptoAlgorithm::Ed25519,
            public_key_b64: pk_b64,
            fingerprint: fp.clone(),
            short_fp: sfp.clone(),
            parent_id: None,
            created_at: Utc::now(),
            description: Some(format!("Master Gene for {}", name)),
            capabilities: vec![
                "gene.create".to_string(),
                "gene.revoke".to_string(),
                "node.spawn".to_string(),
                "agent.spawn".to_string(),
                "network.admin".to_string(),
                "quantum.run".to_string(),
            ],
        };

        info!(
            gene_id = %gene.id,
            name = %name,
            fp = %sfp,
            "Master Gene generated"
        );

        Ok((gene, kp))
    }

    /// Spawn a Sub-Gene (delegated identity) signed by the parent.
    pub fn spawn_sub_gene(
        parent: &NetGene,
        parent_kp: &GeneKeyPair,
        name: impl Into<String>,
        role: GeneRole,
        capabilities: Vec<String>,
    ) -> Result<(Self, GeneKeyPair)> {
        if parent.status != GeneStatus::Active {
            return Err(GeneError::ParentNotActive.into());
        }

        let name = name.into();
        let kp = GeneKeyPair::generate()?;
        let pk_b64 = kp.public_key_b64();
        let fp = fingerprint(kp.public_key_bytes());
        let sfp = short_fingerprint(kp.public_key_bytes());

        // Sign the new gene's public key with parent key to establish lineage
        let _lineage_sig = parent_kp.sign(kp.public_key_bytes())?;

        let gene = NetGene {
            id: Uuid::new_v4(),
            name: name.clone(),
            role: role.clone(),
            status: GeneStatus::Active,
            algorithm: CryptoAlgorithm::Ed25519,
            public_key_b64: pk_b64,
            fingerprint: fp,
            short_fp: sfp.clone(),
            parent_id: Some(parent.id),
            created_at: Utc::now(),
            description: None,
            capabilities,
        };

        debug!(
            sub_gene_id = %gene.id,
            parent_id = %parent.id,
            role = %role,
            name = %name,
            fp = %sfp,
            "Sub-Gene spawned"
        );

        Ok((gene, kp))
    }

    /// Suspend this gene (reversible).
    pub fn suspend(&mut self) {
        self.status = GeneStatus::Suspended;
        info!(gene_id = %self.id, "Gene suspended");
    }

    /// Revoke this gene (irreversible in this implementation).
    pub fn revoke(&mut self) {
        self.status = GeneStatus::Revoked;
        info!(gene_id = %self.id, "Gene REVOKED");
    }

    /// Check if this gene has a specific capability.
    pub fn has_capability(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    /// Returns true if this gene is authorized (active + has capability).
    pub fn is_authorized(&self, cap: &str) -> bool {
        self.status == GeneStatus::Active && self.has_capability(cap)
    }

    /// Display line for terminal output.
    pub fn display_line(&self) -> String {
        format!(
            "[{}] {} | {} | {} | fp:{}",
            self.role, self.name, self.id, self.status, self.short_fp
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_master_gene_generation() {
        let (gene, _kp) = NetGene::generate_master("Test Master").unwrap();
        assert_eq!(gene.role, GeneRole::Master);
        assert_eq!(gene.status, GeneStatus::Active);
        assert!(gene.parent_id.is_none());
        assert!(gene.has_capability("gene.create"));
    }

    #[test]
    fn test_spawn_sub_gene() {
        let (master, master_kp) = NetGene::generate_master("Master").unwrap();
        let (sub, _kp) = NetGene::spawn_sub_gene(
            &master,
            &master_kp,
            "Node-01",
            GeneRole::Node,
            vec!["node.spawn".to_string()],
        )
        .unwrap();
        assert_eq!(sub.parent_id, Some(master.id));
        assert_eq!(sub.role, GeneRole::Node);
    }

    #[test]
    fn test_revoke() {
        let (mut gene, _kp) = NetGene::generate_master("ToRevoke").unwrap();
        gene.revoke();
        assert_eq!(gene.status, GeneStatus::Revoked);
        assert!(!gene.is_authorized("network.admin"));
    }
}
