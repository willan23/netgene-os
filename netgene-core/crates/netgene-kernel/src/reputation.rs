//! Marketplace reputation from signed reviews.
//!
//! A review is hybrid-signed by the reviewer's Gene and bound to the exact
//! manifest it rates (SHA3 of the manifest's signed payload), so a rating of
//! v1.0 never carries over to a different v1.1.
//!
//! Anyone can create Genes, so counting every review would be trivially
//! gamed (Sybil attack). The score only counts reviewers the local user
//! trusts (normally their own lineage); everything else is reported apart.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use uuid::Uuid;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature, NetGene};

use crate::marketplace::{validate_agent_id, AgentManifest};

const SIGNING_DOMAIN: &[u8] = b"netgene-agent-review-v1\0";
const MAX_COMMENT_CHARS: usize = 500;
/// Bayesian prior: the score starts at `PRIOR_MEAN` as if `PRIOR_WEIGHT` reviews existed.
const PRIOR_MEAN: f64 = 3.0;
const PRIOR_WEIGHT: f64 = 2.0;

/// Hex SHA3-256 of a manifest's signed payload (identifies one exact version).
pub fn manifest_digest(manifest: &AgentManifest) -> String {
    hex::encode(Sha3_256::digest(manifest.signing_payload()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentReview {
    pub review_id: Uuid,
    pub agent_id: String,
    pub agent_version: String,
    pub manifest_digest: String,
    pub reviewer_gene_id: Uuid,
    /// 1 to 5 stars.
    pub rating: u8,
    pub comment: String,
    pub created_at: DateTime<Utc>,
    pub reviewer_key: HybridPublicKey,
    pub signature: HybridSignature,
}

#[derive(Serialize)]
struct ReviewBody<'a> {
    review_id: Uuid,
    agent_id: &'a str,
    agent_version: &'a str,
    manifest_digest: &'a str,
    reviewer_gene_id: Uuid,
    rating: u8,
    comment: &'a str,
    created_at: &'a DateTime<Utc>,
}

impl AgentReview {
    fn payload(&self) -> Result<Vec<u8>> {
        let body = ReviewBody {
            review_id: self.review_id,
            agent_id: &self.agent_id,
            agent_version: &self.agent_version,
            manifest_digest: &self.manifest_digest,
            reviewer_gene_id: self.reviewer_gene_id,
            rating: self.rating,
            comment: &self.comment,
            created_at: &self.created_at,
        };
        let mut payload = SIGNING_DOMAIN.to_vec();
        payload.extend_from_slice(&serde_json::to_vec(&body)?);
        Ok(payload)
    }

    /// Review `manifest` as `reviewer`.
    pub fn new_signed(manifest: &AgentManifest, rating: u8, comment: &str, reviewer: &NetGene, reviewer_kp: &GeneKeyPair) -> Result<Self> {
        if !(1..=5).contains(&rating) {
            anyhow::bail!("Rating must be between 1 and 5");
        }
        if comment.chars().count() > MAX_COMMENT_CHARS {
            anyhow::bail!("Comment is longer than {} characters", MAX_COMMENT_CHARS);
        }
        if reviewer_kp.public_key_b64() != reviewer.public_key_b64 {
            anyhow::bail!("Signing key does not belong to gene {}", reviewer.id);
        }
        let mut review = Self {
            review_id: Uuid::new_v4(),
            agent_id: manifest.id.clone(),
            agent_version: manifest.version.clone(),
            manifest_digest: manifest_digest(manifest),
            reviewer_gene_id: reviewer.id,
            rating,
            comment: comment.to_string(),
            created_at: Utc::now(),
            reviewer_key: HybridPublicKey::from_gene(reviewer),
            signature: HybridSignature { ed25519_b64: String::new(), ml_dsa65_b64: None },
        };
        review.signature = reviewer_kp.sign_hybrid(&review.payload()?)?;
        Ok(review)
    }

    pub fn verify(&self) -> Result<()> {
        validate_agent_id(&self.agent_id)?;
        if !(1..=5).contains(&self.rating) || self.comment.chars().count() > MAX_COMMENT_CHARS {
            anyhow::bail!("Review {} is out of bounds", self.review_id);
        }
        if !verify_hybrid(&self.reviewer_key, &self.payload()?, &self.signature) {
            anyhow::bail!("Invalid signature on review {}", self.review_id);
        }
        Ok(())
    }
}

/// Reputation of one manifest version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reputation {
    pub agent_id: String,
    pub agent_version: String,
    /// Bayesian-averaged score (1–5) from trusted reviewers; `None` without any.
    pub score: Option<f64>,
    /// Plain average of trusted ratings.
    pub average: Option<f64>,
    pub trusted_reviews: usize,
    /// Valid reviews from reviewers outside the local trust set (not scored).
    pub untrusted_reviews: usize,
    /// Reviews rejected (bad signature, other version, self-review).
    pub rejected_reviews: usize,
}

/// Compute the reputation of `manifest` from `reviews`, counting only
/// reviewers whose exact key is in `trusted`.
pub fn compute(manifest: &AgentManifest, reviews: &[AgentReview], trusted: &[HybridPublicKey]) -> Reputation {
    let digest = manifest_digest(manifest);
    let mut rejected = 0;
    // Latest review per reviewer.
    let mut latest: HashMap<Uuid, &AgentReview> = HashMap::new();

    for review in reviews {
        let self_review = review.reviewer_key == manifest.publisher_key
            || review.reviewer_gene_id.to_string() == manifest.publisher_gene_id;
        if review.verify().is_err() || review.manifest_digest != digest || review.agent_id != manifest.id || self_review {
            rejected += 1;
            continue;
        }
        let keep = latest.get(&review.reviewer_gene_id).map_or(true, |prev| review.created_at > prev.created_at);
        if keep {
            latest.insert(review.reviewer_gene_id, review);
        }
    }

    let (trusted_ratings, untrusted): (Vec<&&AgentReview>, Vec<&&AgentReview>) = latest
        .values()
        .partition(|r| trusted.iter().any(|k| *k == r.reviewer_key));
    let ratings: Vec<f64> = trusted_ratings.iter().map(|r| r.rating as f64).collect();
    let n = ratings.len() as f64;
    let sum: f64 = ratings.iter().sum();

    Reputation {
        agent_id: manifest.id.clone(),
        agent_version: manifest.version.clone(),
        score: (n > 0.0).then(|| (PRIOR_MEAN * PRIOR_WEIGHT + sum) / (PRIOR_WEIGHT + n)),
        average: (n > 0.0).then(|| sum / n),
        trusted_reviews: ratings.len(),
        untrusted_reviews: untrusted.len(),
        rejected_reviews: rejected,
    }
}

/// File-backed review store: `<dir>/<agent_id>/<review_id>.json`.
pub struct ReviewStore {
    dir: PathBuf,
}

impl ReviewStore {
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// Store a review after verifying it.
    pub fn add(&self, review: &AgentReview) -> Result<()> {
        review.verify()?;
        let agent_dir = self.dir.join(&review.agent_id);
        std::fs::create_dir_all(&agent_dir)?;
        std::fs::write(agent_dir.join(format!("{}.json", review.review_id)), serde_json::to_string_pretty(review)?)?;
        Ok(())
    }

    /// Every parseable review of `agent_id` (verification happens in [`compute`]).
    pub fn reviews_for(&self, agent_id: &str) -> Result<Vec<AgentReview>> {
        validate_agent_id(agent_id)?;
        let agent_dir = self.dir.join(agent_id);
        if !agent_dir.exists() {
            return Ok(vec![]);
        }
        let mut reviews = vec![];
        for entry in std::fs::read_dir(agent_dir)?.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(review) = serde_json::from_str(&std::fs::read_to_string(&path)?) {
                    reviews.push(review);
                }
            }
        }
        Ok(reviews)
    }

    /// Reputation of each manifest.
    pub fn reputations(&self, manifests: &[AgentManifest], trusted: &[HybridPublicKey]) -> Result<BTreeMap<String, Reputation>> {
        manifests
            .iter()
            .map(|m| Ok((m.id.clone(), compute(m, &self.reviews_for(&m.id)?, trusted))))
            .collect()
    }
}

