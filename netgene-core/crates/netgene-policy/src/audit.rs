//! Append-only, hash-chained, signed audit log (JSON Lines).
//!
//! Each entry stores the SHA3-256 hash of the previous entry and a hybrid
//! (Ed25519 + ML-DSA-65) signature by the kernel's operator Gene over its own
//! hash. Editing, deleting or reordering any line breaks verification of
//! every later line.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use tracing::warn;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::{verify_hybrid, HybridPublicKey, HybridSignature};

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// What happened, as supplied by the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub actor_gene_id: String,
    /// Original request (e.g. the natural-language intent).
    pub input: String,
    pub action: Option<String>,
    pub params: serde_json::Value,
    /// allow | confirm | deny | confirmed | executed
    pub decision: String,
    pub detail: String,
}

/// Fields covered by the hash.
#[derive(Serialize)]
struct AuditBody<'a> {
    seq: u64,
    timestamp: &'a DateTime<Utc>,
    record: &'a AuditRecord,
    signer: &'a HybridPublicKey,
    prev_hash: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub timestamp: DateTime<Utc>,
    pub record: AuditRecord,
    pub signer: HybridPublicKey,
    pub prev_hash: String,
    pub hash: String,
    pub signature: HybridSignature,
}

impl AuditEntry {
    fn compute_hash(&self) -> Result<String> {
        let body = AuditBody {
            seq: self.seq,
            timestamp: &self.timestamp,
            record: &self.record,
            signer: &self.signer,
            prev_hash: &self.prev_hash,
        };
        Ok(hex::encode(Sha3_256::digest(serde_json::to_vec(&body)?)))
    }
}

pub struct AuditLog {
    path: Option<PathBuf>,
    signer: GeneKeyPair,
    next_seq: u64,
    last_hash: String,
    memory: Vec<AuditEntry>,
}

impl AuditLog {
    /// Open (or create) a log file and continue its chain.
    ///
    /// A log written by NetGene 0.5 (Ed25519-only entry format) is moved
    /// aside to `<name>.v1` untouched, and a new chain is started.
    pub fn open(path: PathBuf, signer: GeneKeyPair) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.exists() && Self::is_legacy_format(&path)? {
            let archived = path.with_extension("jsonl.v1");
            fs::rename(&path, &archived)?;
            warn!("Archived NetGene 0.5 audit log to {:?}; starting a new hybrid-signed chain", archived);
        }
        let entries = if path.exists() { Self::read_entries(&path)? } else { vec![] };
        verify_chain(&entries).with_context(|| format!("Audit log {} failed verification", path.display()))?;

        let (next_seq, last_hash) = match entries.last() {
            Some(last) => (last.seq + 1, last.hash.clone()),
            None => (0, GENESIS_HASH.to_string()),
        };
        Ok(Self { path: Some(path), signer, next_seq, last_hash, memory: vec![] })
    }

    fn is_legacy_format(path: &Path) -> Result<bool> {
        let text = fs::read_to_string(path)?;
        let Some(first) = text.lines().find(|l| !l.trim().is_empty()) else { return Ok(false) };
        Ok(serde_json::from_str::<serde_json::Value>(first)
            .map(|v| v.get("signer_public_key_b64").is_some() && v.get("signer").is_none())
            .unwrap_or(false))
    }

    /// Log kept only in memory (tests, ephemeral kernels).
    pub fn in_memory(signer: GeneKeyPair) -> Self {
        Self { path: None, signer, next_seq: 0, last_hash: GENESIS_HASH.to_string(), memory: vec![] }
    }

    pub fn append(&mut self, record: AuditRecord) -> Result<AuditEntry> {
        let mut entry = AuditEntry {
            seq: self.next_seq,
            timestamp: Utc::now(),
            record,
            signer: self.signer.hybrid_public_key(),
            prev_hash: self.last_hash.clone(),
            hash: String::new(),
            signature: HybridSignature { ed25519_b64: String::new(), ml_dsa65_b64: None },
        };
        entry.hash = entry.compute_hash()?;
        entry.signature = self.signer.sign_hybrid(entry.hash.as_bytes())?;

        match &self.path {
            Some(path) => {
                let mut file = OpenOptions::new().create(true).append(true).open(path)?;
                writeln!(file, "{}", serde_json::to_string(&entry)?)?;
                file.sync_data()?;
            }
            None => self.memory.push(entry.clone()),
        }

        self.next_seq += 1;
        self.last_hash = entry.hash.clone();
        Ok(entry)
    }

    /// Entries of an in-memory log, or of the backing file.
    pub fn entries(&self) -> Result<Vec<AuditEntry>> {
        match &self.path {
            Some(path) => Self::read_entries(path),
            None => Ok(self.memory.clone()),
        }
    }

    pub fn read_entries(path: &Path) -> Result<Vec<AuditEntry>> {
        fs::read_to_string(path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, l)| serde_json::from_str(l).with_context(|| format!("Audit line {} is not valid JSON", i + 1)))
            .collect()
    }

    /// Verify a log file; returns the number of entries.
    pub fn verify_file(path: &Path) -> Result<usize> {
        let entries = Self::read_entries(path)?;
        verify_chain(&entries)?;
        Ok(entries.len())
    }
}

