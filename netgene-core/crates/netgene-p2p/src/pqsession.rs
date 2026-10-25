//! Post-quantum private sessions between mesh nodes.
//!
//! libp2p's Noise transport uses classical X25519 only, so traffic recorded
//! today could be decrypted by a future quantum computer. For private
//! (point-to-point) messages, nodes run an extra hybrid key exchange on top of
//! the authenticated connection:
//!
//! 1. **Init** (initiator → responder): ephemeral X25519 public key and an
//!    ephemeral ML-KEM-768 encapsulation key, hybrid-signed by the initiator's Gene.
//! 2. **Resp** (responder → initiator): ephemeral X25519 public key and the
//!    ML-KEM ciphertext, hybrid-signed by the responder's Gene over the whole
//!    transcript.
//!
//! Session keys = HKDF-SHA256(X25519 secret ‖ ML-KEM secret, salt = SHA3(transcript)).
//! An attacker must break **both** X25519 and ML-KEM to read the messages.
//! Frames use ChaCha20-Poly1305 with a per-direction counter (replay-proof).

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use libp2p::PeerId;
use ml_kem::{kem::{Decapsulate, Encapsulate, Kem}, Ciphertext, DecapsulationKey, EncapsulationKey, KeyExport, MlKem768, TryKeyInit};
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::agreement::{self, EphemeralPrivateKey, UnparsedPublicKey, X25519};
use ring::hkdf;
use ring::rand::SystemRandom;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use uuid::Uuid;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature};

use crate::identity::peer_id_for_key;

const INIT_DOMAIN: &[u8] = b"netgene-pq-session-init-v1\0";
const RESP_DOMAIN: &[u8] = b"netgene-pq-session-resp-v1\0";
const KDF_INFO_I2R: &[u8] = b"netgene-pq-session-v1 initiator->responder";
const KDF_INFO_R2I: &[u8] = b"netgene-pq-session-v1 responder->initiator";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeInit {
    pub session_id: Uuid,
    pub x25519_b64: String,
    pub mlkem768_ek_b64: String,
    pub initiator_key: HybridPublicKey,
    pub signature: HybridSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResp {
    pub session_id: Uuid,
    pub x25519_b64: String,
    pub mlkem768_ct_b64: String,
    pub responder_key: HybridPublicKey,
    pub signature: HybridSignature,
}

/// An encrypted private message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedFrame {
    pub session_id: Uuid,
    pub counter: u64,
    pub ciphertext_b64: String,
}

/// Initiator state between sending Init and receiving Resp.
pub struct PendingInitiator {
    init: HandshakeInit,
    x25519: EphemeralPrivateKey,
    mlkem_dk: DecapsulationKey<MlKem768>,
}

/// An established session with one peer.
pub struct Session {
    pub session_id: Uuid,
    pub peer_key: HybridPublicKey,
    send: LessSafeKey,
    recv: LessSafeKey,
    send_counter: u64,
    /// Highest counter accepted so far (frames must be strictly increasing).
    recv_counter: Option<u64>,
}

fn init_transcript(session_id: Uuid, x25519: &str, ek: &str, key: &HybridPublicKey) -> Result<Vec<u8>> {
    let mut t = INIT_DOMAIN.to_vec();
    t.extend_from_slice(session_id.as_bytes());
    for field in [x25519.as_bytes(), ek.as_bytes()] {
        t.extend_from_slice(&(field.len() as u32).to_be_bytes());
        t.extend_from_slice(field);
    }
    t.extend_from_slice(&key.encode()?);
    Ok(t)
}

fn resp_transcript(init: &HandshakeInit, x25519: &str, ct: &str, key: &HybridPublicKey) -> Result<Vec<u8>> {
    let mut t = RESP_DOMAIN.to_vec();
    t.extend_from_slice(&init_transcript(init.session_id, &init.x25519_b64, &init.mlkem768_ek_b64, &init.initiator_key)?);
    for field in [x25519.as_bytes(), ct.as_bytes()] {
        t.extend_from_slice(&(field.len() as u32).to_be_bytes());
        t.extend_from_slice(field);
    }
    t.extend_from_slice(&key.encode()?);
    Ok(t)
}

