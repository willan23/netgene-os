//! # NetGene Enterprise
//!
//! Binds a corporate identity (an OpenID Connect ID token from Okta, Entra ID,
//! Keycloak, Google Workspace…, or a SAML 2.0 response with the `saml`
//! feature) to NetGene capabilities. The token is
//! verified against the IdP's JWKS; its group/role claim is mapped to
//! capabilities through `~/.netgene/enterprise.json`; the caller then issues
//! a time-limited [capability token](netgene_gene::CapabilityToken).
//!
//! Verification is deliberately strict and small:
//! - only **RS256** and **ES256** (`alg: none`, HS*, and anything else are refused);
//! - the key is selected by `kid` from the configured JWKS;
//! - `iss` must match exactly, `aud` must contain the configured client id,
//!   `exp`/`nbf` are checked with 60 s of clock leeway.

#[cfg(feature = "saml")]
pub mod saml;

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use chrono::{DateTime, TimeZone, Utc};
use ring::signature::{self, RsaPublicKeyComponents, UnparsedPublicKey};
use serde::{Deserialize, Serialize};

const LEEWAY_SECS: i64 = 60;

/// Where the IdP's signing keys come from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JwksSource {
    /// `https://…/jwks` (fetched on demand and cached, see [`JwksCache`]).
    Url(String),
    /// Local copy of the JWKS (air-gapped deployments, tests).
    File(String),
}

/// `~/.netgene/enterprise.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OidcConfig {
    /// Expected `iss`, e.g. `https://login.example.com/realms/corp`.
    pub issuer: String,
    /// Expected audience (the client id registered for NetGene).
    pub audience: String,
    pub jwks: JwksSource,
    /// Claim holding the user's groups or roles (default `groups`).
    #[serde(default = "default_roles_claim")]
    pub roles_claim: String,
    /// Group/role → NetGene capabilities.
    pub role_capabilities: BTreeMap<String, Vec<String>>,
    /// Upper bound for the issued capability token lifetime.
    #[serde(default = "default_max_ttl")]
    pub max_ttl_minutes: i64,
}

fn default_roles_claim() -> String {
    "groups".into()
}

fn default_max_ttl() -> i64 {
    480
}

