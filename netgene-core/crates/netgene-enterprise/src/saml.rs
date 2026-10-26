//! SAML 2.0 (feature `saml`): bind an IdP-signed SAML response to NetGene
//! capabilities, like [`crate::verify_id_token`] does for OIDC.
//!
//! Signature checking is delegated to `samael` + xmlsec, which first reduces
//! the document to the parts covered by a valid signature (the defence against
//! XML signature wrapping) and then checks issuer, audience, recipient,
//! destination and validity times. On top of that this module:
//!
//! - **fails closed** when the IdP metadata has no signing certificate (samael
//!   would otherwise skip signature verification);
//! - accepts only SHA-256+ RSA/ECDSA signatures (no SHA-1);
//! - refuses a replayed assertion ID ([`ReplayCache`]), since responses are
//!   IdP-initiated (there is no AuthnRequest to match).
//!
//! Requires libxml2, xmlsec1 and OpenSSL; build with `--features saml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use samael::crypto::AllowedSignatureAlgorithm;
use samael::metadata::EntityDescriptor;
use samael::service_provider::ServiceProvider;
use serde::{Deserialize, Serialize};

use crate::VerifiedIdentity;

/// Used when an assertion carries no expiry of its own.
const DEFAULT_VALIDITY: Duration = Duration::minutes(5);

fn default_roles_attribute() -> String {
    "groups".into()
}

fn default_email_attributes() -> Vec<String> {
    vec!["mail".into(), "email".into(), "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress".into()]
}

fn default_max_ttl() -> i64 {
    480
}

/// `~/.netgene/saml.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SamlConfig {
    /// IdP metadata XML file (must contain a signing certificate).
    pub idp_metadata: PathBuf,
    /// This service provider's entity id (the expected Audience).
    pub sp_entity_id: String,
    /// Assertion consumer URL registered at the IdP (expected Recipient/Destination).
    pub acs_url: String,
    /// Attribute holding the user's groups or roles.
    #[serde(default = "default_roles_attribute")]
    pub roles_attribute: String,
    #[serde(default = "default_email_attributes")]
    pub email_attributes: Vec<String>,
    /// Group/role → NetGene capabilities.
    pub role_capabilities: BTreeMap<String, Vec<String>>,
    #[serde(default = "default_max_ttl")]
    pub max_ttl_minutes: i64,
}

impl SamlConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("Cannot read {}", path.display()))?;
        serde_json::from_str(&text).with_context(|| format!("Invalid SAML config {}", path.display()))
    }
}

/// Assertion IDs already used, kept until they expire (`~/.netgene/saml-replay.json`).
pub struct ReplayCache {
    path: Option<PathBuf>,
    seen: BTreeMap<String, DateTime<Utc>>,
}

impl ReplayCache {
    pub fn open(path: PathBuf) -> Result<Self> {
        let seen = match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).context("Invalid SAML replay cache")?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path: Some(path), seen })
    }

    pub fn in_memory() -> Self {
        Self { path: None, seen: BTreeMap::new() }
    }

    fn check_and_insert(&mut self, id: &str, expires_at: DateTime<Utc>, now: DateTime<Utc>) -> Result<()> {
        self.seen.retain(|_, exp| *exp > now);
        if self.seen.contains_key(id) {
            anyhow::bail!("SAML assertion {} was already used (replay)", id);
        }
        self.seen.insert(id.to_string(), expires_at);
        if let Some(path) = &self.path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, serde_json::to_vec_pretty(&self.seen)?)?;
        }
        Ok(())
    }
}

fn service_provider(config: &SamlConfig, metadata_xml: &str) -> Result<ServiceProvider> {
    let idp_metadata: EntityDescriptor = metadata_xml.parse().context("Invalid IdP metadata XML")?;
    let sp = ServiceProvider {
        entity_id: Some(config.sp_entity_id.clone()),
        metadata_url: None,
        acs_url: Some(config.acs_url.clone()),
        idp_metadata,
        allow_idp_initiated: true,
        allowed_signature_algorithms: Some(vec![
            AllowedSignatureAlgorithm::RsaSha256,
            AllowedSignatureAlgorithm::RsaSha384,
            AllowedSignatureAlgorithm::RsaSha512,
            AllowedSignatureAlgorithm::EcdsaSha256,
            AllowedSignatureAlgorithm::EcdsaSha384,
            AllowedSignatureAlgorithm::EcdsaSha512,
        ]),
        ..Default::default()
    };
    // Without a signing certificate samael would accept unsigned responses.
    match sp.idp_signing_certs().map_err(|e| anyhow::anyhow!("Invalid IdP certificate: {}", e))? {
        Some(certs) if !certs.is_empty() => Ok(sp),
        _ => anyhow::bail!("IdP metadata has no signing certificate; refusing to accept SAML responses"),
    }
}