fn derive_key(prk: &hkdf::Prk, info: &[u8]) -> Result<LessSafeKey> {
    let info = [info];
    let okm = prk.expand(&info, &CHACHA20_POLY1305).map_err(|_| anyhow::anyhow!("HKDF expand failed"))?;
    Ok(LessSafeKey::new(UnboundKey::from(okm)))
}

/// Derive (initiator→responder, responder→initiator) keys.
fn derive_keys(x25519_secret: &[u8], mlkem_secret: &[u8], transcript: &[u8]) -> Result<(LessSafeKey, LessSafeKey)> {
    let mut ikm = Vec::with_capacity(x25519_secret.len() + mlkem_secret.len());
    ikm.extend_from_slice(x25519_secret);
    ikm.extend_from_slice(mlkem_secret);
    let transcript_hash = Sha3_256::digest(transcript);
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, &transcript_hash);
    let prk = salt.extract(&ikm);
    Ok((derive_key(&prk, KDF_INFO_I2R)?, derive_key(&prk, KDF_INFO_R2I)?))
}

fn x25519_agree(private: EphemeralPrivateKey, peer_public: &[u8]) -> Result<Vec<u8>> {
    agreement::agree_ephemeral(private, &UnparsedPublicKey::new(&X25519, peer_public), |s| s.to_vec())
        .map_err(|_| anyhow::anyhow!("X25519 agreement failed"))
}

fn check_peer(key: &HybridPublicKey, expected: &PeerId) -> Result<()> {
    if peer_id_for_key(key)? != *expected {
        anyhow::bail!("Handshake key does not belong to the connected peer {}", expected);
    }
    if !key.is_post_quantum() {
        anyhow::bail!("Peer {} has no ML-DSA key; post-quantum sessions need hybrid genes", expected);
    }
    Ok(())
}

/// Start a handshake as initiator.
pub fn initiate(me: &GeneKeyPair) -> Result<(PendingInitiator, HandshakeInit)> {
    let rng = SystemRandom::new();
    let x25519 = EphemeralPrivateKey::generate(&X25519, &rng).map_err(|_| anyhow::anyhow!("X25519 keygen failed"))?;
    let x25519_pub = x25519.compute_public_key().map_err(|_| anyhow::anyhow!("X25519 public key failed"))?;
    let (mlkem_dk, mlkem_ek) = MlKem768::generate_keypair();

    let session_id = Uuid::new_v4();
    let x25519_b64 = BASE64.encode(x25519_pub.as_ref());
    let mlkem768_ek_b64 = BASE64.encode(mlkem_ek.to_bytes());
    let initiator_key = me.hybrid_public_key();
    let signature = me.sign_hybrid(&init_transcript(session_id, &x25519_b64, &mlkem768_ek_b64, &initiator_key)?)?;

    let init = HandshakeInit { session_id, x25519_b64, mlkem768_ek_b64, initiator_key, signature };
    Ok((PendingInitiator { init: init.clone(), x25519, mlkem_dk }, init))
}

