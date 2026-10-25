//! # NetGene Presence
//!
//! Proof that a human approved a specific action, using a hardware-backed key
//! that only signs after local user verification (PIN, fingerprint, face).
//!
//! - **Windows:** Windows Hello `KeyCredentialManager` (RSA-2048 key in the
//!   TPM / Windows Hello container, signs with RSASSA-PKCS1-v1.5 + SHA-256).
//! - Other platforms: not yet implemented; [`is_available`] returns `false`.
//!
//! The challenge binds the proposal id and the exact action, so a proof for
//! one action cannot be replayed for another.

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use ring::signature::{UnparsedPublicKey, RSA_PKCS1_2048_8192_SHA256};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use uuid::Uuid;

const CHALLENGE_DOMAIN: &[u8] = b"netgene-presence-challenge-v1\0";

/// Verification method identifier stored with an enrolment.
pub const METHOD_WINDOWS_HELLO: &str = "windows-hello-rsa2048-pkcs1-sha256";

/// Public record of an enrolled presence key (safe to store on disk).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenceEnrollment {
    pub gene_id: Uuid,
    pub method: String,
    /// Name of the platform credential holding the private key.
    pub credential_name: String,
    /// PKCS#1 RSAPublicKey (DER), base64.
    pub public_key_b64: String,
    pub enrolled_at: DateTime<Utc>,
}

impl PresenceEnrollment {
    pub fn path_for(dir: &Path, gene_id: Uuid) -> PathBuf {
        dir.join(format!("{}.json", gene_id))
    }

    pub fn load(dir: &Path, gene_id: Uuid) -> Result<Option<Self>> {
        let path = Self::path_for(dir, gene_id);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&text).with_context(|| format!("Invalid enrolment {}", path.display()))?))
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        std::fs::write(Self::path_for(dir, self.gene_id), serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn public_key(&self) -> Result<Vec<u8>> {
        Ok(BASE64.decode(&self.public_key_b64)?)
    }
}

/// Bytes the user's key signs to approve `proposal_id` for exactly this action.
pub fn challenge(proposal_id: Uuid, action_kind: &str, params: &serde_json::Value) -> Result<Vec<u8>> {
    let mut hasher = Sha3_256::new();
    hasher.update(CHALLENGE_DOMAIN);
    hasher.update(proposal_id.as_bytes());
    hasher.update((action_kind.len() as u32).to_be_bytes());
    hasher.update(action_kind.as_bytes());
    hasher.update(serde_json::to_vec(params)?);
    Ok(hasher.finalize().to_vec())
}

/// Verify a presence signature (RSASSA-PKCS1-v1.5, SHA-256, RSA ≥ 2048).
pub fn verify(public_key_pkcs1_der: &[u8], challenge: &[u8], signature: &[u8]) -> bool {
    UnparsedPublicKey::new(&RSA_PKCS1_2048_8192_SHA256, public_key_pkcs1_der)
        .verify(challenge, signature)
        .is_ok()
}

