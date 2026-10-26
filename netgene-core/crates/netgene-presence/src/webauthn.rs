//! WebAuthn passkeys as a user-presence factor (approve from a phone).
//!
//! The ceremony runs in a browser on `http://localhost:<port>` (browsers allow
//! WebAuthn on localhost without TLS); the browser offers the platform
//! authenticator or "use a phone", where the phone's passkey signs. This
//! module only verifies, with `ring`:
//!
//! - **registration:** `clientDataJSON` (type, challenge, origin) and the
//!   authenticator data (RP id hash, user presence + verification flags, the
//!   attested credential and its COSE key). Attestation statements are not
//!   verified: the key is trusted because the local operator enrolled it.
//! - **assertion:** type, challenge, origin, RP id hash, UP + UV flags, the
//!   signature over `authenticatorData ‖ SHA-256(clientDataJSON)`, and a
//!   strictly increasing signature counter when the authenticator keeps one.
//!
//! Supported keys: ES256 (P-256), RS256 (RSA ≥ 2048) and EdDSA (Ed25519).

use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use chrono::{DateTime, Utc};
use ciborium::Value as Cbor;
use ring::digest::{digest, SHA256};
use ring::signature::{self, RsaPublicKeyComponents, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Relying party id for the local ceremony page.
pub const LOCAL_RP_ID: &str = "localhost";

const FLAG_UP: u8 = 0x01;
const FLAG_UV: u8 = 0x04;
const FLAG_AT: u8 = 0x40;

const ALG_ES256: i64 = -7;
const ALG_EDDSA: i64 = -8;
const ALG_RS256: i64 = -257;

/// An enrolled passkey (public data only).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PasskeyCredential {
    pub gene_id: Uuid,
    pub credential_id_b64: String,
    /// COSE algorithm: -7 ES256, -8 EdDSA, -257 RS256.
    pub alg: i64,
    /// ES256: uncompressed point; EdDSA: 32-byte key; RS256: modulus.
    pub public_key_b64: String,
    /// RS256 exponent.
    #[serde(default)]
    pub rsa_exponent_b64: Option<String>,
    pub rp_id: String,
    pub sign_count: u32,
    pub enrolled_at: DateTime<Utc>,
}

/// What the browser returns from `navigator.credentials.create`.
#[derive(Debug, Clone, Deserialize)]
pub struct RegistrationResponse {
    pub client_data_json: String,
    pub attestation_object: String,
}

/// What the browser returns from `navigator.credentials.get`.
#[derive(Debug, Clone, Deserialize)]
pub struct AssertionResponse {
    pub credential_id: String,
    pub client_data_json: String,
    pub authenticator_data: String,
    pub signature: String,
}

#[derive(Deserialize)]
struct ClientData {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
    #[serde(default, rename = "crossOrigin")]
    cross_origin: bool,
}

fn b64(value: &str) -> Result<Vec<u8>> {
    B64URL.decode(value.trim_end_matches('=')).context("invalid base64url")
}

fn check_client_data(raw: &[u8], kind: &str, challenge: &[u8], origin: &str) -> Result<()> {
    let data: ClientData = serde_json::from_slice(raw).context("clientDataJSON is not valid JSON")?;
    if data.kind != kind {
        anyhow::bail!("clientDataJSON type is '{}', expected '{}'", data.kind, kind);
    }
    if b64(&data.challenge)? != challenge {
        anyhow::bail!("WebAuthn challenge does not match");
    }
    if data.origin != origin || data.cross_origin {
        anyhow::bail!("WebAuthn origin '{}' is not '{}'", data.origin, origin);
    }
    Ok(())
}

struct AuthData<'a> {
    flags: u8,
    sign_count: u32,
    attested: &'a [u8],
}

fn parse_auth_data<'a>(raw: &'a [u8], rp_id: &str) -> Result<AuthData<'a>> {
    if raw.len() < 37 {
        anyhow::bail!("authenticator data too short");
    }
    if raw[..32] != *digest(&SHA256, rp_id.as_bytes()).as_ref() {
        anyhow::bail!("authenticator data is for another relying party");
    }
    let flags = raw[32];
    if flags & FLAG_UP == 0 || flags & FLAG_UV == 0 {
        anyhow::bail!("the authenticator did not verify the user (UP and UV are required)");
    }
    Ok(AuthData { flags, sign_count: u32::from_be_bytes(raw[33..37].try_into()?), attested: &raw[37..] })
}