/// Answer an Init from `peer`, returning the established session and the Resp.
pub fn respond(me: &GeneKeyPair, init: &HandshakeInit, peer: &PeerId) -> Result<(Session, HandshakeResp)> {
    check_peer(&init.initiator_key, peer)?;
    let transcript = init_transcript(init.session_id, &init.x25519_b64, &init.mlkem768_ek_b64, &init.initiator_key)?;
    if !verify_hybrid(&init.initiator_key, &transcript, &init.signature) {
        anyhow::bail!("Invalid signature on handshake init from {}", peer);
    }

    let ek_bytes = BASE64.decode(&init.mlkem768_ek_b64)?;
    let ek = EncapsulationKey::<MlKem768>::new_from_slice(&ek_bytes).map_err(|_| anyhow::anyhow!("Invalid ML-KEM key"))?;
    let (ct, mlkem_secret) = ek.encapsulate();

    let rng = SystemRandom::new();
    let x25519 = EphemeralPrivateKey::generate(&X25519, &rng).map_err(|_| anyhow::anyhow!("X25519 keygen failed"))?;
    let x25519_b64 = BASE64.encode(x25519.compute_public_key().map_err(|_| anyhow::anyhow!("X25519 public key failed"))?.as_ref());
    let x25519_secret = x25519_agree(x25519, &BASE64.decode(&init.x25519_b64)?)?;

    let mlkem768_ct_b64 = BASE64.encode(ct.as_slice());
    let responder_key = me.hybrid_public_key();
    let transcript = resp_transcript(init, &x25519_b64, &mlkem768_ct_b64, &responder_key)?;
    let signature = me.sign_hybrid(&transcript)?;

    let (i2r, r2i) = derive_keys(&x25519_secret, mlkem_secret.as_slice(), &transcript)?;
    let session = Session {
        session_id: init.session_id,
        peer_key: init.initiator_key.clone(),
        send: r2i,
        recv: i2r,
        send_counter: 0,
        recv_counter: None,
    };
    Ok((session, HandshakeResp { session_id: init.session_id, x25519_b64, mlkem768_ct_b64, responder_key, signature }))
}

/// Finish the handshake as initiator.
pub fn complete(pending: PendingInitiator, resp: &HandshakeResp, peer: &PeerId) -> Result<Session> {
    if resp.session_id != pending.init.session_id {
        anyhow::bail!("Handshake response is for another session");
    }
    check_peer(&resp.responder_key, peer)?;
    let transcript = resp_transcript(&pending.init, &resp.x25519_b64, &resp.mlkem768_ct_b64, &resp.responder_key)?;
    if !verify_hybrid(&resp.responder_key, &transcript, &resp.signature) {
        anyhow::bail!("Invalid signature on handshake response from {}", peer);
    }

    let ct_bytes = BASE64.decode(&resp.mlkem768_ct_b64)?;
    let ct = Ciphertext::<MlKem768>::try_from(ct_bytes.as_slice()).map_err(|_| anyhow::anyhow!("Invalid ML-KEM ciphertext"))?;
    let mlkem_secret = pending.mlkem_dk.decapsulate(&ct);
    let x25519_secret = x25519_agree(pending.x25519, &BASE64.decode(&resp.x25519_b64)?)?;

    let (i2r, r2i) = derive_keys(&x25519_secret, mlkem_secret.as_slice(), &transcript)?;
    Ok(Session {
        session_id: resp.session_id,
        peer_key: resp.responder_key.clone(),
        send: i2r,
        recv: r2i,
        send_counter: 0,
        recv_counter: None,
    })
}

fn nonce(counter: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    Nonce::assume_unique_for_key(bytes)
}

fn frame_aad(session_id: Uuid, counter: u64) -> [u8; 24] {
    let mut aad = [0u8; 24];
    aad[..16].copy_from_slice(session_id.as_bytes());
    aad[16..].copy_from_slice(&counter.to_be_bytes());
    aad
}

