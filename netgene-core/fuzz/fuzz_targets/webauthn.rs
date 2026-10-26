//! WebAuthn registration (CBOR attestation object, authenticator data, COSE
//! key) and assertion parsing with attacker-controlled bytes.
#![no_main]

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use libfuzzer_sys::fuzz_target;
use netgene_presence::webauthn::{self, AssertionResponse, PasskeyCredential, RegistrationResponse, LOCAL_RP_ID};

const ORIGIN: &str = "http://localhost:4000";
const CHALLENGE: [u8; 32] = [7; 32];

fn client_data(kind: &str) -> String {
    let json = format!(r#"{{"type":"{}","challenge":"{}","origin":"{}"}}"#, kind, B64URL.encode(CHALLENGE), ORIGIN);
    B64URL.encode(json)
}

fuzz_target!(|data: &[u8]| {
    let Some((&mode, rest)) = data.split_first() else { return };
    if mode % 2 == 0 {
        // Valid client data, so parsing reaches the attestation object.
        let response = RegistrationResponse { client_data_json: client_data("webauthn.create"), attestation_object: B64URL.encode(rest) };
        let _ = webauthn::verify_registration(uuid::Uuid::nil(), &response, &CHALLENGE, LOCAL_RP_ID, ORIGIN);
    } else {
        let split = rest.first().map(|&n| n as usize).unwrap_or(0).min(rest.len().saturating_sub(1));
        let (auth, sig) = rest.get(1..).unwrap_or_default().split_at(split.min(rest.len().saturating_sub(1)));
        for alg in [-7, -8, -257] {
            let credential = PasskeyCredential {
                gene_id: uuid::Uuid::nil(),
                credential_id_b64: B64URL.encode([1u8; 16]),
                alg,
                public_key_b64: B64URL.encode([4u8; 65]),
                rsa_exponent_b64: Some(B64URL.encode([1, 0, 1])),
                rp_id: LOCAL_RP_ID.into(),
                sign_count: 0,
                enrolled_at: chrono::Utc::now(),
            };
            let response = AssertionResponse {
                credential_id: B64URL.encode([1u8; 16]),
                client_data_json: client_data("webauthn.get"),
                authenticator_data: B64URL.encode(auth),
                signature: B64URL.encode(sig),
            };
            let _ = webauthn::verify_assertion(&[credential], &response, &CHALLENGE, ORIGIN);
        }
    }
});