impl OidcConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("Invalid enterprise config {}", path.display()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Jwk {
    pub kty: String,
    #[serde(default)]
    pub kid: Option<String>,
    #[serde(default)]
    pub alg: Option<String>,
    // RSA
    #[serde(default)]
    pub n: Option<String>,
    #[serde(default)]
    pub e: Option<String>,
    // EC
    #[serde(default)]
    pub crv: Option<String>,
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwkSet {
    pub keys: Vec<Jwk>,
}

impl JwkSet {
    /// Load the JWKS from its configured source (HTTPS only for URLs).
    pub async fn load(source: &JwksSource) -> Result<Self> {
        match source {
            JwksSource::File(path) => Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?),
            JwksSource::Url(url) => Self::fetch(url.clone()).await,
        }
    }

    /// Download a JWKS over HTTPS.
    pub async fn fetch(url: String) -> Result<Self> {
        if !url.starts_with("https://") {
            anyhow::bail!("JWKS URL must use https://");
        }
        Ok(reqwest::get(&url).await?.error_for_status()?.json().await?)
    }

    fn has_kid(&self, kid: &str) -> bool {
        self.keys.iter().any(|k| k.kid.as_deref() == Some(kid))
    }
}

#[derive(Serialize, Deserialize)]
struct CachedJwks {
    url: String,
    fetched_at: DateTime<Utc>,
    jwks: JwkSet,
}

/// On-disk cache of a remote JWKS (`~/.netgene/jwks-cache.json`).
///
/// - a cached set younger than `ttl` is used without network access;
/// - a token whose `kid` is not in the cache (IdP key rotation) forces a
///   refetch, at most once per `min_refetch`, so bogus tokens cannot hammer the IdP;
/// - when the IdP is unreachable, a cache younger than `max_stale` is still used.
///
/// The cache holds only public keys, but it decides which keys are trusted, so
/// it lives next to `enterprise.json` and shares its trust boundary.
pub struct JwksCache {
    path: PathBuf,
    pub ttl: chrono::Duration,
    pub max_stale: chrono::Duration,
    pub min_refetch: chrono::Duration,
}

impl JwksCache {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            ttl: chrono::Duration::hours(1),
            max_stale: chrono::Duration::hours(24),
            min_refetch: chrono::Duration::seconds(60),
        }
    }

    fn read(&self, url: &str) -> Option<CachedJwks> {
        let cached: CachedJwks = serde_json::from_str(&std::fs::read_to_string(&self.path).ok()?).ok()?;
        (cached.url == url).then_some(cached)
    }

    fn write(&self, cached: &CachedJwks) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = self.path.with_extension("tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(cached)?)?;
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    /// Verify `token`, fetching the JWKS through `fetch` only when needed.
    pub async fn verify<F, Fut>(&self, config: &OidcConfig, token: &str, now: DateTime<Utc>, fetch: F) -> Result<VerifiedIdentity>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<JwkSet>>,
    {
        let url = match &config.jwks {
            JwksSource::File(_) => return verify_id_token(config, &JwkSet::load(&config.jwks).await?, token, now),
            JwksSource::Url(url) => url.clone(),
        };
        let kid = token_kid(token)?;
        let cached = self.read(&url);
        let age = |c: &CachedJwks| now - c.fetched_at;

        let usable = match &cached {
            Some(c) if age(c) < self.ttl && (c.jwks.has_kid(&kid) || age(c) < self.min_refetch) => Some(&c.jwks),
            _ => None,
        };
        if let Some(jwks) = usable {
            return verify_id_token(config, jwks, token, now);
        }

        match fetch(url.clone()).await {
            Ok(jwks) => {
                let fresh = CachedJwks { url, fetched_at: now, jwks };
                if let Err(e) = self.write(&fresh) {
                    tracing::warn!("Could not write JWKS cache {:?}: {}", self.path, e);
                }
                verify_id_token(config, &fresh.jwks, token, now)
            }
            Err(e) => match cached {
                Some(c) if age(&c) < self.max_stale => {
                    tracing::warn!("JWKS fetch failed ({}); using cached keys from {}", e, c.fetched_at);
                    verify_id_token(config, &c.jwks, token, now)
                }
                _ => Err(e.context("JWKS fetch failed and no usable cache")),
            },
        }
    }
}

fn token_kid(token: &str) -> Result<String> {
    let header: Header = decode_part(token.split('.').next().unwrap_or_default())?;
    header.kid.context("ID token has no kid")
}

/// The identity asserted by a verified ID token.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifiedIdentity {
    pub subject: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct Header {
    alg: String,
    #[serde(default)]
    kid: Option<String>,
}

fn decode_part<T: serde::de::DeserializeOwned>(part: &str) -> Result<T> {
    Ok(serde_json::from_slice(&B64URL.decode(part).context("Token part is not base64url")?)?)
}

fn verify_signature(alg: &str, jwk: &Jwk, signed: &[u8], sig: &[u8]) -> Result<()> {
    let ok = match alg {
        "RS256" => {
            if jwk.kty != "RSA" {
                anyhow::bail!("JWK {} is not an RSA key", jwk.kid.clone().unwrap_or_default());
            }
            let n = B64URL.decode(jwk.n.as_deref().context("RSA JWK without n")?)?;
            let e = B64URL.decode(jwk.e.as_deref().context("RSA JWK without e")?)?;
            RsaPublicKeyComponents { n: &n, e: &e }
                .verify(&signature::RSA_PKCS1_2048_8192_SHA256, signed, sig)
                .is_ok()
        }
        "ES256" => {
            if jwk.kty != "EC" || jwk.crv.as_deref() != Some("P-256") {
                anyhow::bail!("JWK {} is not a P-256 key", jwk.kid.clone().unwrap_or_default());
            }
            let mut point = vec![0x04];
            point.extend(B64URL.decode(jwk.x.as_deref().context("EC JWK without x")?)?);
            point.extend(B64URL.decode(jwk.y.as_deref().context("EC JWK without y")?)?);
            UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, &point).verify(signed, sig).is_ok()
        }
        other => anyhow::bail!("Unsupported token algorithm '{}' (only RS256 and ES256)", other),
    };
    if !ok {
        anyhow::bail!("ID token signature is invalid");
    }
    Ok(())
}

