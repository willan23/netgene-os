//! Capability tokens: time-limited, revocable delegation of capabilities.
//!
//! An issuer Gene grants a subject Gene a set of capabilities for a time
//! window. Tokens are hybrid-signed (Ed25519 + ML-DSA-65). An issuer can
//! only delegate capabilities it holds itself (a Master Gene holds all), so
//! tokens cannot escalate privileges.

use std::collections::BTreeSet;
use std::path::Path;
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::GeneKeyPair;
use crate::hybrid::{verify_hybrid, HybridPublicKey, HybridSignature};
use crate::identity::{GeneRole, GeneStatus, NetGene};

const SIGNING_DOMAIN: &[u8] = b"netgene-capability-token-v1\0";

/// What a token grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityClaims {
    pub token_id: Uuid,
    pub issuer_gene_id: Uuid,
    pub subject_gene_id: Uuid,
    pub capabilities: Vec<String>,
    pub not_before: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// A signed capability grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub claims: CapabilityClaims,
    pub issuer_key: HybridPublicKey,
    pub signature: HybridSignature,
}

/// Holds capability `cap`? A Master Gene holds every capability.
fn holds(gene: &NetGene, cap: &str) -> bool {
    gene.role == GeneRole::Master || gene.has_capability(cap)
}

impl CapabilityToken {
    /// Grant `capabilities` to `subject` for `ttl`, signed by `issuer`.
    pub fn issue(
        issuer: &NetGene,
        issuer_kp: &GeneKeyPair,
        subject: &NetGene,
        capabilities: Vec<String>,
        ttl: Duration,
    ) -> Result<Self> {
        if issuer.status != GeneStatus::Active {
            anyhow::bail!("Issuer gene {} is {}", issuer.id, issuer.status);
        }
        if issuer_kp.public_key_b64() != issuer.public_key_b64 {
            anyhow::bail!("Signing key does not belong to gene {}", issuer.id);
        }
        if capabilities.is_empty() || ttl <= Duration::zero() {
            anyhow::bail!("A token needs at least one capability and a positive lifetime");
        }
        if let Some(cap) = capabilities.iter().find(|c| !holds(issuer, c)) {
            anyhow::bail!("Issuer cannot delegate '{}', which it does not hold", cap);
        }

        let now = Utc::now();
        let claims = CapabilityClaims {
            token_id: Uuid::new_v4(),
            issuer_gene_id: issuer.id,
            subject_gene_id: subject.id,
            capabilities,
            not_before: now,
            expires_at: now + ttl,
        };
        let signature = issuer_kp.sign_hybrid(&Self::signing_payload(&claims)?)?;
        Ok(Self { claims, issuer_key: HybridPublicKey::from_gene(issuer), signature })
    }

    fn signing_payload(claims: &CapabilityClaims) -> Result<Vec<u8>> {
        let mut payload = SIGNING_DOMAIN.to_vec();
        payload.extend_from_slice(&serde_json::to_vec(claims)?);
        Ok(payload)
    }

    /// Check the token for `subject`, given the issuing Gene (looked up by the
    /// caller from its trusted store), the revocation list and the time.
    pub fn verify(&self, issuer: &NetGene, subject: &NetGene, revoked: &RevocationList, now: DateTime<Utc>) -> Result<&CapabilityClaims> {
        let c = &self.claims;
        if c.issuer_gene_id != issuer.id || self.issuer_key != HybridPublicKey::from_gene(issuer) {
            anyhow::bail!("Token {} was not issued by gene {}", c.token_id, issuer.id);
        }
        if c.subject_gene_id != subject.id {
            anyhow::bail!("Token {} belongs to another gene", c.token_id);
        }
        if !verify_hybrid(&self.issuer_key, &Self::signing_payload(c)?, &self.signature) {
            anyhow::bail!("Token {} has an invalid signature", c.token_id);
        }
        if revoked.contains(c.token_id) {
            anyhow::bail!("Token {} was revoked", c.token_id);
        }
        if now < c.not_before || now >= c.expires_at {
            anyhow::bail!("Token {} is outside its validity window", c.token_id);
        }
        if issuer.status != GeneStatus::Active || subject.status != GeneStatus::Active {
            anyhow::bail!("Issuer or subject of token {} is not active", c.token_id);
        }
        if let Some(cap) = c.capabilities.iter().find(|cap| !holds(issuer, cap)) {
            anyhow::bail!("Issuer no longer holds '{}'", cap);
        }
        Ok(c)
    }
}

