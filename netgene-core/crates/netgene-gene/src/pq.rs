//! Post-quantum signatures (ML-DSA-65, FIPS 204) used alongside Ed25519.
//!
//! NetGene signatures are *hybrid*: where both keys exist, both signatures
//! must verify. Breaking one algorithm (e.g. Ed25519 by a future quantum
//! computer, or an ML-DSA implementation flaw) is then not enough to forge.
//!
//! Keys are derived from a 32-byte seed, which is what goes into the
//! secret store.

use anyhow::Result;
use ml_dsa::{EncodedVerifyingKey, Keypair, MlDsa65, Seed, Signature, Signer, SigningKey, Verifier, VerifyingKey};
use ring::rand::{SecureRandom, SystemRandom};
use zeroize::Zeroizing;

/// An ML-DSA-65 key pair.
pub struct PqKeyPair {
    seed: Zeroizing<[u8; 32]>,
    signing_key: SigningKey<MlDsa65>,
}

impl PqKeyPair {
    pub fn generate() -> Result<Self> {
        let mut seed = Zeroizing::new([0u8; 32]);
        SystemRandom::new()
            .fill(seed.as_mut())
            .map_err(|_| anyhow::anyhow!("Failed to generate ML-DSA seed"))?;
        Self::from_seed(seed.as_ref())
    }

    pub fn from_seed(bytes: &[u8]) -> Result<Self> {
        let raw: [u8; 32] = bytes.try_into().map_err(|_| anyhow::anyhow!("ML-DSA seed must be 32 bytes"))?;
        let seed = Zeroizing::new(raw);
        let signing_key = SigningKey::<MlDsa65>::from_seed(&Seed::from(raw));
        Ok(Self { seed, signing_key })
    }

    /// Secret seed (store it in a `SecretStore`, never on disk).
    pub fn seed_bytes(&self) -> &[u8] {
        self.seed.as_ref()
    }

    pub fn public_key_bytes(&self) -> Vec<u8> {
        self.signing_key.verifying_key().encode().to_vec()
    }

    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let signature: Signature<MlDsa65> = self.signing_key.sign(message);
        signature.encode().to_vec()
    }
}

/// Verify an ML-DSA-65 signature.
pub fn verify_pq(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let Ok(encoded) = EncodedVerifyingKey::<MlDsa65>::try_from(public_key) else { return false };
    let verifying_key = VerifyingKey::<MlDsa65>::decode(&encoded);
    let Ok(signature) = Signature::<MlDsa65>::try_from(signature) else { return false };
    verifying_key.verify(message, &signature).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let kp = PqKeyPair::generate().unwrap();
        let sig = kp.sign(b"netgene");
        assert!(verify_pq(&kp.public_key_bytes(), b"netgene", &sig));
        assert!(!verify_pq(&kp.public_key_bytes(), b"tampered", &sig));
    }

    #[test]
    fn test_seed_roundtrip_gives_same_key() {
        let kp = PqKeyPair::generate().unwrap();
        let again = PqKeyPair::from_seed(kp.seed_bytes()).unwrap();
        assert_eq!(kp.public_key_bytes(), again.public_key_bytes());
    }

    #[test]
    fn test_wrong_key_rejected() {
        let a = PqKeyPair::generate().unwrap();
        let b = PqKeyPair::generate().unwrap();
        assert!(!verify_pq(&b.public_key_bytes(), b"m", &a.sign(b"m")));
        assert!(!verify_pq(b"short", b"m", &a.sign(b"m")));
    }
}
