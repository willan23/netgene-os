//! Hybrid (Ed25519 + ML-DSA-65) signatures for NetGene artifacts.
//!
//! The signer's [`HybridPublicKey`] is bound into the signed bytes. Removing
//! the post-quantum half of the key (to downgrade to Ed25519-only) changes
//! those bytes, so the Ed25519 signature stops verifying too. Where the key
//! has an ML-DSA part, the ML-DSA signature is mandatory.

use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};

use crate::crypto::{short_fingerprint, verify_signature, GeneKeyPair};
use crate::identity::NetGene;
use crate::pq::verify_pq;

const BINDING_DOMAIN: &[u8] = b"netgene-hybrid-signature-v1\0";

/// Public half of a Gene key: Ed25519 plus optional ML-DSA-65.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridPublicKey {
    pub ed25519_b64: String,
    #[serde(default)]
    pub ml_dsa65_b64: Option<String>,
}

impl HybridPublicKey {
    pub fn from_gene(gene: &NetGene) -> Self {
        Self { ed25519_b64: gene.public_key_b64.clone(), ml_dsa65_b64: gene.pq_public_key_b64.clone() }
    }

    pub fn from_keypair(kp: &GeneKeyPair) -> Self {
        Self { ed25519_b64: kp.public_key_b64(), ml_dsa65_b64: kp.pq_public_key_b64() }
    }

    pub fn is_post_quantum(&self) -> bool {
        self.ml_dsa65_b64.is_some()
    }

    /// Short fingerprint of the Ed25519 half (matches `NetGene::short_fp`).
    pub fn short_fingerprint(&self) -> String {
        BASE64.decode(&self.ed25519_b64).map(|k| short_fingerprint(&k)).unwrap_or_default()
    }

    /// Canonical, length-prefixed encoding used inside signed payloads.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let ed = BASE64.decode(&self.ed25519_b64)?;
        out.extend_from_slice(&(ed.len() as u32).to_be_bytes());
        out.extend_from_slice(&ed);
        match &self.ml_dsa65_b64 {
            Some(pq) => {
                let pq = BASE64.decode(pq)?;
                out.extend_from_slice(&(pq.len() as u32).to_be_bytes());
                out.extend_from_slice(&pq);
            }
            None => out.extend_from_slice(&0u32.to_be_bytes()),
        }
        Ok(out)
    }

    fn bind(&self, message: &[u8]) -> Result<Vec<u8>> {
        let mut bound = BINDING_DOMAIN.to_vec();
        bound.extend_from_slice(&self.encode()?);
        bound.extend_from_slice(message);
        Ok(bound)
    }
}

/// Signature produced by [`GeneKeyPair::sign_hybrid`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HybridSignature {
    pub ed25519_b64: String,
    #[serde(default)]
    pub ml_dsa65_b64: Option<String>,
}

impl GeneKeyPair {
    /// Sign `message` with every key this pair holds.
    pub fn sign_hybrid(&self, message: &[u8]) -> Result<HybridSignature> {
        let bound = HybridPublicKey::from_keypair(self).bind(message)?;
        Ok(HybridSignature {
            ed25519_b64: BASE64.encode(self.sign(&bound)?),
            ml_dsa65_b64: self.pq().map(|pq| BASE64.encode(pq.sign(&bound))),
        })
    }

    pub fn hybrid_public_key(&self) -> HybridPublicKey {
        HybridPublicKey::from_keypair(self)
    }
}

/// Verify a hybrid signature: Ed25519 always, ML-DSA-65 whenever the key has one.
pub fn verify_hybrid(key: &HybridPublicKey, message: &[u8], signature: &HybridSignature) -> bool {
    let Ok(bound) = key.bind(message) else { return false };
    let (Ok(ed_pk), Ok(ed_sig)) = (BASE64.decode(&key.ed25519_b64), BASE64.decode(&signature.ed25519_b64)) else {
        return false;
    };
    if !verify_signature(&ed_pk, &bound, &ed_sig) {
        return false;
    }
    match &key.ml_dsa65_b64 {
        None => true,
        Some(pq_pk_b64) => {
            let Some(pq_sig_b64) = &signature.ml_dsa65_b64 else { return false };
            match (BASE64.decode(pq_pk_b64), BASE64.decode(pq_sig_b64)) {
                (Ok(pq_pk), Ok(pq_sig)) => verify_pq(&pq_pk, &bound, &pq_sig),
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hybrid_roundtrip() {
        let kp = GeneKeyPair::generate().unwrap();
        let key = kp.hybrid_public_key();
        assert!(key.is_post_quantum());
        let sig = kp.sign_hybrid(b"payload").unwrap();
        assert!(verify_hybrid(&key, b"payload", &sig));
        assert!(!verify_hybrid(&key, b"other", &sig));
    }

    #[test]
    fn test_missing_pq_signature_rejected() {
        let kp = GeneKeyPair::generate().unwrap();
        let mut sig = kp.sign_hybrid(b"m").unwrap();
        sig.ml_dsa65_b64 = None;
        assert!(!verify_hybrid(&kp.hybrid_public_key(), b"m", &sig));
    }

    #[test]
    fn test_stripping_pq_key_breaks_ed25519_too() {
        let kp = GeneKeyPair::generate().unwrap();
        let sig = kp.sign_hybrid(b"m").unwrap();
        let mut downgraded = kp.hybrid_public_key();
        downgraded.ml_dsa65_b64 = None;
        assert!(!verify_hybrid(&downgraded, b"m", &sig), "Ed25519 signature is bound to the full key");
    }

    #[test]
    fn test_ed25519_only_keys_still_work() {
        let kp = GeneKeyPair::generate().unwrap();
        let ed_only = GeneKeyPair::from_pkcs8(kp.pkcs8_bytes().to_vec()).unwrap();
        let key = ed_only.hybrid_public_key();
        assert!(!key.is_post_quantum());
        assert!(verify_hybrid(&key, b"m", &ed_only.sign_hybrid(b"m").unwrap()));
    }
}
