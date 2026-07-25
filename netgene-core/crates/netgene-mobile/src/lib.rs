//! # NetGene Mobile Bridge & Passkey Engine (`netgene-mobile`)

pub mod passkey;
pub mod bridge;

pub use passkey::{PasskeyEngine, PasskeyChallenge, PasskeyAuthResponse};
pub use bridge::{MobileLiveBridge, MobileBridgeSession};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passkey_challenge_generation() {
        let engine = PasskeyEngine::new("netgene.io");
        let challenge = engine.create_challenge("master-gene-01");
        assert_eq!(challenge.rp_id, "netgene.io");
        assert!(!challenge.challenge_bytes_b64.is_empty());
    }
}