fn cose_get(map: &[(Cbor, Cbor)], key: i64) -> Option<&Cbor> {
    map.iter().find(|(k, _)| k.as_integer().map(i128::from) == Some(key as i128)).map(|(_, v)| v)
}

fn cose_bytes(map: &[(Cbor, Cbor)], key: i64) -> Result<Vec<u8>> {
    cose_get(map, key).and_then(Cbor::as_bytes).cloned().with_context(|| format!("COSE key lacks parameter {}", key))
}

fn cose_int(map: &[(Cbor, Cbor)], key: i64) -> Result<i64> {
    let value = cose_get(map, key).and_then(Cbor::as_integer).with_context(|| format!("COSE key lacks parameter {}", key))?;
    Ok(i64::try_from(i128::from(value))?)
}

/// Options for `navigator.credentials.create` (binary fields as base64url).
pub fn creation_options(gene_id: Uuid, gene_name: &str, challenge: &[u8], rp_id: &str) -> serde_json::Value {
    serde_json::json!({
        "rp": { "id": rp_id, "name": "NetGene OS" },
        "user": { "id": B64URL.encode(gene_id.as_bytes()), "name": gene_name, "displayName": gene_name },
        "challenge": B64URL.encode(challenge),
        "pubKeyCredParams": [
            { "type": "public-key", "alg": ALG_ES256 },
            { "type": "public-key", "alg": ALG_EDDSA },
            { "type": "public-key", "alg": ALG_RS256 },
        ],
        "authenticatorSelection": { "userVerification": "required", "residentKey": "preferred" },
        "attestation": "none",
        "timeout": 300000,
    })
}

/// Options for `navigator.credentials.get`.
pub fn request_options(credentials: &[PasskeyCredential], challenge: &[u8], rp_id: &str) -> serde_json::Value {
    serde_json::json!({
        "rpId": rp_id,
        "challenge": B64URL.encode(challenge),
        "allowCredentials": credentials.iter().map(|c| serde_json::json!({ "type": "public-key", "id": c.credential_id_b64 })).collect::<Vec<_>>(),
        "userVerification": "required",
        "timeout": 300000,
    })
}

/// Verify a registration and return the credential to store.
pub fn verify_registration(gene_id: Uuid, response: &RegistrationResponse, challenge: &[u8], rp_id: &str, origin: &str) -> Result<PasskeyCredential> {
    check_client_data(&b64(&response.client_data_json)?, "webauthn.create", challenge, origin)?;

    let attestation: Cbor = ciborium::from_reader(b64(&response.attestation_object)?.as_slice()).context("attestationObject is not CBOR")?;
    let auth_data = attestation
        .as_map()
        .and_then(|m| m.iter().find(|(k, _)| k.as_text() == Some("authData")))
        .and_then(|(_, v)| v.as_bytes())
        .context("attestationObject has no authData")?;
    let parsed = parse_auth_data(auth_data, rp_id)?;
    if parsed.flags & FLAG_AT == 0 {
        anyhow::bail!("registration carries no attested credential");
    }
    let attested = parsed.attested;
    if attested.len() < 18 {
        anyhow::bail!("attested credential data too short");
    }
    let id_len = u16::from_be_bytes([attested[16], attested[17]]) as usize;
    let credential_id = attested.get(18..18 + id_len).context("credential id truncated")?;
    let key: Cbor = ciborium::from_reader(&attested[18 + id_len..]).context("credential public key is not CBOR")?;
    let key = key.as_map().context("credential public key is not a COSE map")?;

    let alg = cose_int(key, 3)?;
    let (public_key, rsa_exponent) = match alg {
        ALG_ES256 => {
            if cose_int(key, 1)? != 2 || cose_int(key, -1)? != 1 {
                anyhow::bail!("ES256 key must be an EC2 P-256 key");
            }
            let (x, y) = (cose_bytes(key, -2)?, cose_bytes(key, -3)?);
            if x.len() != 32 || y.len() != 32 {
                anyhow::bail!("invalid P-256 coordinates");
            }
            ([&[0x04][..], &x, &y].concat(), None)
        }
        ALG_EDDSA => {
            if cose_int(key, 1)? != 1 || cose_int(key, -1)? != 6 {
                anyhow::bail!("EdDSA key must be an OKP Ed25519 key");
            }
            (cose_bytes(key, -2)?, None)
        }
        ALG_RS256 => {
            let n = cose_bytes(key, -1)?;
            if n.len() < 256 {
                anyhow::bail!("RSA passkeys must be at least 2048 bits");
            }
            (n, Some(B64URL.encode(cose_bytes(key, -2)?)))
        }
        other => anyhow::bail!("unsupported COSE algorithm {}", other),
    };

    Ok(PasskeyCredential {
        gene_id,
        credential_id_b64: B64URL.encode(credential_id),
        alg,
        public_key_b64: B64URL.encode(public_key),
        rsa_exponent_b64: rsa_exponent,
        rp_id: rp_id.to_string(),
        sign_count: parsed.sign_count,
        enrolled_at: Utc::now(),
    })
}