/// Check sequence numbers, hash links, hashes and signatures.
pub fn verify_chain(entries: &[AuditEntry]) -> Result<()> {
    let mut prev = GENESIS_HASH.to_string();
    for (i, entry) in entries.iter().enumerate() {
        if entry.seq != i as u64 {
            anyhow::bail!("entry {} has sequence {} (expected {})", i, entry.seq, i);
        }
        if entry.prev_hash != prev {
            anyhow::bail!("entry {} does not link to the previous entry", entry.seq);
        }
        if entry.compute_hash()? != entry.hash {
            anyhow::bail!("entry {} was modified (hash mismatch)", entry.seq);
        }
        if !verify_hybrid(&entry.signer, entry.hash.as_bytes(), &entry.signature) {
            anyhow::bail!("entry {} has an invalid signature", entry.seq);
        }
        prev = entry.hash.clone();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(n: u32) -> AuditRecord {
        AuditRecord {
            actor_gene_id: "gene".into(),
            input: format!("intent {n}"),
            action: Some("system.status".into()),
            params: serde_json::json!({}),
            decision: "allow".into(),
            detail: String::new(),
        }
    }

    fn signer() -> GeneKeyPair {
        GeneKeyPair::generate().unwrap()
    }

    #[test]
    fn test_chain_verifies_and_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");

        let mut log = AuditLog::open(path.clone(), signer()).unwrap();
        log.append(record(1)).unwrap();
        log.append(record(2)).unwrap();
        drop(log);

        // A new session (new signer) continues the same chain.
        let mut log = AuditLog::open(path.clone(), signer()).unwrap();
        assert_eq!(log.append(record(3)).unwrap().seq, 2);
        assert_eq!(AuditLog::verify_file(&path).unwrap(), 3);
    }

    #[test]
    fn test_tampering_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        let mut log = AuditLog::open(path.clone(), signer()).unwrap();
        for n in 0..3 {
            log.append(record(n)).unwrap();
        }

        let original = fs::read_to_string(&path).unwrap();

        // Edit a decision.
        fs::write(&path, original.replacen("\"decision\":\"allow\"", "\"decision\":\"deny\"", 1)).unwrap();
        assert!(AuditLog::verify_file(&path).is_err());

        // Delete the middle line.
        let lines: Vec<&str> = original.lines().collect();
        fs::write(&path, format!("{}\n{}\n", lines[0], lines[2])).unwrap();
        assert!(AuditLog::verify_file(&path).is_err());

        // A tampered log refuses to be extended.
        assert!(AuditLog::open(path, signer()).is_err());
    }

    #[test]
    fn test_entries_are_post_quantum_signed() {
        let mut log = AuditLog::in_memory(signer());
        let mut entry = log.append(record(1)).unwrap();
        assert!(entry.signer.is_post_quantum());
        verify_chain(std::slice::from_ref(&entry)).unwrap();

        entry.signature.ml_dsa65_b64 = None;
        assert!(verify_chain(&[entry]).is_err());
    }

    #[test]
    fn test_legacy_log_is_archived() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");
        fs::write(&path, "{\"seq\":0,\"signer_public_key_b64\":\"x\",\"signature_b64\":\"y\"}\n").unwrap();

        let mut log = AuditLog::open(path.clone(), signer()).unwrap();
        log.append(record(1)).unwrap();
        assert!(dir.path().join("audit.jsonl.v1").exists());
        assert_eq!(AuditLog::verify_file(&path).unwrap(), 1);
    }

    #[test]
    fn test_in_memory_log() {
        let mut log = AuditLog::in_memory(signer());
        log.append(record(1)).unwrap();
        log.append(record(2)).unwrap();
        let entries = log.entries().unwrap();
        assert_eq!(entries.len(), 2);
        verify_chain(&entries).unwrap();
    }
}