impl Default for ReviewStore {
    fn default() -> Self {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::at(home.join(".netgene").join("marketplace-reviews"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use netgene_gene::GeneRole;

    struct World {
        root: NetGene,
        root_kp: GeneKeyPair,
        manifest: AgentManifest,
    }

    fn world() -> World {
        let (root, root_kp) = NetGene::generate_master("me").unwrap();
        let (publisher, publisher_kp) = NetGene::generate_master("publisher").unwrap();
        let manifest = AgentManifest::new_signed("log-summarizer", "Logs", "1.0.0", "", vec![], &publisher, &publisher_kp).unwrap();
        World { root, root_kp, manifest }
    }

    fn reviewer(w: &World, name: &str) -> (NetGene, GeneKeyPair) {
        NetGene::spawn_sub_gene(&w.root, &w.root_kp, name, GeneRole::Agent, vec![]).unwrap()
    }

    #[test]
    fn test_only_trusted_reviewers_count() {
        let w = world();
        let (alice, alice_kp) = reviewer(&w, "alice");
        let (sybil, sybil_kp) = NetGene::generate_master("sybil").unwrap();
        let reviews = vec![
            AgentReview::new_signed(&w.manifest, 4, "solid", &alice, &alice_kp).unwrap(),
            AgentReview::new_signed(&w.manifest, 5, "best ever", &sybil, &sybil_kp).unwrap(),
        ];
        let rep = compute(&w.manifest, &reviews, &[HybridPublicKey::from_gene(&alice)]);
        assert_eq!(rep.trusted_reviews, 1);
        assert_eq!(rep.untrusted_reviews, 1);
        assert_eq!(rep.average, Some(4.0));
        // Bayesian: (3*2 + 4) / (2 + 1) = 3.33…
        assert!((rep.score.unwrap() - 10.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn test_one_review_per_reviewer_latest_wins() {
        let w = world();
        let (alice, alice_kp) = reviewer(&w, "alice");
        let first = AgentReview::new_signed(&w.manifest, 1, "", &alice, &alice_kp).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let second = AgentReview::new_signed(&w.manifest, 5, "", &alice, &alice_kp).unwrap();
        let rep = compute(&w.manifest, &[first, second], &[HybridPublicKey::from_gene(&alice)]);
        assert_eq!(rep.trusted_reviews, 1);
        assert_eq!(rep.average, Some(5.0));
    }

    #[test]
    fn test_tampered_other_version_and_self_reviews_rejected() {
        let w = world();
        let (alice, alice_kp) = reviewer(&w, "alice");
        let mut tampered = AgentReview::new_signed(&w.manifest, 2, "", &alice, &alice_kp).unwrap();
        tampered.rating = 5;

        let (publisher, publisher_kp) = NetGene::generate_master("p2").unwrap();
        let v2 = AgentManifest::new_signed("log-summarizer", "Logs", "2.0.0", "", vec![], &publisher, &publisher_kp).unwrap();
        let other_version = AgentReview::new_signed(&v2, 5, "", &alice, &alice_kp).unwrap();

        let self_review = AgentReview::new_signed(&v2, 5, "mine is great", &publisher, &publisher_kp).unwrap();

        let trusted = [HybridPublicKey::from_gene(&alice), HybridPublicKey::from_gene(&publisher)];
        let rep = compute(&w.manifest, &[tampered, other_version], &trusted);
        assert_eq!((rep.trusted_reviews, rep.rejected_reviews), (0, 2));
        assert!(rep.score.is_none());

        let rep_v2 = compute(&v2, &[self_review], &trusted);
        assert_eq!(rep_v2.rejected_reviews, 1, "a publisher cannot rate its own agent");
    }

    #[test]
    fn test_review_bounds() {
        let w = world();
        let (alice, alice_kp) = reviewer(&w, "alice");
        assert!(AgentReview::new_signed(&w.manifest, 0, "", &alice, &alice_kp).is_err());
        assert!(AgentReview::new_signed(&w.manifest, 6, "", &alice, &alice_kp).is_err());
        assert!(AgentReview::new_signed(&w.manifest, 3, &"x".repeat(501), &alice, &alice_kp).is_err());
    }

    #[test]
    fn test_review_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ReviewStore::at(dir.path().to_path_buf());
        let w = world();
        let (alice, alice_kp) = reviewer(&w, "alice");
        store.add(&AgentReview::new_signed(&w.manifest, 5, "great", &alice, &alice_kp).unwrap()).unwrap();

        let reps = store.reputations(std::slice::from_ref(&w.manifest), &[HybridPublicKey::from_gene(&alice)]).unwrap();
        assert_eq!(reps["log-summarizer"].trusted_reviews, 1);

        let mut bad = AgentReview::new_signed(&w.manifest, 5, "", &alice, &alice_kp).unwrap();
        bad.comment = "edited".into();
        assert!(store.add(&bad).is_err());
    }
}
