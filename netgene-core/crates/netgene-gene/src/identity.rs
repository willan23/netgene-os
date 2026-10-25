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

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::crypto::{fingerprint, short_fingerprint, verify_signature, GeneKeyPair};
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
    /// Ed25519 and ML-DSA-65 together; both signatures must verify.
    HybridEd25519MlDsa65,
}

impl std::fmt::Display for CryptoAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoAlgorithm::Ed25519 => write!(f, "Ed25519"),
            CryptoAlgorithm::MlKem768 => write!(f, "ML-KEM-768 (Kyber PQC)"),
            CryptoAlgorithm::MlDsa65 => write!(f, "ML-DSA-65 (Dilithium PQC)"),
            CryptoAlgorithm::HybridEd25519MlDsa65 => write!(f, "Hybrid Ed25519 + ML-DSA-65 (post-quantum)"),
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
    /// Parent's Ed25519 signature over this gene's public key (None for a Master Gene).
    #[serde(default)]
    pub lineage_signature_b64: Option<String>,
    /// ML-DSA-65 public key (hybrid genes).
    #[serde(default)]
    pub pq_public_key_b64: Option<String>,
    /// Parent's ML-DSA-65 signature over [`NetGene::pq_lineage_payload`].
    #[serde(default)]
    pub pq_lineage_signature_b64: Option<String>,
}

const PQ_LINEAGE_DOMAIN: &[u8] = b"netgene-lineage-mldsa65-v1\0";