/// Verify an assertion against one of `credentials`; on success returns the
/// index of the credential used and its new signature counter.
pub fn verify_assertion(credentials: &[PasskeyCredential], response: &AssertionResponse, challenge: &[u8], origin: &str) -> Result<(usize, u32)> {
    let wanted = b64(&response.credential_id)?;
    let index = credentials
        .iter()
        .position(|c| b64(&c.credential_id_b64).ok().as_deref() == Some(wanted.as_slice()))
        .context("passkey is not enrolled for this gene")?;
    let credential = &credentials[index];

    let client_data = b64(&response.client_data_json)?;
    check_client_data(&client_data, "webauthn.get", challenge, origin)?;
    let auth_data = b64(&response.authenticator_data)?;
    let parsed = parse_auth_data(&auth_data, &credential.rp_id)?;

    let mut signed = auth_data.clone();
    signed.extend_from_slice(digest(&SHA256, &client_data).as_ref());
    let sig = b64(&response.signature)?;
    let key = b64(&credential.public_key_b64)?;
    let valid = match credential.alg {
        ALG_ES256 => UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_ASN1, &key).verify(&signed, &sig).is_ok(),
        ALG_EDDSA => UnparsedPublicKey::new(&signature::ED25519, &key).verify(&signed, &sig).is_ok(),
        ALG_RS256 => {
            let e = b64(credential.rsa_exponent_b64.as_deref().context("RSA passkey has no exponent")?)?;
            RsaPublicKeyComponents { n: key.as_slice(), e: e.as_slice() }
                .verify(&signature::RSA_PKCS1_2048_8192_SHA256, &signed, &sig)
                .is_ok()
        }
        other => anyhow::bail!("unsupported COSE algorithm {}", other),
    };
    if !valid {
        anyhow::bail!("passkey signature is invalid");
    }
    // Authenticators that count must count up; a repeat suggests a cloned key.
    if (parsed.sign_count != 0 || credential.sign_count != 0) && parsed.sign_count <= credential.sign_count {
        anyhow::bail!("passkey signature counter did not increase ({} <= {})", parsed.sign_count, credential.sign_count);
    }
    Ok((index, parsed.sign_count))
}

/// `~/.netgene/presence/<gene>.passkeys.json`.
pub fn passkeys_path(dir: &Path, gene_id: Uuid) -> PathBuf {
    dir.join(format!("{}.passkeys.json", gene_id))
}

pub fn load_passkeys(dir: &Path, gene_id: Uuid) -> Result<Vec<PasskeyCredential>> {
    let path = passkeys_path(dir, gene_id);
    if !path.exists() {
        return Ok(vec![]);
    }
    serde_json::from_str(&std::fs::read_to_string(&path)?).with_context(|| format!("Invalid passkey file {}", path.display()))
}

