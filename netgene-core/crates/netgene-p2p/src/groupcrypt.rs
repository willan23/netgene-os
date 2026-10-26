//! Confidential gossip with per-author "sender keys".
//!
//! Gossipsub messages are signed by their author, but on their own they travel
//! in clear inside a classical (X25519) Noise transport. Here every node owns a
//! random 256-bit *sender key* and encrypts its broadcasts with it
//! (ChaCha20-Poly1305, AAD = author PeerId ‖ key id ‖ counter). The key is
//! handed to other members **only inside the hybrid X25519 + ML-KEM-768
//! sessions** of [`crate::pqsession`], so recorded gossip stays confidential
//! against a future quantum computer.
//!
//! A key travels as a [`SenderKeyAnnouncement`] hybrid-signed by its author.
//! Members forward announcements to peers the author is not directly connected
//! to (multi-hop meshes); a forwarder can relay a key but cannot forge one. Keys
//! rotate every [`ROTATE_AFTER`] and are dropped after [`KEY_MAX_AGE`].

use std::collections::{HashMap, HashSet, VecDeque};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Duration, Utc};
use libp2p::PeerId;
use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use uuid::Uuid;
use zeroize::Zeroizing;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature};

use crate::identity::peer_id_for_key;

const ANNOUNCE_DOMAIN: &[u8] = b"netgene-sender-key-v1\0";
const GOSSIP_AAD_DOMAIN: &[u8] = b"netgene-gossip-v2\0";
/// A node starts a new sender key after this long.
pub const ROTATE_AFTER: Duration = Duration::hours(1);
/// Keys older than this are no longer accepted for decryption.
pub const KEY_MAX_AGE: Duration = Duration::hours(3);
/// Also rotate well before nonce reuse could become a concern.
const ROTATE_AFTER_MESSAGES: u64 = 1 << 32;
const MAX_KEYS_PER_AUTHOR: usize = 4;
const SEEN_CAPACITY: usize = 8192;

/// A sender key, as handed to other members over a PQ session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SenderKeyAnnouncement {
    pub author_key: HybridPublicKey,
    pub key_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub key_b64: String,
    /// Hybrid signature over (domain, key id, created_at, SHA3-256(key)).
    pub signature: HybridSignature,
}

/// Payload of a gossipsub message on the v2 topic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedGossip {
    pub key_id: Uuid,
    pub counter: u64,
    pub ciphertext_b64: String,
}

fn announce_transcript(key_id: Uuid, created_at: DateTime<Utc>, key: &[u8], author: &HybridPublicKey) -> Result<Vec<u8>> {
    let mut t = ANNOUNCE_DOMAIN.to_vec();
    t.extend_from_slice(key_id.as_bytes());
    t.extend_from_slice(&created_at.timestamp_micros().to_be_bytes());
    t.extend_from_slice(&Sha3_256::digest(key));
    t.extend_from_slice(&author.encode()?);
    Ok(t)
}

fn gossip_aad(author: &PeerId, key_id: Uuid, counter: u64) -> Vec<u8> {
    let mut aad = GOSSIP_AAD_DOMAIN.to_vec();
    aad.extend_from_slice(&author.to_bytes());
    aad.extend_from_slice(key_id.as_bytes());
    aad.extend_from_slice(&counter.to_be_bytes());
    aad
}

fn nonce(counter: u64) -> Nonce {
    let mut bytes = [0u8; 12];
    bytes[4..].copy_from_slice(&counter.to_be_bytes());
    Nonce::assume_unique_for_key(bytes)
}

fn aead_key(bytes: &[u8]) -> Result<LessSafeKey> {
    let unbound = UnboundKey::new(&CHACHA20_POLY1305, bytes).map_err(|_| anyhow::anyhow!("Invalid sender key"))?;
    Ok(LessSafeKey::new(unbound))
}

struct OwnKey {
    announcement: SenderKeyAnnouncement,
    cipher: LessSafeKey,
    counter: u64,
}

struct KnownKey {
    announcement: SenderKeyAnnouncement,
    cipher: LessSafeKey,
}

impl OwnKey {
    fn generate(me: &GeneKeyPair, now: DateTime<Utc>) -> Result<Self> {
        let mut key = Zeroizing::new([0u8; 32]);
        SystemRandom::new().fill(key.as_mut()).map_err(|_| anyhow::anyhow!("RNG failure"))?;
        let key_id = Uuid::new_v4();
        let author_key = me.hybrid_public_key();
        let signature = me.sign_hybrid(&announce_transcript(key_id, now, key.as_ref(), &author_key)?)?;
        let announcement = SenderKeyAnnouncement { author_key, key_id, created_at: now, key_b64: BASE64.encode(key.as_ref()), signature };
        Ok(Self { cipher: aead_key(key.as_ref())?, announcement, counter: 0 })
    }
}

