//! Cryptographic primitives for the Gene Layer.
//!
//! Wraps `ring` for Ed25519 key generation/signing/verification
//! and `sha3` for fingerprint computation.

use ring::{
    rand::SystemRandom,
    signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519},
};
use sha3::{Digest, Sha3_256};
use zeroize::Zeroizing;
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::pq::PqKeyPair;

/// A Gene key pair: Ed25519, plus an optional ML-DSA-65 key for hybrid
/// (post-quantum) signatures.
pub struct GeneKeyPair {
    /// PKCS#8 encoded private key bytes (zeroized on drop)
    pkcs8: Zeroizing<Vec<u8>>,
    /// Public key bytes
    pub_key: Vec<u8>,
    /// Post-quantum key (absent on genes created before v0.5).
    pq: Option<PqKeyPair>,
}

impl GeneKeyPair {
    /// Generate a new random hybrid (Ed25519 + ML-DSA-65) keypair.
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|_| anyhow::anyhow!("Key generation failed"))?;

        let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
            .map_err(|_| anyhow::anyhow!("PKCS8 parse failed"))?;

        let pub_key = pair.public_key().as_ref().to_vec();

        Ok(Self {
            pkcs8: Zeroizing::new(pkcs8.as_ref().to_vec()),
            pub_key,
            pq: Some(PqKeyPair::generate()?),
        })
    }

    /// Load from existing PKCS#8 bytes (Ed25519 only; see [`Self::with_pq_seed`]).
    pub fn from_pkcs8(bytes: Vec<u8>) -> Result<Self> {
        let pair = Ed25519KeyPair::from_pkcs8(&bytes)
            .map_err(|_| anyhow::anyhow!("Invalid PKCS8 key"))?;
        let pub_key = pair.public_key().as_ref().to_vec();
        Ok(Self {
            pkcs8: Zeroizing::new(bytes),
            pub_key,
            pq: None,
        })
    }

    /// Attach the ML-DSA-65 key derived from `seed`.
    pub fn with_pq_seed(mut self, seed: &[u8]) -> Result<Self> {
        self.pq = Some(PqKeyPair::from_seed(seed)?);
        Ok(self)
    }

    /// The post-quantum half, if any.
    pub fn pq(&self) -> Option<&PqKeyPair> {
        self.pq.as_ref()
    }

    /// Base64 ML-DSA-65 public key, if any.
    pub fn pq_public_key_b64(&self) -> Option<String> {
        self.pq.as_ref().map(|pq| BASE64.encode(pq.public_key_bytes()))
    }

    /// Sign a message, returning signature bytes.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>> {
        let pair = Ed25519KeyPair::from_pkcs8(&self.pkcs8)
            .map_err(|_| anyhow::anyhow!("Sign: failed to load keypair"))?;
        Ok(pair.sign(message).as_ref().to_vec())
    }

    /// Get public key bytes.
    pub fn public_key_bytes(&self) -> &[u8] {
        &self.pub_key
    }

    /// Get the PKCS8 private key bytes (handle with care!).
    pub fn pkcs8_bytes(&self) -> &[u8] {
        &self.pkcs8
    }

    /// The raw 32-byte Ed25519 seed, e.g. to derive the node's libp2p identity
    /// from its Gene. Only supports the PKCS#8 v2 layout produced by `ring`.
    pub fn ed25519_seed(&self) -> Result<Zeroizing<[u8; 32]>> {
        // PKCS#8 v2 (RFC 8410) prefix: SEQUENCE, version 1, AlgorithmIdentifier(Ed25519), OCTET STRING(OCTET STRING(32)).
        const PREFIX: [u8; 16] = [0x30, 0x51, 0x02, 0x01, 0x01, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];
        if self.pkcs8.len() < 48 || self.pkcs8[..16] != PREFIX {
            anyhow::bail!("Unsupported Ed25519 PKCS#8 layout");
        }
        let mut seed = Zeroizing::new([0u8; 32]);
        seed.copy_from_slice(&self.pkcs8[16..48]);
        Ok(seed)
    }

    /// Base64-encode the public key for display/storage.
    pub fn public_key_b64(&self) -> String {
        BASE64.encode(&self.pub_key)
    }

    /// Base64-encode the PKCS8 private key for storage.
    pub fn pkcs8_b64(&self) -> String {
        BASE64.encode(&self.pkcs8 as &[u8])
    }
}

/// Verify an Ed25519 signature.
pub fn verify_signature(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let pk = UnparsedPublicKey::new(&ED25519, public_key);
    pk.verify(message, signature).is_ok()
}

/// Compute SHA3-256 fingerprint of data, returned as hex string.
pub fn fingerprint(data: &[u8]) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Short fingerprint (first 16 hex chars = 8 bytes).
pub fn short_fingerprint(data: &[u8]) -> String {
    fingerprint(data)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keygen_and_sign() {
        let kp = GeneKeyPair::generate().unwrap();
        let msg = b"netgene-test-message";
        let sig = kp.sign(msg).unwrap();
        assert!(verify_signature(kp.public_key_bytes(), msg, &sig));
        // Tampered message should fail
        assert!(!verify_signature(kp.public_key_bytes(), b"tampered", &sig));
    }

    #[test]
    fn test_ed25519_seed_roundtrip() {
        let kp = GeneKeyPair::generate().unwrap();
        let seed = kp.ed25519_seed().unwrap();
        let rebuilt = Ed25519KeyPair::from_seed_unchecked(seed.as_ref()).unwrap();
        assert_eq!(rebuilt.public_key().as_ref(), kp.public_key_bytes());
    }

    #[test]
    fn test_fingerprint() {
        let fp = fingerprint(b"netgene");
        assert_eq!(fp.len(), 64); // SHA3-256 = 32 bytes = 64 hex chars
    }
}
