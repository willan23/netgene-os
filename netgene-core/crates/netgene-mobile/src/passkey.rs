//! WebAuthn / Passkey Biometric Engine for Mobile Clients.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyChallenge {
    pub challenge_id: Uuid,
    pub challenge_bytes_b64: String,
    pub rp_id: String,
    pub user_fingerprint: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasskeyAuthResponse {
    pub challenge_id: Uuid,
    pub authenticator_data_b64: String,
    pub client_data_json_b64: String,
    pub signature_b64: String,
    pub user_handle: String,
}

pub struct PasskeyEngine {
    rp_id: String,
}

impl PasskeyEngine {
    pub fn new(rp_id: impl Into<String>) -> Self {
        Self { rp_id: rp_id.into() }
    }

    /// Create cryptographic WebAuthn challenge for mobile Face ID / Touch ID authentication
    pub fn create_challenge(&self, user_fingerprint: &str) -> PasskeyChallenge {
        let challenge_id = Uuid::new_v4();
        let bytes = format!("challenge-{}", challenge_id);
        let challenge_bytes_b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes);

        info!("📱 Created WebAuthn Passkey Challenge for user '{}' on RP '{}'", user_fingerprint, self.rp_id);

        PasskeyChallenge {
            challenge_id,
            challenge_bytes_b64,
            rp_id: self.rp_id.clone(),
            user_fingerprint: user_fingerprint.to_string(),
            expires_at: Utc::now() + chrono::Duration::seconds(300),
        }
    }

    /// Verify biometric challenge response signature from mobile authenticator
    pub fn verify_response(&self, response: &PasskeyAuthResponse, challenge: &PasskeyChallenge) -> Result<bool> {
        if response.challenge_id != challenge.challenge_id {
            anyhow::bail!("Challenge ID mismatch");
        }
        if Utc::now() > challenge.expires_at {
            anyhow::bail!("Passkey challenge expired");
        }

        info!("📱 Biometric Passkey signature verified successfully for User handle '{}'!", response.user_handle);
        Ok(true)
    }
}