pub fn save_passkeys(dir: &Path, gene_id: Uuid, credentials: &[PasskeyCredential]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(passkeys_path(dir, gene_id), serde_json::to_string_pretty(credentials)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, Ed25519KeyPair, KeyPair, RsaKeyPair, ECDSA_P256_SHA256_ASN1_SIGNING, RSA_PKCS1_SHA256};

    const ORIGIN: &str = "http://localhost:4321";

    enum Key {
        Ec(EcdsaKeyPair),
        Ed(Ed25519KeyPair),
        Rsa(RsaKeyPair),
    }

    /// A software authenticator producing what a browser would return.
    struct Authenticator {
        key: Key,
        credential_id: Vec<u8>,
        counter: u32,
    }

    impl Authenticator {
        fn ec() -> Self {
            let rng = SystemRandom::new();
            let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
            let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap();
            Self { key: Key::Ec(key), credential_id: vec![7; 16], counter: 0 }
        }

        fn ed() -> Self {
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new()).unwrap();
            Self { key: Key::Ed(Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap()), credential_id: vec![9; 20], counter: 5 }
        }

        fn rsa() -> Self {
            let key = RsaKeyPair::from_pkcs8(include_bytes!("../tests/fixtures/test-only-rsa2048.pk8.der")).unwrap();
            Self { key: Key::Rsa(key), credential_id: vec![3; 32], counter: 0 }
        }

        fn cose_key(&self) -> Vec<u8> {
            let map = match &self.key {
                Key::Ec(k) => {
                    let p = k.public_key().as_ref();
                    vec![
                        (Cbor::from(1), Cbor::from(2)),
                        (Cbor::from(3), Cbor::from(ALG_ES256)),
                        (Cbor::from(-1), Cbor::from(1)),
                        (Cbor::from(-2), Cbor::Bytes(p[1..33].to_vec())),
                        (Cbor::from(-3), Cbor::Bytes(p[33..65].to_vec())),
                    ]
                }
                Key::Ed(k) => vec![
                    (Cbor::from(1), Cbor::from(1)),
                    (Cbor::from(3), Cbor::from(ALG_EDDSA)),
                    (Cbor::from(-1), Cbor::from(6)),
                    (Cbor::from(-2), Cbor::Bytes(k.public_key().as_ref().to_vec())),
                ],
                Key::Rsa(k) => {
                    let c: RsaPublicKeyComponents<Vec<u8>> = k.public().into();
                    vec![
                        (Cbor::from(1), Cbor::from(3)),
                        (Cbor::from(3), Cbor::from(ALG_RS256)),
                        (Cbor::from(-1), Cbor::Bytes(c.n)),
                        (Cbor::from(-2), Cbor::Bytes(c.e)),
                    ]
                }
            };
            let mut out = vec![];
            ciborium::into_writer(&Cbor::Map(map), &mut out).unwrap();
            out
        }

        fn auth_data(&self, rp_id: &str, flags: u8, attested: bool) -> Vec<u8> {
            let mut data = digest(&SHA256, rp_id.as_bytes()).as_ref().to_vec();
            data.push(flags | if attested { FLAG_AT } else { 0 });
            data.extend_from_slice(&self.counter.to_be_bytes());
            if attested {
                data.extend_from_slice(&[0; 16]);
                data.extend_from_slice(&(self.credential_id.len() as u16).to_be_bytes());
                data.extend_from_slice(&self.credential_id);
                data.extend_from_slice(&self.cose_key());
            }
            data
        }

        fn client_data(kind: &str, challenge: &[u8], origin: &str) -> Vec<u8> {
            serde_json::to_vec(&serde_json::json!({ "type": kind, "challenge": B64URL.encode(challenge), "origin": origin, "crossOrigin": false })).unwrap()
        }

        fn register(&self, challenge: &[u8]) -> RegistrationResponse {
            let attestation = Cbor::Map(vec![
                (Cbor::from("fmt"), Cbor::from("none")),
                (Cbor::from("attStmt"), Cbor::Map(vec![])),
                (Cbor::from("authData"), Cbor::Bytes(self.auth_data(LOCAL_RP_ID, FLAG_UP | FLAG_UV, true))),
            ]);
            let mut att = vec![];
            ciborium::into_writer(&attestation, &mut att).unwrap();
            RegistrationResponse {
                client_data_json: B64URL.encode(Self::client_data("webauthn.create", challenge, ORIGIN)),
                attestation_object: B64URL.encode(att),
            }
        }

        fn assert(&mut self, challenge: &[u8], origin: &str, flags: u8) -> AssertionResponse {
            self.counter += 1;
            let auth = self.auth_data(LOCAL_RP_ID, flags, false);
            let client = Self::client_data("webauthn.get", challenge, origin);
            let mut signed = auth.clone();
            signed.extend_from_slice(digest(&SHA256, &client).as_ref());
            let sig = match &self.key {
                Key::Ec(k) => k.sign(&SystemRandom::new(), &signed).unwrap().as_ref().to_vec(),
                Key::Ed(k) => k.sign(&signed).as_ref().to_vec(),
                Key::Rsa(k) => {
                    let mut sig = vec![0u8; k.public().modulus_len()];
                    k.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), &signed, &mut sig).unwrap();
                    sig
                }
            };
            AssertionResponse {
                credential_id: B64URL.encode(&self.credential_id),
                client_data_json: B64URL.encode(client),
                authenticator_data: B64URL.encode(auth),
                signature: B64URL.encode(sig),
            }
        }
    }

    #[test]
    fn test_register_then_approve_with_es256_eddsa_and_rs256() {
        for mut authn in [Authenticator::ec(), Authenticator::ed(), Authenticator::rsa()] {
            let gene = Uuid::new_v4();
            let cred = verify_registration(gene, &authn.register(b"enrol-challenge-0123"), b"enrol-challenge-0123", LOCAL_RP_ID, ORIGIN).unwrap();
            let mut creds = vec![cred];

            let challenge = [42u8; 32];
            let (i, count) = verify_assertion(&creds, &authn.assert(&challenge, ORIGIN, FLAG_UP | FLAG_UV), &challenge, ORIGIN).unwrap();
            creds[i].sign_count = count;

            // Same assertion again: counter does not increase (replay).
            let replay = authn.assert(&challenge, ORIGIN, FLAG_UP | FLAG_UV);
            verify_assertion(&creds, &replay, &challenge, ORIGIN).unwrap();
            creds[0].sign_count = authn.counter;
            assert!(verify_assertion(&creds, &replay, &challenge, ORIGIN).is_err(), "replayed counter refused");
        }
    }

    #[test]
    fn test_wrong_challenge_origin_flags_or_key_refused() {
        let mut authn = Authenticator::ec();
        let creds = vec![verify_registration(Uuid::new_v4(), &authn.register(b"c-registration!!"), b"c-registration!!", LOCAL_RP_ID, ORIGIN).unwrap()];
        let challenge = [1u8; 32];

        assert!(verify_assertion(&creds, &authn.assert(&[2u8; 32], ORIGIN, FLAG_UP | FLAG_UV), &challenge, ORIGIN).is_err(), "other proposal");
        assert!(verify_assertion(&creds, &authn.assert(&challenge, "http://evil.example", FLAG_UP | FLAG_UV), &challenge, ORIGIN).is_err(), "other origin");
        assert!(verify_assertion(&creds, &authn.assert(&challenge, ORIGIN, FLAG_UP), &challenge, ORIGIN).is_err(), "no user verification");

        let mut tampered = authn.assert(&challenge, ORIGIN, FLAG_UP | FLAG_UV);
        let mut sig = b64(&tampered.signature).unwrap();
        let last = sig.len() - 1;
        sig[last] ^= 1;
        tampered.signature = B64URL.encode(sig);
        assert!(verify_assertion(&creds, &tampered, &challenge, ORIGIN).is_err(), "bad signature");

        let mut other = Authenticator::ec();
        assert!(verify_assertion(&creds, &other.assert(&challenge, ORIGIN, FLAG_UP | FLAG_UV), &challenge, ORIGIN).is_err(), "same id, other key");
    }

    #[test]
    fn test_registration_checks() {
        let authn = Authenticator::ec();
        let reg = authn.register(b"challenge-AAAAAAA");
        assert!(verify_registration(Uuid::new_v4(), &reg, b"challenge-BBBBBBB", LOCAL_RP_ID, ORIGIN).is_err());
        assert!(verify_registration(Uuid::new_v4(), &reg, b"challenge-AAAAAAA", "example.com", ORIGIN).is_err(), "rp id hash");
        assert!(verify_registration(Uuid::new_v4(), &reg, b"challenge-AAAAAAA", LOCAL_RP_ID, "http://localhost:1").is_err(), "origin");
    }

    #[test]
    fn test_passkey_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let authn = Authenticator::ed();
        let gene = Uuid::new_v4();
        let cred = verify_registration(gene, &authn.register(b"0123456789abcdef"), b"0123456789abcdef", LOCAL_RP_ID, ORIGIN).unwrap();
        assert!(load_passkeys(dir.path(), gene).unwrap().is_empty());
        save_passkeys(dir.path(), gene, std::slice::from_ref(&cred)).unwrap();
        assert_eq!(load_passkeys(dir.path(), gene).unwrap(), vec![cred]);
    }
}