/// Revoked token ids (stored as JSON, e.g. `~/.netgene/revoked-tokens.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RevocationList {
    revoked: BTreeSet<Uuid>,
}

impl RevocationList {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        serde_json::from_str(&std::fs::read_to_string(path)?)
            .with_context(|| format!("Invalid revocation list {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn revoke(&mut self, token_id: Uuid) -> bool {
        self.revoked.insert(token_id)
    }

    pub fn contains(&self, token_id: Uuid) -> bool {
        self.revoked.contains(&token_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (NetGene, GeneKeyPair, NetGene) {
        let (master, kp) = NetGene::generate_master("root").unwrap();
        let (agent, _) = NetGene::spawn_sub_gene(&master, &kp, "agent", GeneRole::Agent, vec!["network.read".into()]).unwrap();
        (master, kp, agent)
    }

    #[test]
    fn test_issue_and_verify() {
        let (master, kp, agent) = setup();
        let token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();
        assert!(token.issuer_key.is_post_quantum());
        let claims = token.verify(&master, &agent, &RevocationList::default(), Utc::now()).unwrap();
        assert_eq!(claims.capabilities, vec!["node.spawn".to_string()]);
    }

    #[test]
    fn test_expired_and_not_yet_valid() {
        let (master, kp, agent) = setup();
        let token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::minutes(5)).unwrap();
        let revoked = RevocationList::default();
        assert!(token.verify(&master, &agent, &revoked, Utc::now() + Duration::minutes(6)).is_err());
        assert!(token.verify(&master, &agent, &revoked, Utc::now() - Duration::minutes(1)).is_err());
    }

    #[test]
    fn test_revoked_token_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("revoked.json");
        let (master, kp, agent) = setup();
        let token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();

        let mut list = RevocationList::load(&path).unwrap();
        assert!(list.revoke(token.claims.token_id));
        list.save(&path).unwrap();

        assert!(token.verify(&master, &agent, &RevocationList::load(&path).unwrap(), Utc::now()).is_err());
    }

    #[test]
    fn test_tampered_claims_rejected() {
        let (master, kp, agent) = setup();
        let mut token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();
        token.claims.capabilities.push("network.admin".into());
        assert!(token.verify(&master, &agent, &RevocationList::default(), Utc::now()).is_err());

        let mut extended = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();
        extended.claims.expires_at = extended.claims.expires_at + Duration::days(365);
        assert!(extended.verify(&master, &agent, &RevocationList::default(), Utc::now()).is_err());
    }

    #[test]
    fn test_wrong_subject_or_issuer_rejected() {
        let (master, kp, agent) = setup();
        let (other_master, _) = NetGene::generate_master("other").unwrap();
        let token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();
        let revoked = RevocationList::default();
        assert!(token.verify(&other_master, &agent, &revoked, Utc::now()).is_err());
        assert!(token.verify(&master, &master, &revoked, Utc::now()).is_err());
    }

    #[test]
    fn test_no_privilege_escalation() {
        let (master, kp, _) = setup();
        let (agent, agent_kp) = NetGene::spawn_sub_gene(&master, &kp, "a", GeneRole::Agent, vec!["network.read".into()]).unwrap();
        let (peer, _) = NetGene::spawn_sub_gene(&master, &kp, "b", GeneRole::Agent, vec![]).unwrap();

        assert!(CapabilityToken::issue(&agent, &agent_kp, &peer, vec!["network.read".into()], Duration::hours(1)).is_ok());
        assert!(CapabilityToken::issue(&agent, &agent_kp, &peer, vec!["network.admin".into()], Duration::hours(1)).is_err());
    }

    #[test]
    fn test_revoked_issuer_invalidates_tokens() {
        let (mut master, kp, agent) = setup();
        let token = CapabilityToken::issue(&master, &kp, &agent, vec!["node.spawn".into()], Duration::hours(1)).unwrap();
        master.revoke();
        assert!(token.verify(&master, &agent, &RevocationList::default(), Utc::now()).is_err());
    }
}
