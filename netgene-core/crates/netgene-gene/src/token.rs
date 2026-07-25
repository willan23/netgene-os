//! Signed Gene Tokens for authorization.
//!
//! A `GeneToken` is a JWT-like signed proof that a Gene is authorized
//! to perform a specific capability at a given time.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use anyhow::Result;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::crypto::{GeneKeyPair, verify_signature};

/// Claims embedded in a GeneToken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Subject gene ID.
    pub sub: Uuid,
    /// Capability being claimed.
    pub cap: String,
    /// Issued at (Unix timestamp).
    pub iat: i64,
    /// Expiry (Unix timestamp).
    pub exp: i64,
    /// Unique token ID.
    pub jti: Uuid,
}

/// A signed authorization token produced by a NetGene.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneToken {
    /// Base64-encoded JSON claims.
    pub claims_b64: String,
    /// Base64-encoded Ed25519 signature of claims_b64 bytes.
    pub signature_b64: String,
    /// Public key of the issuing gene (base64).
    pub issuer_pubkey_b64: String,
}

impl GeneToken {
    /// Issue a signed token for `capability`, valid for `valid_secs` seconds.
    pub fn issue(
        gene_id: Uuid,
        capability: &str,
        valid_secs: i64,
        keypair: &GeneKeyPair,
    ) -> Result<Self> {
        let now = Utc::now();
        let claims = TokenClaims {
            sub: gene_id,
            cap: capability.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::seconds(valid_secs)).timestamp(),
            jti: Uuid::new_v4(),
        };

        let claims_json = serde_json::to_string(&claims)?;
        let claims_b64 = BASE64.encode(claims_json.as_bytes());
        let signature = keypair.sign(claims_b64.as_bytes())?;
        let signature_b64 = BASE64.encode(&signature);

        Ok(Self {
            claims_b64,
            signature_b64,
            issuer_pubkey_b64: keypair.public_key_b64(),
        })
    }

    /// Verify the token signature and check expiry.
    pub fn verify(&self) -> Result<TokenClaims> {
        let pubkey = BASE64.decode(&self.issuer_pubkey_b64)?;
        let sig = BASE64.decode(&self.signature_b64)?;

        if !verify_signature(&pubkey, self.claims_b64.as_bytes(), &sig) {
            return Err(anyhow::anyhow!("Token signature invalid"));
        }

        let claims_json = BASE64.decode(&self.claims_b64)?;
        let claims: TokenClaims = serde_json::from_slice(&claims_json)?;

        let now = Utc::now().timestamp();
        if claims.exp < now {
            return Err(anyhow::anyhow!("Token expired"));
        }

        Ok(claims)
    }

    /// Get token fingerprint for logging.
    pub fn fingerprint(&self) -> String {
        let data = format!("{}{}", self.claims_b64, self.signature_b64);
        crate::crypto::short_fingerprint(data.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::GeneKeyPair;

    #[test]
    fn test_issue_and_verify() {
        let kp = GeneKeyPair::generate().unwrap();
        let gene_id = Uuid::new_v4();
        let token = GeneToken::issue(gene_id, "node.spawn", 3600, &kp).unwrap();
        let claims = token.verify().unwrap();
        assert_eq!(claims.sub, gene_id);
        assert_eq!(claims.cap, "node.spawn");
    }

    #[test]
    fn test_expired_token_fails() {
        let kp = GeneKeyPair::generate().unwrap();
        let gene_id = Uuid::new_v4();
        // Issue token with -1 second validity (already expired)
        let token = GeneToken::issue(gene_id, "network.admin", -1, &kp).unwrap();
        assert!(token.verify().is_err());
    }
}
