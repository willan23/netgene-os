//! Audit log export for SIEMs.
//!
//! - **OCSF** (Open Cybersecurity Schema Framework) JSON, class *API Activity*
//!   (`class_uid` 6003), one object per line. NetGene-specific fields
//!   (hash chain, signer, raw decision) go under `unmapped`.
//! - **CEF** (ArcSight Common Event Format), one line per entry.
//!
//! Exports are produced only from a log whose chain verifies.

use serde_json::{json, Value};

use crate::audit::AuditEntry;

const VENDOR: &str = "NetGene";
const PRODUCT: &str = "NetGene OS";

fn severity(decision: &str) -> (u8, &'static str, u8) {
    // (OCSF severity_id, OCSF severity, CEF severity 0-10)
    match decision {
        "deny" | "presence_failed" => (3, "Medium", 6),
        "rejected" => (2, "Low", 4),
        _ => (1, "Informational", 2),
    }
}

/// OCSF API Activity event for one audit entry.
pub fn to_ocsf(entry: &AuditEntry) -> Value {
    let decision = entry.record.decision.as_str();
    let (severity_id, severity_name, _) = severity(decision);
    let denied = matches!(decision, "deny" | "presence_failed" | "rejected");
    json!({
        "class_uid": 6003,
        "class_name": "API Activity",
        "category_uid": 6,
        "category_name": "Application Activity",
        "activity_id": 99,
        "activity_name": decision,
        "type_uid": 600399,
        "time": entry.timestamp.timestamp_millis(),
        "severity_id": severity_id,
        "severity": severity_name,
        "status_id": if denied { 2 } else { 1 },
        "status": if denied { "Failure" } else { "Success" },
        "status_detail": entry.record.detail,
        "message": entry.record.input,
        "actor": { "user": { "uid": entry.record.actor_gene_id, "type": "Gene" } },
        "api": {
            "operation": entry.record.action.clone().unwrap_or_default(),
            "request": { "data": entry.record.params },
        },
        "metadata": {
            "version": "1.3.0",
            "uid": entry.hash,
            "sequence": entry.seq,
            "product": { "name": PRODUCT, "vendor_name": VENDOR, "version": env!("CARGO_PKG_VERSION") },
        },
        "unmapped": {
            "netgene_decision": decision,
            "prev_hash": entry.prev_hash,
            "signer_ed25519": entry.signer.ed25519_b64,
            "signer_post_quantum": entry.signer.is_post_quantum(),
        },
    })
}

fn cef_header(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

fn cef_value(s: &str) -> String {
    s.replace('\\', "\\\\").replace('=', "\\=").replace('\n', "\\n").replace('\r', "\\r")
}

/// CEF line for one audit entry.
pub fn to_cef(entry: &AuditEntry) -> String {
    let decision = entry.record.decision.as_str();
    let (_, _, sev) = severity(decision);
    let action = entry.record.action.clone().unwrap_or_else(|| "-".into());
    format!(
        "CEF:0|{}|{}|{}|{}|{}|{}|rt={} suser={} act={} msg={} cs1Label=auditHash cs1={} cs2Label=prevHash cs2={} cn1Label=seq cn1={}",
        cef_header(VENDOR),
        cef_header(PRODUCT),
        cef_header(env!("CARGO_PKG_VERSION")),
        cef_header(decision),
        cef_header(&action),
        sev,
        entry.timestamp.timestamp_millis(),
        cef_value(&entry.record.actor_gene_id),
        cef_value(decision),
        cef_value(&entry.record.input),
        entry.hash,
        entry.prev_hash,
        entry.seq,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditLog, AuditRecord};
    use netgene_gene::crypto::GeneKeyPair;

    fn entry(decision: &str, input: &str) -> AuditEntry {
        let mut log = AuditLog::in_memory(GeneKeyPair::generate().unwrap());
        log.append(AuditRecord {
            actor_gene_id: "gene-1".into(),
            input: input.into(),
            action: Some("node.provision".into()),
            params: serde_json::json!({"count": 2}),
            decision: decision.into(),
            detail: "policy".into(),
        })
        .unwrap()
    }

    #[test]
    fn test_ocsf_shape() {
        let v = to_ocsf(&entry("deny", "spawn 5000 nodes"));
        assert_eq!(v["class_uid"], 6003);
        assert_eq!(v["status"], "Failure");
        assert_eq!(v["api"]["operation"], "node.provision");
        assert_eq!(v["actor"]["user"]["uid"], "gene-1");
        assert_eq!(v["unmapped"]["signer_post_quantum"], true);
    }

    #[test]
    fn test_cef_escaping() {
        let line = to_cef(&entry("allow", "a=b | c\\d\nnext"));
        assert!(line.starts_with("CEF:0|NetGene|NetGene OS|"));
        assert!(line.contains("msg=a\\=b | c\\\\d\\nnext"), "{line}");
        assert!(!line.contains('\n'));
    }
}
