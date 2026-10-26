//! OIDC ID token verification (JWT header/claims decoding, alg selection, JWKS lookup).
#![no_main]

use std::collections::BTreeMap;
use std::sync::OnceLock;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use libfuzzer_sys::fuzz_target;
use netgene_enterprise::{verify_id_token, Jwk, JwkSet, JwksSource, OidcConfig};
use ring::rand::SystemRandom;
use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

fn setup() -> (OidcConfig, JwkSet) {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).unwrap();
    let p = key.public_key().as_ref();
    let ec = Jwk { kty: "EC".into(), kid: Some("k1".into()), alg: Some("ES256".into()), n: None, e: None, crv: Some("P-256".into()), x: Some(B64URL.encode(&p[1..33])), y: Some(B64URL.encode(&p[33..65])) };
    let rsa = Jwk { kty: "RSA".into(), kid: Some("r1".into()), alg: Some("RS256".into()), n: Some(B64URL.encode([0xC5u8; 256])), e: Some("AQAB".into()), crv: None, x: None, y: None };
    let config = OidcConfig {
        issuer: "https://idp.example.com".into(),
        audience: "netgene".into(),
        jwks: JwksSource::File("unused".into()),
        roles_claim: "groups".into(),
        role_capabilities: BTreeMap::new(),
        max_ttl_minutes: 60,
    };
    (config, JwkSet { keys: vec![ec, rsa] })
}

static SETUP: OnceLock<(OidcConfig, JwkSet)> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
    let (config, jwks) = SETUP.get_or_init(setup);
    let token = String::from_utf8_lossy(data);
    let _ = verify_id_token(config, jwks, &token, chrono::Utc::now());
    // Also as three base64url segments, so the decoders see structured input.
    let thirds: Vec<_> = data.chunks(data.len() / 3 + 1).map(|c| B64URL.encode(c)).collect();
    let _ = verify_id_token(config, jwks, &thirds.join("."), chrono::Utc::now());
});