/// Whether a hardware-backed user-verification key can be used here.
pub fn is_available() -> bool {
    #[cfg(windows)]
    {
        windows_hello::is_supported().unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Create (or replace) the presence key for `gene_id`. Shows the platform prompt.
pub fn enroll(gene_id: Uuid) -> Result<PresenceEnrollment> {
    let credential_name = format!("netgene-{}", gene_id);
    #[cfg(windows)]
    {
        let public_key = windows_hello::create(&credential_name)?;
        Ok(PresenceEnrollment {
            gene_id,
            method: METHOD_WINDOWS_HELLO.to_string(),
            credential_name,
            public_key_b64: BASE64.encode(public_key),
            enrolled_at: Utc::now(),
        })
    }
    #[cfg(not(windows))]
    {
        let _ = credential_name;
        anyhow::bail!("Hardware user verification is only implemented on Windows (Windows Hello) so far")
    }
}

/// Ask the user to approve `challenge` with the enrolled key. Shows the platform prompt.
pub fn sign(enrollment: &PresenceEnrollment, challenge: &[u8]) -> Result<Vec<u8>> {
    #[cfg(windows)]
    {
        windows_hello::sign(&enrollment.credential_name, challenge)
    }
    #[cfg(not(windows))]
    {
        let _ = (enrollment, challenge);
        anyhow::bail!("Hardware user verification is only implemented on Windows (Windows Hello) so far")
    }
}

#[cfg(windows)]
mod windows_hello {
    use anyhow::Result;
    use windows::core::{Array, HSTRING};
    use windows::Security::Credentials::{KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus};
    use windows::Security::Cryptography::{Core::CryptographicPublicKeyBlobType, CryptographicBuffer};

    fn check(status: KeyCredentialStatus) -> Result<()> {
        if status == KeyCredentialStatus::Success {
            return Ok(());
        }
        let reason = if status == KeyCredentialStatus::UserCanceled {
            "cancelled by the user"
        } else if status == KeyCredentialStatus::NotFound {
            "no presence key enrolled (run `netgene gene enroll-presence`)"
        } else if status == KeyCredentialStatus::UserPrefersPassword {
            "the user chose a password instead"
        } else if status == KeyCredentialStatus::SecurityDeviceLocked {
            "the security device is locked"
        } else {
            "Windows Hello returned an error"
        };
        anyhow::bail!("Windows Hello: {}", reason)
    }

    pub fn is_supported() -> Result<bool> {
        Ok(KeyCredentialManager::IsSupportedAsync()?.join()?)
    }

    pub fn create(name: &str) -> Result<Vec<u8>> {
        let result = KeyCredentialManager::RequestCreateAsync(&HSTRING::from(name), KeyCredentialCreationOption::ReplaceExisting)?.join()?;
        check(result.Status()?)?;
        let buffer = result.Credential()?.RetrievePublicKeyWithBlobType(CryptographicPublicKeyBlobType::Pkcs1RsaPublicKey)?;
        let mut bytes = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&buffer, &mut bytes)?;
        Ok(bytes.to_vec())
    }

    pub fn sign(name: &str, data: &[u8]) -> Result<Vec<u8>> {
        let opened = KeyCredentialManager::OpenAsync(&HSTRING::from(name))?.join()?;
        check(opened.Status()?)?;
        let signed = opened.Credential()?.RequestSignAsync(&CryptographicBuffer::CreateFromByteArray(data)?)?.join()?;
        check(signed.Status()?)?;
        let mut bytes = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&signed.Result()?, &mut bytes)?;
        Ok(bytes.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};

    /// Software stand-in for the hardware key (test fixture only).
    fn test_key() -> RsaKeyPair {
        RsaKeyPair::from_pkcs8(include_bytes!("../tests/fixtures/test-only-rsa2048.pk8.der")).unwrap()
    }

    fn sign_with(key: &RsaKeyPair, msg: &[u8]) -> Vec<u8> {
        let mut sig = vec![0u8; key.public().modulus_len()];
        key.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), msg, &mut sig).unwrap();
        sig
    }

    #[test]
    fn test_challenge_binds_proposal_and_action() {
        let id = Uuid::new_v4();
        let params = serde_json::json!({"count": 2});
        let c = challenge(id, "node.provision", &params).unwrap();
        assert_ne!(c, challenge(Uuid::new_v4(), "node.provision", &params).unwrap());
        assert_ne!(c, challenge(id, "network.optimize", &params).unwrap());
        assert_ne!(c, challenge(id, "node.provision", &serde_json::json!({"count": 9})).unwrap());
    }

    #[test]
    fn test_verify_accepts_only_matching_signature() {
        let key = test_key();
        let public = key.public().as_ref().to_vec();
        let c = challenge(Uuid::new_v4(), "node.provision", &serde_json::json!({})).unwrap();
        let sig = sign_with(&key, &c);
        assert!(verify(&public, &c, &sig));

        let other = challenge(Uuid::new_v4(), "node.provision", &serde_json::json!({})).unwrap();
        assert!(!verify(&public, &other, &sig), "a proof cannot be replayed for another proposal");
    }

    #[test]
    fn test_enrollment_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let e = PresenceEnrollment {
            gene_id: Uuid::new_v4(),
            method: METHOD_WINDOWS_HELLO.into(),
            credential_name: "netgene-test".into(),
            public_key_b64: BASE64.encode(test_key().public().as_ref()),
            enrolled_at: Utc::now(),
        };
        e.save(dir.path()).unwrap();
        assert_eq!(PresenceEnrollment::load(dir.path(), e.gene_id).unwrap(), Some(e.clone()));
        assert!(PresenceEnrollment::load(dir.path(), Uuid::new_v4()).unwrap().is_none());
    }
}
