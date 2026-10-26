//! Signed records read from disk or the network: capability tokens, WASM
//! Gene Modules and audit log entries.
#![no_main]

use std::sync::OnceLock;
use libfuzzer_sys::fuzz_target;
use netgene_gene::{CapabilityToken, GeneRole, HybridPublicKey, NetGene, RevocationList};
use netgene_policy::audit::{verify_chain, AuditEntry};
use netgene_wasm::{GeneModule, WasmSandbox};

struct Setup {
    master: NetGene,
    child: NetGene,
}

fn setup() -> Setup {
    let (master, kp) = NetGene::generate_master("fuzz-master").unwrap();
    let (child, _) = NetGene::spawn_sub_gene(&master, &kp, "fuzz-child", GeneRole::Agent, vec![]).unwrap();
    Setup { master, child }
}

static SETUP: OnceLock<Setup> = OnceLock::new();

fuzz_target!(|data: &[u8]| {
    let s = SETUP.get_or_init(setup);
    if let Ok(token) = serde_json::from_slice::<CapabilityToken>(data) {
        let _ = token.verify(&s.master, &s.child, &RevocationList::default(), chrono::Utc::now());
    }
    if let Ok(module) = serde_json::from_slice::<GeneModule>(data) {
        let sandbox = WasmSandbox::new(1 << 20, vec![HybridPublicKey::from_gene(&s.master)]).allow_permissions(["log", "io", "kv", "clock"]);
        let _ = sandbox.verify(&module);
    }
    if let Ok(entries) = serde_json::from_slice::<Vec<AuditEntry>>(data) {
        let _ = verify_chain(&entries);
    }
});