/// Verify a base64 SAML response (HTTP-POST binding) and return the identity it asserts.
pub fn verify_saml_response(
    config: &SamlConfig,
    metadata_xml: &str,
    base64_response: &str,
    replay: &mut ReplayCache,
    now: DateTime<Utc>,
) -> Result<VerifiedIdentity> {
    let sp = service_provider(config, metadata_xml)?;
    let assertion = sp
        .parse_base64_response(base64_response.trim(), None)
        .map_err(|e| anyhow::anyhow!("SAML response rejected: {}", e))?;

    let subject = assertion
        .subject
        .as_ref()
        .and_then(|s| s.name_id.as_ref())
        .map(|n| n.value.trim().to_string())
        .filter(|v| !v.is_empty())
        .context("SAML assertion has no NameID")?;

    let confirmation_expiry = assertion
        .subject
        .as_ref()
        .and_then(|s| s.subject_confirmations.as_ref())
        .into_iter()
        .flatten()
        .filter_map(|c| c.subject_confirmation_data.as_ref().and_then(|d| d.not_on_or_after))
        .min();
    let condition_expiry = assertion.conditions.as_ref().and_then(|c| c.not_on_or_after);
    let expires_at = [confirmation_expiry, condition_expiry].into_iter().flatten().min().unwrap_or(now + DEFAULT_VALIDITY);
    if expires_at <= now {
        anyhow::bail!("SAML assertion has expired");
    }

    let attributes: Vec<_> = assertion.attribute_statements.iter().flatten().flat_map(|s| s.attributes.iter()).collect();
    let values = |name: &str| -> Vec<String> {
        attributes
            .iter()
            .filter(|a| a.name.as_deref() == Some(name) || a.friendly_name.as_deref() == Some(name))
            .flat_map(|a| a.values.iter().filter_map(|v| v.value.as_ref().map(|s| s.trim().to_string())))
            .filter(|v| !v.is_empty())
            .collect()
    };
    let roles = values(&config.roles_attribute);
    let email = config.email_attributes.iter().find_map(|a| values(a).into_iter().next());

    replay.check_and_insert(&assertion.id, expires_at, now)?;
    Ok(VerifiedIdentity { subject, email, roles, expires_at })
}

/// Capabilities granted by the identity's roles.
pub fn saml_capabilities_for(config: &SamlConfig, identity: &VerifiedIdentity) -> Vec<String> {
    crate::map_roles(&config.role_capabilities, &identity.roles)
}

