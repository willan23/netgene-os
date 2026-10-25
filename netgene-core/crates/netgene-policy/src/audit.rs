//! Append-only, hash-chained, signed audit log (JSON Lines).
//!
//! Each entry stores the SHA3-256 hash of the previous entry and an Ed25519
//! signature (by the kernel's operator Gene) over its own hash. Editing,
//! deleting or reordering any line breaks verification of every later line.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use netgene_gene::crypto::{verify_signature, GeneKeyPair};

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
    signer_public_key_b64: &'a str,
    prev_hash: &'a str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub seq: u64,
    pub timestamp: DateTime<Utc>,
    pub record: AuditRecord,
    pub signer_public_key_b64: String,
    pub prev_hash: String,
    pub hash: String,
    pub signature_b64: String,
}

impl AuditEntry {
    fn compute_hash(&self) -> Result<String> {
        let body = AuditBody {
            seq: self.seq,
            timestamp: &self.timestamp,
            record: &self.record,
            signer_public_key_b64: &self.signer_public_key_b64,
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
    pub fn open(path: PathBuf, signer: GeneKeyPair) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let entries = if path.exists() { Self::read_entries(&path)? } else { vec![] };
        verify_chain(&entries).with_context(|| format!("Audit log {} failed verification", path.display()))?;

        let (next_seq, last_hash) = match entries.last() {
            Some(last) => (last.seq + 1, last.hash.clone()),
            None => (0, GENESIS_HASH.to_string()),
        };
        Ok(Self { path: Some(path), signer, next_seq, last_hash, memory: vec![] })
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
            signer_public_key_b64: self.signer.public_key_b64(),
            prev_hash: self.last_hash.clone(),
            hash: String::new(),
            signature_b64: String::new(),
        };
        entry.hash = entry.compute_hash()?;
        entry.signature_b64 = BASE64.encode(self.signer.sign(entry.hash.as_bytes())?);

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
        let key = BASE64.decode(&entry.signer_public_key_b64)?;
        let sig = BASE64.decode(&entry.signature_b64)?;
        if !verify_signature(&key, entry.hash.as_bytes(), &sig) {
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
    fn test_in_memory_log() {
        let mut log = AuditLog::in_memory(signer());
        log.append(record(1)).unwrap();
        log.append(record(2)).unwrap();
        let entries = log.entries().unwrap();
        assert_eq!(entries.len(), 2);
        verify_chain(&entries).unwrap();
    }
}