/// Sender-key state of one node.
pub struct GroupCrypto {
    peer_id: PeerId,
    own: OwnKey,
    known: HashMap<PeerId, VecDeque<KnownKey>>,
    seen: HashSet<(Uuid, u64)>,
    seen_order: VecDeque<(Uuid, u64)>,
}

impl GroupCrypto {
    pub fn new(me: &GeneKeyPair, now: DateTime<Utc>) -> Result<Self> {
        let peer_id = peer_id_for_key(&me.hybrid_public_key())?;
        Ok(Self { peer_id, own: OwnKey::generate(me, now)?, known: HashMap::new(), seen: HashSet::new(), seen_order: VecDeque::new() })
    }

    /// The current own key, to send to a peer.
    pub fn own_announcement(&self) -> &SenderKeyAnnouncement {
        &self.own.announcement
    }

    /// Every announcement worth giving to a newly connected peer (own + forwarded).
    pub fn all_announcements(&self, now: DateTime<Utc>) -> Vec<SenderKeyAnnouncement> {
        let mut all = vec![self.own.announcement.clone()];
        for keys in self.known.values() {
            all.extend(keys.iter().filter(|k| now - k.announcement.created_at < KEY_MAX_AGE).map(|k| k.announcement.clone()));
        }
        all
    }

    /// Start a new own key when the current one is old; returns it so the caller can distribute it.
    pub fn rotate_if_due(&mut self, me: &GeneKeyPair, now: DateTime<Utc>) -> Result<Option<SenderKeyAnnouncement>> {
        if now - self.own.announcement.created_at < ROTATE_AFTER && self.own.counter < ROTATE_AFTER_MESSAGES {
            return Ok(None);
        }
        self.own = OwnKey::generate(me, now)?;
        Ok(Some(self.own.announcement.clone()))
    }

    /// Encrypt a broadcast authored by this node.
    pub fn seal(&mut self, plaintext: &[u8]) -> Result<EncryptedGossip> {
        let counter = self.own.counter;
        self.own.counter = counter.checked_add(1).context("Sender key counter exhausted")?;
        let key_id = self.own.announcement.key_id;
        let mut buf = plaintext.to_vec();
        self.own
            .cipher
            .seal_in_place_append_tag(nonce(counter), Aad::from(gossip_aad(&self.peer_id, key_id, counter)), &mut buf)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;
        Ok(EncryptedGossip { key_id, counter, ciphertext_b64: BASE64.encode(buf) })
    }

    /// Verify and store an announcement. Returns `true` if it is new (worth forwarding).
    pub fn accept_announcement(&mut self, ann: &SenderKeyAnnouncement, now: DateTime<Utc>) -> Result<bool> {
        let author = peer_id_for_key(&ann.author_key)?;
        if author == self.peer_id {
            return Ok(false);
        }
        if !ann.author_key.is_post_quantum() {
            anyhow::bail!("Sender key from {} is not bound to a hybrid (post-quantum) Gene", author);
        }
        if now - ann.created_at >= KEY_MAX_AGE || ann.created_at - now > Duration::minutes(5) {
            anyhow::bail!("Sender key from {} is expired or from the future", author);
        }
        let key = Zeroizing::new(BASE64.decode(&ann.key_b64)?);
        if key.len() != 32 {
            anyhow::bail!("Sender key from {} has the wrong length", author);
        }
        if !verify_hybrid(&ann.author_key, &announce_transcript(ann.key_id, ann.created_at, &key, &ann.author_key)?, &ann.signature) {
            anyhow::bail!("Invalid signature on sender key from {}", author);
        }
        let keys = self.known.entry(author).or_default();
        if keys.iter().any(|k| k.announcement.key_id == ann.key_id) {
            return Ok(false);
        }
        keys.push_back(KnownKey { cipher: aead_key(&key)?, announcement: ann.clone() });
        while keys.len() > MAX_KEYS_PER_AUTHOR {
            keys.pop_front();
        }
        Ok(true)
    }

    /// Whether a key for this author/key id is known (otherwise the caller may buffer the message).
    pub fn has_key(&self, author: &PeerId, key_id: Uuid) -> bool {
        self.known.get(author).is_some_and(|keys| keys.iter().any(|k| k.announcement.key_id == key_id))
    }