/// Token lifetime: until the assertion expires, capped by config.
pub fn saml_grant_ttl(config: &SamlConfig, identity: &VerifiedIdentity, now: DateTime<Utc>) -> Duration {
    (identity.expires_at - now).min(Duration::minutes(config.max_ttl_minutes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use samael::crypto::{Crypto, CryptoProvider};

    const IDP: &str = "https://idp.example.com/saml";
    const SP: &str = "https://netgene.example.com/saml";
    const ACS: &str = "https://netgene.example.com/saml/acs";

    struct Idp {
        cert_b64: String,
        key_der: Vec<u8>,
    }

    fn idp() -> Idp {
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(vec!["idp.example.com".into()]).unwrap().self_signed(&key).unwrap();
        Idp { cert_b64: B64.encode(cert.der()), key_der: key.serialize_der() }
    }

    fn metadata(cert_b64: Option<&str>) -> String {
        let key = cert_b64
            .map(|c| format!(r#"<md:KeyDescriptor use="signing"><ds:KeyInfo xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:X509Data><ds:X509Certificate>{c}</ds:X509Certificate></ds:X509Data></ds:KeyInfo></md:KeyDescriptor>"#))
            .unwrap_or_default();
        format!(
            r#"<md:EntityDescriptor xmlns:md="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{IDP}"><md:IDPSSODescriptor protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">{key}<md:SingleSignOnService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-Redirect" Location="https://idp.example.com/sso"/></md:IDPSSODescriptor></md:EntityDescriptor>"#
        )
    }

    struct Claims<'a> {
        audience: &'a str,
        group: &'a str,
        not_before: DateTime<Utc>,
        expires: DateTime<Utc>,
    }

    fn claims() -> Claims<'static> {
        Claims { audience: SP, group: "netgene-operators", not_before: Utc::now() - Duration::minutes(1), expires: Utc::now() + Duration::minutes(10) }
    }

    fn ts(t: DateTime<Utc>) -> String {
        t.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    }

    fn assertion_xml(c: &Claims, id: &str) -> String {
        let now = ts(Utc::now());
        format!(
            r#"<saml:Assertion ID="{id}" Version="2.0" IssueInstant="{now}"><saml:Issuer>{IDP}</saml:Issuer><saml:Subject><saml:NameID Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress">ana@example.com</saml:NameID><saml:SubjectConfirmation Method="urn:oasis:names:tc:SAML:2.0:cm:bearer"><saml:SubjectConfirmationData NotOnOrAfter="{exp}" Recipient="{ACS}"/></saml:SubjectConfirmation></saml:Subject><saml:Conditions NotBefore="{nb}" NotOnOrAfter="{exp}"><saml:AudienceRestriction><saml:Audience>{aud}</saml:Audience></saml:AudienceRestriction></saml:Conditions><saml:AuthnStatement AuthnInstant="{now}"><saml:AuthnContext><saml:AuthnContextClassRef>urn:oasis:names:tc:SAML:2.0:ac:classes:Password</saml:AuthnContextClassRef></saml:AuthnContext></saml:AuthnStatement><saml:AttributeStatement><saml:Attribute Name="groups"><saml:AttributeValue>{group}</saml:AttributeValue></saml:Attribute><saml:Attribute Name="mail"><saml:AttributeValue>ana@example.com</saml:AttributeValue></saml:Attribute></saml:AttributeStatement></saml:Assertion>"#,
            exp = ts(c.expires),
            nb = ts(c.not_before),
            aud = c.audience,
            group = c.group,
        )
    }

    /// A Response signed (enveloped, exc-c14n, ECDSA-SHA256) by `idp`.
    fn signed_response(idp: &Idp, c: &Claims) -> String {
        let rid = format!("_r{}", uuid::Uuid::new_v4().simple());
        let aid = format!("_a{}", uuid::Uuid::new_v4().simple());
        let now = ts(Utc::now());
        let template = format!(
            r##"<?xml version="1.0"?><samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{rid}" Version="2.0" IssueInstant="{now}" Destination="{ACS}"><saml:Issuer>{IDP}</saml:Issuer><ds:Signature xmlns:ds="http://www.w3.org/2000/09/xmldsig#"><ds:SignedInfo><ds:CanonicalizationMethod Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"/><ds:SignatureMethod Algorithm="http://www.w3.org/2001/04/xmldsig-more#ecdsa-sha256"/><ds:Reference URI="#{rid}"><ds:Transforms><ds:Transform Algorithm="http://www.w3.org/2000/09/xmldsig#enveloped-signature"/><ds:Transform Algorithm="http://www.w3.org/2001/10/xml-exc-c14n#"/></ds:Transforms><ds:DigestMethod Algorithm="http://www.w3.org/2001/04/xmlenc#sha256"/><ds:DigestValue/></ds:Reference></ds:SignedInfo><ds:SignatureValue/></ds:Signature><samlp:Status><samlp:StatusCode Value="urn:oasis:names:tc:SAML:2.0:status:Success"/></samlp:Status>{assertion}</samlp:Response>"##,
            assertion = assertion_xml(c, &aid),
        );
        Crypto::sign_xml(template, &idp.key_der).unwrap()
    }

    fn config() -> SamlConfig {
        SamlConfig {
            idp_metadata: "unused".into(),
            sp_entity_id: SP.into(),
            acs_url: ACS.into(),
            roles_attribute: default_roles_attribute(),
            email_attributes: default_email_attributes(),
            role_capabilities: BTreeMap::from([("netgene-operators".into(), vec!["node.spawn".into(), "network.read".into()])]),
            max_ttl_minutes: 60,
        }
    }

    fn verify(meta: &str, xml: &str, replay: &mut ReplayCache) -> Result<VerifiedIdentity> {
        verify_saml_response(&config(), meta, &B64.encode(xml), replay, Utc::now())
    }

    #[test]
    fn test_valid_response_maps_roles_once() {
        let idp = idp();
        let meta = metadata(Some(&idp.cert_b64));
        let xml = signed_response(&idp, &claims());
        let mut replay = ReplayCache::in_memory();

        let id = verify(&meta, &xml, &mut replay).unwrap();
        assert_eq!(id.subject, "ana@example.com");
        assert_eq!(id.email.as_deref(), Some("ana@example.com"));
        assert_eq!(saml_capabilities_for(&config(), &id), vec!["network.read".to_string(), "node.spawn".to_string()]);
        assert!(saml_grant_ttl(&config(), &id, Utc::now()) <= Duration::minutes(10));

        assert!(verify(&meta, &xml, &mut replay).is_err(), "the same assertion cannot be used twice");
    }

    #[test]
    fn test_tampered_or_foreign_signatures_rejected() {
        let idp = idp();
        let meta = metadata(Some(&idp.cert_b64));
        let mut replay = ReplayCache::in_memory();

        let tampered = signed_response(&idp, &claims()).replace("netgene-operators", "netgene-admins");
        assert!(verify(&meta, &tampered, &mut replay).is_err(), "edited after signing");

        let other = self::idp();
        assert!(verify(&meta, &signed_response(&other, &claims()), &mut replay).is_err(), "signed by a key not in the metadata");

        let unsigned = signed_response(&idp, &claims());
        assert!(verify(&metadata(None), &unsigned, &mut replay).is_err(), "metadata without a certificate fails closed");
    }

    #[test]
    fn test_audience_and_time_checked() {
        let idp = idp();
        let meta = metadata(Some(&idp.cert_b64));
        let mut replay = ReplayCache::in_memory();
        let wrong_audience = Claims { audience: "https://other-sp.example.com", ..claims() };
        assert!(verify(&meta, &signed_response(&idp, &wrong_audience), &mut replay).is_err());
        let expired = Claims { expires: Utc::now() - Duration::minutes(10), not_before: Utc::now() - Duration::minutes(20), ..claims() };
        assert!(verify(&meta, &signed_response(&idp, &expired), &mut replay).is_err());
    }

    #[test]
    fn test_signature_wrapping_does_not_grant_forged_roles() {
        let idp = idp();
        let meta = metadata(Some(&idp.cert_b64));
        let genuine = signed_response(&idp, &claims());
        let genuine_body = genuine.trim_start_matches(r#"<?xml version="1.0"?>"#).trim();

        // Attacker puts a forged admin assertion first and hides the signed response inside.
        let forged = assertion_xml(&Claims { group: "netgene-admins", ..claims() }, "_forged");
        let wrapped = format!(
            r#"<?xml version="1.0"?><samlp:Response xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="_evil" Version="2.0" IssueInstant="{now}" Destination="{ACS}"><saml:Issuer>{IDP}</saml:Issuer><samlp:Status><samlp:StatusCode Value="urn:oasis:names:tc:SAML:2.0:status:Success"/></samlp:Status>{forged}<samlp:Extensions>{genuine_body}</samlp:Extensions></samlp:Response>"#,
            now = ts(Utc::now()),
        );
        match verify(&meta, &wrapped, &mut ReplayCache::in_memory()) {
            Err(_) => {}
            Ok(id) => assert!(!id.roles.iter().any(|r| r == "netgene-admins"), "forged roles must never be returned: {:?}", id.roles),
        }
    }

    #[test]
    fn test_replay_cache_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("saml-replay.json");
        let now = Utc::now();
        ReplayCache::open(path.clone()).unwrap().check_and_insert("_a1", now + Duration::minutes(5), now).unwrap();
        assert!(ReplayCache::open(path.clone()).unwrap().check_and_insert("_a1", now + Duration::minutes(5), now).is_err());
        // Expired entries are forgotten.
        ReplayCache::open(path).unwrap().check_and_insert("_a1", now + Duration::minutes(15), now + Duration::minutes(10)).unwrap();
    }
}