fn algorithm_for(kp: &GeneKeyPair) -> CryptoAlgorithm {
    if kp.pq().is_some() { CryptoAlgorithm::HybridEd25519MlDsa65 } else { CryptoAlgorithm::Ed25519 }
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
            algorithm: algorithm_for(&kp),
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
            ],
            lineage_signature_b64: None,
            pq_public_key_b64: kp.pq_public_key_b64(),
            pq_lineage_signature_b64: None,
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
        let lineage_sig = parent_kp.sign(kp.public_key_bytes())?;
        let pq_pk = kp.pq().map(|pq| pq.public_key_bytes());
        let pq_lineage_sig = parent_kp
            .pq()
            .map(|pq| pq.sign(&Self::pq_lineage_payload(kp.public_key_bytes(), pq_pk.as_deref())));

        let gene = NetGene {
            id: Uuid::new_v4(),
            name: name.clone(),
            role: role.clone(),
            status: GeneStatus::Active,
            algorithm: algorithm_for(&kp),
            public_key_b64: pk_b64,
            fingerprint: fp,
            short_fp: sfp.clone(),
            parent_id: Some(parent.id),
            created_at: Utc::now(),
            description: None,
            capabilities,
            lineage_signature_b64: Some(BASE64.encode(lineage_sig)),
            pq_public_key_b64: kp.pq_public_key_b64(),
            pq_lineage_signature_b64: pq_lineage_sig.map(|s| BASE64.encode(s)),
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

    /// Decode this gene's public key.
    pub fn public_key_bytes(&self) -> Result<Vec<u8>> {
        Ok(BASE64.decode(&self.public_key_b64)?)
    }

    /// Decode this gene's ML-DSA-65 public key, if it has one.
    pub fn pq_public_key_bytes(&self) -> Result<Option<Vec<u8>>> {
        self.pq_public_key_b64.as_ref().map(|k| Ok(BASE64.decode(k)?)).transpose()
    }

    /// Bytes the parent's ML-DSA key signs: both of the child's public keys.
    pub fn pq_lineage_payload(child_ed_pk: &[u8], child_pq_pk: Option<&[u8]>) -> Vec<u8> {
        let mut payload = PQ_LINEAGE_DOMAIN.to_vec();
        payload.extend_from_slice(&(child_ed_pk.len() as u32).to_be_bytes());
        payload.extend_from_slice(child_ed_pk);
        if let Some(pq) = child_pq_pk {
            payload.extend_from_slice(&(pq.len() as u32).to_be_bytes());
            payload.extend_from_slice(pq);
        }
        payload
    }

    /// True if `parent` signed this gene (verifiable lineage).
    ///
    /// The Ed25519 lineage signature must always verify. If the parent has an
    /// ML-DSA key, the post-quantum lineage signature (covering both of this
    /// gene's keys) must verify too, so stripping it is not a downgrade path.
    pub fn verify_lineage(&self, parent: &NetGene) -> bool {
        if self.parent_id != Some(parent.id) {
            return false;
        }
        let (Some(sig_b64), Ok(own_pk), Ok(parent_pk)) =
            (&self.lineage_signature_b64, self.public_key_bytes(), parent.public_key_bytes())
        else {
            return false;
        };
        let ed_ok = BASE64.decode(sig_b64).map(|sig| verify_signature(&parent_pk, &own_pk, &sig)).unwrap_or(false);
        if !ed_ok {
            return false;
        }

        match parent.pq_public_key_bytes() {
            Ok(None) => true,
            Ok(Some(parent_pq)) => {
                let (Some(pq_sig_b64), Ok(own_pq)) = (&self.pq_lineage_signature_b64, self.pq_public_key_bytes()) else {
                    return false;
                };
                let payload = Self::pq_lineage_payload(&own_pk, own_pq.as_deref());
                BASE64
                    .decode(pq_sig_b64)
                    .map(|sig| crate::pq::verify_pq(&parent_pq, &payload, &sig))
                    .unwrap_or(false)
            }
            Err(_) => false,
        }
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
        assert_eq!(sub.algorithm, CryptoAlgorithm::HybridEd25519MlDsa65);
        assert!(sub.pq_public_key_b64.is_some());
        assert!(sub.verify_lineage(&master));
    }

    #[test]
    fn test_pq_lineage_cannot_be_stripped_or_swapped() {
        let (master, master_kp) = NetGene::generate_master("Master").unwrap();
        let (sub, _) = NetGene::spawn_sub_gene(&master, &master_kp, "Node", GeneRole::Node, vec![]).unwrap();
        let (other, _) = NetGene::spawn_sub_gene(&master, &master_kp, "Other", GeneRole::Node, vec![]).unwrap();

        let mut stripped = sub.clone();
        stripped.pq_lineage_signature_b64 = None;
        assert!(!stripped.verify_lineage(&master), "downgrade to Ed25519-only must fail");

        let mut swapped = sub.clone();
        swapped.pq_public_key_b64 = other.pq_public_key_b64.clone();
        assert!(!swapped.verify_lineage(&master), "replacing the PQ key must fail");
    }

    #[test]
    fn test_legacy_ed25519_parent_still_verifies() {
        let (mut master, master_kp) = NetGene::generate_master("Legacy").unwrap();
        let ed_only = GeneKeyPair::from_pkcs8(master_kp.pkcs8_bytes().to_vec()).unwrap();
        master.pq_public_key_b64 = None;
        let (sub, _) = NetGene::spawn_sub_gene(&master, &ed_only, "Node", GeneRole::Node, vec![]).unwrap();
        assert!(sub.pq_lineage_signature_b64.is_none());
        assert!(sub.verify_lineage(&master));
    }

    #[test]
    fn test_lineage_rejects_wrong_parent() {
        let (master, master_kp) = NetGene::generate_master("Master").unwrap();
        let (impostor, _) = NetGene::generate_master("Impostor").unwrap();
        let (sub, _) = NetGene::spawn_sub_gene(&master, &master_kp, "Node", GeneRole::Node, vec![]).unwrap();

        let mut forged = sub.clone();
        forged.parent_id = Some(impostor.id);
        assert!(!forged.verify_lineage(&impostor));
        assert!(!master.verify_lineage(&master));
    }

    #[test]
    fn test_revoke() {
        let (mut gene, _kp) = NetGene::generate_master("ToRevoke").unwrap();
        gene.revoke();
        assert_eq!(gene.status, GeneStatus::Revoked);
        assert!(!gene.is_authorized("network.admin"));
    }
}
