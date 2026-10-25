//! Local storage of capability tokens and their verification for the CLI.

use std::path::PathBuf;
use anyhow::Result;
use chrono::Utc;
use netgene_gene::{CapabilityToken, NetGene, RevocationList};
use netgene_policy::Grant;

pub fn netgene_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".netgene")
}

pub fn tokens_dir() -> PathBuf {
    netgene_home().join("tokens")
}

pub fn revocation_path() -> PathBuf {
    netgene_home().join("revoked-tokens.json")
}

pub fn presence_dir() -> PathBuf {
    netgene_home().join("presence")
}

/// Every token file in the local token store.
pub fn list_tokens() -> Result<Vec<CapabilityToken>> {
    let dir = tokens_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut tokens = vec![];
    for entry in std::fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "json") {
            match serde_json::from_str::<CapabilityToken>(&std::fs::read_to_string(&path)?) {
                Ok(token) => tokens.push(token),
                Err(e) => tracing::warn!("Ignoring token file {:?}: {}", path, e),
            }
        }
    }
    tokens.sort_by_key(|t| t.claims.expires_at);
    Ok(tokens)
}

/// Check one token against the local gene store and revocation list.
pub fn check(token: &CapabilityToken, genes: &[NetGene], revoked: &RevocationList) -> Result<()> {
    let find = |id| genes.iter().find(|g| g.id == id);
    let issuer = find(token.claims.issuer_gene_id)
        .ok_or_else(|| anyhow::anyhow!("issuer gene {} is not in the local store", token.claims.issuer_gene_id))?;
    let subject = find(token.claims.subject_gene_id)
        .ok_or_else(|| anyhow::anyhow!("subject gene {} is not in the local store", token.claims.subject_gene_id))?;
    // Only the subject's own (verified) parent may delegate to it.
    if !subject.verify_lineage(issuer) {
        anyhow::bail!("issuer {} is not the verified parent of {}", issuer.short_fp, subject.short_fp);
    }
    token.verify(issuer, subject, revoked, Utc::now())?;
    Ok(())
}

/// Grants usable by `subject` right now.
pub fn grants_for(subject: &NetGene, genes: &[NetGene]) -> Result<Vec<Grant>> {
    let revoked = RevocationList::load(&revocation_path())?;
    let mut grants = vec![];
    for token in list_tokens()?.into_iter().filter(|t| t.claims.subject_gene_id == subject.id) {
        match check(&token, genes, &revoked) {
            Ok(()) => grants.push(Grant {
                token_id: token.claims.token_id,
                capabilities: token.claims.capabilities.clone(),
                expires_at: token.claims.expires_at,
            }),
            Err(e) => tracing::warn!("Skipping token {}: {}", token.claims.token_id, e),
        }
    }
    Ok(grants)
}