impl Session {
    /// Encrypt a message for the peer.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<SealedFrame> {
        let counter = self.send_counter;
        self.send_counter = self.send_counter.checked_add(1).context("Session counter exhausted")?;
        let mut buf = plaintext.to_vec();
        self.send
            .seal_in_place_append_tag(nonce(counter), Aad::from(frame_aad(self.session_id, counter)), &mut buf)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        Ok(SealedFrame { session_id: self.session_id, counter, ciphertext_b64: BASE64.encode(buf) })
    }

    /// Decrypt a frame from the peer; rejects replays and reordering.
    pub fn open(&mut self, frame: &SealedFrame) -> Result<Vec<u8>> {
        if frame.session_id != self.session_id {
            anyhow::bail!("Frame belongs to another session");
        }
        if self.recv_counter.is_some_and(|last| frame.counter <= last) {
            anyhow::bail!("Replayed or out-of-order frame {}", frame.counter);
        }
        let mut buf = BASE64.decode(&frame.ciphertext_b64)?;
        let plain = self
            .recv
            .open_in_place(nonce(frame.counter), Aad::from(frame_aad(self.session_id, frame.counter)), &mut buf)
            .map_err(|_| anyhow::anyhow!("Decryption failed (tampered frame or wrong key)"))?
            .to_vec();
        self.recv_counter = Some(frame.counter);
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::libp2p_keypair_from_gene;
    use netgene_gene::NetGene;

    fn node() -> (GeneKeyPair, PeerId) {
        let (_, kp) = NetGene::generate_master("node").unwrap();
        let peer = PeerId::from(libp2p_keypair_from_gene(&kp).unwrap().public());
        (kp, peer)
    }

    fn handshake() -> (Session, Session) {
        let (a, a_peer) = node();
        let (b, b_peer) = node();
        let (pending, init) = initiate(&a).unwrap();
        let (b_session, resp) = respond(&b, &init, &a_peer).unwrap();
        let a_session = complete(pending, &resp, &b_peer).unwrap();
        (a_session, b_session)
    }

    #[test]
    fn test_both_directions_roundtrip() {
        let (mut a, mut b) = handshake();
        let f = a.seal(b"hello b").unwrap();
        assert_eq!(b.open(&f).unwrap(), b"hello b");
        let g = b.seal(b"hello a").unwrap();
        assert_eq!(a.open(&g).unwrap(), b"hello a");
    }

    #[test]
    fn test_replay_and_tamper_rejected() {
        let (mut a, mut b) = handshake();
        let f1 = a.seal(b"one").unwrap();
        let f2 = a.seal(b"two").unwrap();
        b.open(&f2).unwrap();
        assert!(b.open(&f1).is_err(), "older frame after a newer one is a replay");
        assert!(b.open(&f2).is_err(), "same frame twice is a replay");

        let mut f3 = a.seal(b"three").unwrap();
        let mut bytes = BASE64.decode(&f3.ciphertext_b64).unwrap();
        bytes[0] ^= 1;
        f3.ciphertext_b64 = BASE64.encode(bytes);
        assert!(b.open(&f3).is_err());
    }

    #[test]
    fn test_wrong_peer_identity_rejected() {
        let (a, _) = node();
        let (b, _) = node();
        let (_, someone_else) = node();
        let (_, init) = initiate(&a).unwrap();
        assert!(respond(&b, &init, &someone_else).is_err(), "init must come from the connected peer");
    }

    #[test]
    fn test_tampered_handshake_rejected() {
        let (a, a_peer) = node();
        let (b, b_peer) = node();

        let (_, mut init) = initiate(&a).unwrap();
        let (other, _) = initiate(&a).unwrap();
        init.mlkem768_ek_b64 = other.init.mlkem768_ek_b64.clone();
        assert!(respond(&b, &init, &a_peer).is_err(), "swapped ML-KEM key breaks the signature");

        let (pending, init) = initiate(&a).unwrap();
        let (_, mut resp) = respond(&b, &init, &a_peer).unwrap();
        let (_, other_resp) = respond(&b, &init, &a_peer).unwrap();
        resp.mlkem768_ct_b64 = other_resp.mlkem768_ct_b64;
        assert!(complete(pending, &resp, &b_peer).is_err(), "swapped ciphertext breaks the signature");
    }

    #[test]
    fn test_sessions_have_independent_keys() {
        let (mut a1, _) = handshake();
        let (_, mut b2) = handshake();
        let frame = a1.seal(b"secret").unwrap();
        let mut forged = frame.clone();
        forged.session_id = b2.session_id;
        assert!(b2.open(&forged).is_err());
    }
}