    /// Decrypt a broadcast whose gossipsub signature names `author`.
    pub fn open(&mut self, author: &PeerId, msg: &EncryptedGossip, now: DateTime<Utc>) -> Result<Vec<u8>> {
        let key = self
            .known
            .get(author)
            .and_then(|keys| keys.iter().find(|k| k.announcement.key_id == msg.key_id))
            .with_context(|| format!("No sender key {} for {}", msg.key_id, author))?;
        if now - key.announcement.created_at >= KEY_MAX_AGE {
            anyhow::bail!("Sender key {} of {} has expired", msg.key_id, author);
        }
        if self.seen.contains(&(msg.key_id, msg.counter)) {
            anyhow::bail!("Replayed gossip {}:{} from {}", msg.key_id, msg.counter, author);
        }
        let mut buf = BASE64.decode(&msg.ciphertext_b64)?;
        let plain = key
            .cipher
            .open_in_place(nonce(msg.counter), Aad::from(gossip_aad(author, msg.key_id, msg.counter)), &mut buf)
            .map_err(|_| anyhow::anyhow!("Gossip decryption failed (tampered, or not from {})", author))?
            .to_vec();
        self.seen.insert((msg.key_id, msg.counter));
        self.seen_order.push_back((msg.key_id, msg.counter));
        if self.seen_order.len() > SEEN_CAPACITY {
            if let Some(old) = self.seen_order.pop_front() {
                self.seen.remove(&old);
            }
        }
        Ok(plain)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netgene_gene::NetGene;

    fn member() -> (GeneKeyPair, PeerId) {
        let (_, kp) = NetGene::generate_master("node").unwrap();
        let peer = peer_id_for_key(&kp.hybrid_public_key()).unwrap();
        (kp, peer)
    }

    #[test]
    fn test_broadcast_roundtrip_and_forwarding() {
        let now = Utc::now();
        let (a, a_peer) = member();
        let (b, _) = member();
        let (c, _) = member();
        let mut ga = GroupCrypto::new(&a, now).unwrap();
        let mut gb = GroupCrypto::new(&b, now).unwrap();
        let mut gc = GroupCrypto::new(&c, now).unwrap();

        // A gives its key to B; B forwards it to C (A and C are not connected).
        assert!(gb.accept_announcement(ga.own_announcement(), now).unwrap());
        assert!(!gb.accept_announcement(ga.own_announcement(), now).unwrap(), "second copy is not new");
        for ann in gb.all_announcements(now) {
            gc.accept_announcement(&ann, now).unwrap();
        }

        let msg = ga.seal(b"hello mesh").unwrap();
        assert!(!msg.ciphertext_b64.contains("hello"));
        assert_eq!(gb.open(&a_peer, &msg, now).unwrap(), b"hello mesh");
        assert_eq!(gc.open(&a_peer, &msg, now).unwrap(), b"hello mesh");
        assert!(gb.open(&a_peer, &msg, now).is_err(), "replay rejected");
    }

    #[test]
    fn test_misattributed_or_tampered_gossip_rejected() {
        let now = Utc::now();
        let (a, a_peer) = member();
        let (b, b_peer) = member();
        let mut ga = GroupCrypto::new(&a, now).unwrap();
        let mut gb = GroupCrypto::new(&b, now).unwrap();
        gb.accept_announcement(ga.own_announcement(), now).unwrap();

        let msg = ga.seal(b"x").unwrap();
        assert!(gb.open(&b_peer, &msg, now).is_err(), "AAD binds the author");
        let mut tampered = ga.seal(b"y").unwrap();
        tampered.counter += 1;
        assert!(gb.open(&a_peer, &tampered, now).is_err());
    }

    #[test]
    fn test_forged_and_stale_announcements_rejected() {
        let now = Utc::now();
        let (a, _) = member();
        let (b, _) = member();
        let (m, _) = member();
        let ga = GroupCrypto::new(&a, now).unwrap();
        let mut gb = GroupCrypto::new(&b, now).unwrap();

        // A forwarder swaps in its own key under A's name.
        let mallory = GroupCrypto::new(&m, now).unwrap();
        let mut forged = ga.own_announcement().clone();
        forged.key_b64 = mallory.own_announcement().key_b64.clone();
        assert!(gb.accept_announcement(&forged, now).is_err());

        let old = GroupCrypto::new(&a, now - KEY_MAX_AGE - Duration::minutes(1)).unwrap();
        assert!(gb.accept_announcement(old.own_announcement(), now).is_err());
    }

    #[test]
    fn test_rotation_and_expiry() {
        let now = Utc::now();
        let (a, a_peer) = member();
        let (b, _) = member();
        let mut ga = GroupCrypto::new(&a, now).unwrap();
        let mut gb = GroupCrypto::new(&b, now).unwrap();
        gb.accept_announcement(ga.own_announcement(), now).unwrap();
        let before = ga.seal(b"old key").unwrap();

        assert!(ga.rotate_if_due(&a, now + Duration::minutes(10)).unwrap().is_none());
        let later = now + ROTATE_AFTER;
        let new = ga.rotate_if_due(&a, later).unwrap().expect("rotated");
        gb.accept_announcement(&new, later).unwrap();
        let after = ga.seal(b"new key").unwrap();
        assert_ne!(before.key_id, after.key_id);
        assert_eq!(gb.open(&a_peer, &after, later).unwrap(), b"new key");
        assert_eq!(gb.open(&a_peer, &before, later).unwrap(), b"old key", "previous key still valid for a while");
        let mut late = before.clone();
        late.counter = 99;
        assert!(gb.open(&a_peer, &late, now + KEY_MAX_AGE).is_err(), "expired key");
    }
}