fn claim_strings(value: Option<&serde_json::Value>) -> Vec<String> {
    match value {
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(serde_json::Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

/// Verify an ID token against `config` and `jwks` at time `now`.
pub fn verify_id_token(config: &OidcConfig, jwks: &JwkSet, token: &str, now: DateTime<Utc>) -> Result<VerifiedIdentity> {
    let parts: Vec<&str> = token.trim().split('.').collect();
    let [header_b64, payload_b64, sig_b64] = parts.as_slice() else {
        anyhow::bail!("ID token must have three parts");
    };
    let header: Header = decode_part(header_b64)?;
    if header.alg != "RS256" && header.alg != "ES256" {
        anyhow::bail!("Unsupported token algorithm '{}' (only RS256 and ES256)", header.alg);
    }
    let kid = header.kid.context("ID token has no kid")?;
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid.as_deref() == Some(kid.as_str()))
        .with_context(|| format!("No JWKS key with kid '{}'", kid))?;
    if jwk.alg.as_deref().is_some_and(|a| a != header.alg) {
        anyhow::bail!("Token alg does not match the JWK alg");
    }

    let signed = format!("{}.{}", header_b64, payload_b64);
    verify_signature(&header.alg, jwk, signed.as_bytes(), &B64URL.decode(sig_b64)?)?;

    let claims: serde_json::Value = decode_part(payload_b64)?;
    if claims.get("iss").and_then(|v| v.as_str()) != Some(config.issuer.as_str()) {
        anyhow::bail!("ID token issuer does not match");
    }
    if !claim_strings(claims.get("aud")).contains(&config.audience) {
        anyhow::bail!("ID token audience does not include '{}'", config.audience);
    }
    let exp = claims.get("exp").and_then(|v| v.as_i64()).context("ID token has no exp")?;
    if now.timestamp() > exp + LEEWAY_SECS {
        anyhow::bail!("ID token expired");
    }
    if let Some(nbf) = claims.get("nbf").and_then(|v| v.as_i64()) {
        if now.timestamp() + LEEWAY_SECS < nbf {
            anyhow::bail!("ID token not valid yet");
        }
    }
    let subject = claims.get("sub").and_then(|v| v.as_str()).context("ID token has no sub")?.to_string();

    Ok(VerifiedIdentity {
        subject,
        email: claims.get("email").and_then(|v| v.as_str()).map(String::from),
        roles: claim_strings(claims.get(&config.roles_claim)),
        expires_at: Utc.timestamp_opt(exp, 0).single().context("Invalid exp")?,
    })
}

/// Capabilities granted by the identity's roles (deduplicated, sorted).
pub fn capabilities_for(config: &OidcConfig, identity: &VerifiedIdentity) -> Vec<String> {
    map_roles(&config.role_capabilities, &identity.roles)
}

/// Union of the capabilities mapped to `roles`, sorted.
pub fn map_roles(role_capabilities: &BTreeMap<String, Vec<String>>, roles: &[String]) -> Vec<String> {
    let mut caps: Vec<String> = roles.iter().filter_map(|r| role_capabilities.get(r)).flatten().cloned().collect();
    caps.sort();
    caps.dedup();
    caps
}

/// Lifetime for the capability token: until the ID token expires, capped by config.
pub fn grant_ttl(config: &OidcConfig, identity: &VerifiedIdentity, now: DateTime<Utc>) -> chrono::Duration {
    let until_expiry = identity.expires_at - now;
    until_expiry.min(chrono::Duration::minutes(config.max_ttl_minutes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

    struct Idp {
        key: EcdsaKeyPair,
        jwks: JwkSet,
    }

    fn idp() -> Idp {
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng).unwrap();
        let point = key.public_key().as_ref();
        let jwk = Jwk {
            kty: "EC".into(),
            kid: Some("k1".into()),
            alg: Some("ES256".into()),
            n: None,
            e: None,
            crv: Some("P-256".into()),
            x: Some(B64URL.encode(&point[1..33])),
            y: Some(B64URL.encode(&point[33..65])),
        };
        Idp { key, jwks: JwkSet { keys: vec![jwk] } }
    }

    fn token(idp: &Idp, header: serde_json::Value, claims: serde_json::Value) -> String {
        let signed = format!("{}.{}", B64URL.encode(header.to_string()), B64URL.encode(claims.to_string()));
        let sig = idp.key.sign(&SystemRandom::new(), signed.as_bytes()).unwrap();
        format!("{}.{}", signed, B64URL.encode(sig.as_ref()))
    }

    fn config() -> OidcConfig {
        OidcConfig {
            issuer: "https://idp.example.com".into(),
            audience: "netgene".into(),
            jwks: JwksSource::File("unused".into()),
            roles_claim: "groups".into(),
            role_capabilities: BTreeMap::from([
                ("netgene-operators".into(), vec!["node.spawn".into(), "network.read".into()]),
                ("netgene-admins".into(), vec!["network.admin".into(), "node.spawn".into()]),
            ]),
            max_ttl_minutes: 60,
        }
    }

    fn claims(exp_offset: i64) -> serde_json::Value {
        serde_json::json!({
            "iss": "https://idp.example.com",
            "aud": ["netgene", "other"],
            "sub": "user-123",
            "email": "ana@example.com",
            "groups": ["netgene-operators", "netgene-admins", "unrelated"],
            "exp": Utc::now().timestamp() + exp_offset,
        })
    }

    fn header() -> serde_json::Value {
        serde_json::json!({"alg": "ES256", "kid": "k1", "typ": "JWT"})
    }

    #[test]
    fn test_valid_token_maps_roles() {
        let idp = idp();
        let id = verify_id_token(&config(), &idp.jwks, &token(&idp, header(), claims(3600)), Utc::now()).unwrap();
        assert_eq!(id.subject, "user-123");
        assert_eq!(capabilities_for(&config(), &id), vec!["network.admin", "network.read", "node.spawn"]);
        assert!(grant_ttl(&config(), &id, Utc::now()) <= chrono::Duration::minutes(60));
    }

    #[test]
    fn test_wrong_issuer_audience_or_expired_rejected() {
        let idp = idp();
        let mut bad_iss = claims(3600);
        bad_iss["iss"] = "https://evil.example.com".into();
        let mut bad_aud = claims(3600);
        bad_aud["aud"] = "someone-else".into();
        for c in [bad_iss, bad_aud, claims(-3600)] {
            assert!(verify_id_token(&config(), &idp.jwks, &token(&idp, header(), c), Utc::now()).is_err());
        }
    }

    #[test]
    fn test_forged_or_downgraded_tokens_rejected() {
        let idp = idp();
        let other = idp_with_other_key();
        // Signed by another key under the same kid.
        assert!(verify_id_token(&config(), &idp.jwks, &token(&other, header(), claims(3600)), Utc::now()).is_err());

        // alg=none and HS256 are refused outright.
        let none = format!("{}.{}.", B64URL.encode(r#"{"alg":"none","kid":"k1"}"#), B64URL.encode(claims(3600).to_string()));
        assert!(verify_id_token(&config(), &idp.jwks, &none, Utc::now()).is_err());
        let hs = token(&idp, serde_json::json!({"alg": "HS256", "kid": "k1"}), claims(3600));
        assert!(verify_id_token(&config(), &idp.jwks, &hs, Utc::now()).is_err());

        // Edited claims break the signature.
        let good = token(&idp, header(), claims(3600));
        let parts: Vec<&str> = good.split('.').collect();
        let mut forged_claims = claims(3600);
        forged_claims["groups"] = serde_json::json!(["netgene-admins"]);
        let forged = format!("{}.{}.{}", parts[0], B64URL.encode(forged_claims.to_string()), parts[2]);
        assert!(verify_id_token(&config(), &idp.jwks, &forged, Utc::now()).is_err());

        // Unknown kid.
        let unknown = token(&idp, serde_json::json!({"alg": "ES256", "kid": "nope"}), claims(3600));
        assert!(verify_id_token(&config(), &idp.jwks, &unknown, Utc::now()).is_err());
    }

    fn idp_with_other_key() -> Idp {
        let mut other = idp();
        other.jwks = JwkSet { keys: vec![] };
        other
    }

    #[test]
    fn test_rs256_with_jwk_components() {
        use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};
        let key = RsaKeyPair::from_pkcs8(include_bytes!("../../netgene-presence/tests/fixtures/test-only-rsa2048.pk8.der")).unwrap();
        let components: RsaPublicKeyComponents<Vec<u8>> = key.public().into();
        let jwk = Jwk {
            kty: "RSA".into(),
            kid: Some("r1".into()),
            alg: Some("RS256".into()),
            n: Some(B64URL.encode(&components.n)),
            e: Some(B64URL.encode(&components.e)),
            crv: None,
            x: None,
            y: None,
        };
        let jwks = JwkSet { keys: vec![jwk] };
        let signed = format!("{}.{}", B64URL.encode(r#"{"alg":"RS256","kid":"r1"}"#), B64URL.encode(claims(600).to_string()));
        let mut sig = vec![0u8; key.public().modulus_len()];
        key.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), signed.as_bytes(), &mut sig).unwrap();
        let jwt = format!("{}.{}", signed, B64URL.encode(sig));
        assert_eq!(verify_id_token(&config(), &jwks, &jwt, Utc::now()).unwrap().subject, "user-123");
    }

    fn url_config() -> OidcConfig {
        OidcConfig { jwks: JwksSource::Url("https://idp.example.com/jwks".into()), ..config() }
    }

    #[tokio::test]
    async fn test_jwks_cache_fetches_once_then_on_rotation() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let dir = tempfile::tempdir().unwrap();
        let cache = JwksCache::new(dir.path().join("jwks-cache.json"));
        let idp = idp();
        let calls = AtomicUsize::new(0);
        let served = std::sync::Mutex::new(idp.jwks.clone());
        let fetch = |_url: String| {
            calls.fetch_add(1, Ordering::SeqCst);
            let jwks = served.lock().unwrap().clone();
            async move { Ok(jwks) }
        };
        let now = Utc::now();
        let tok = token(&idp, header(), claims(3600));
        cache.verify(&url_config(), &tok, now, fetch).await.unwrap();
        cache.verify(&url_config(), &tok, now + chrono::Duration::minutes(5), fetch).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 1, "second verification uses the cache");

        // The IdP rotates to a new key `k2`: the unknown kid forces one refetch.
        let mut rotated = super::tests::idp();
        rotated.jwks.keys[0].kid = Some("k2".into());
        *served.lock().unwrap() = rotated.jwks.clone();
        let tok2 = token(&rotated, serde_json::json!({"alg": "ES256", "kid": "k2"}), claims(3600));
        cache.verify(&url_config(), &tok2, now + chrono::Duration::minutes(6), fetch).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        // The old key is gone from the IdP, so tokens signed with it are refused.
        assert!(cache.verify(&url_config(), &tok, now + chrono::Duration::minutes(6), fetch).await.is_err());

        // Unknown kids right after a fetch do not trigger another one.
        let bogus = token(&rotated, serde_json::json!({"alg": "ES256", "kid": "zzz"}), claims(3600));
        assert!(cache.verify(&url_config(), &bogus, now + chrono::Duration::minutes(6), fetch).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_jwks_cache_survives_idp_outage_until_max_stale() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JwksCache::new(dir.path().join("jwks-cache.json"));
        let idp = idp();
        let jwks = idp.jwks.clone();
        let now = Utc::now();
        let tok = token(&idp, header(), claims(30 * 3600));
        cache.verify(&url_config(), &tok, now, |_| { let j = jwks.clone(); async move { Ok(j) } }).await.unwrap();

        let down = |_url: String| async { Err::<JwkSet, _>(anyhow::anyhow!("connection refused")) };
        cache.verify(&url_config(), &tok, now + chrono::Duration::hours(3), down).await.expect("stale cache used during outage");
        assert!(cache.verify(&url_config(), &tok, now + chrono::Duration::hours(25), down).await.is_err());
    }

    #[tokio::test]
    async fn test_jwks_url_must_be_https() {
        assert!(JwkSet::load(&JwksSource::Url("http://idp.example.com/jwks".into())).await.is_err());
    }
}
