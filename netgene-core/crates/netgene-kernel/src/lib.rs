//! # Netsphere Kernel
//!
//! Turns intents into policy-checked, audited actions:
//!
//! ```text
//! intent --> LLM / rules --> Action --> PolicyEngine --> allow | confirm | deny
//!            (proposes only)                 |               (human, optional
//!                                            v                Windows Hello)
//!                                        Executor --> result --> signed audit log
//!                          (genes, host metrics, signed WASM agents)
//! ```

pub mod agent;
pub mod executor;
pub mod memory;
pub mod kernel;
pub mod error;
pub mod marketplace;
pub mod reputation;

pub use kernel::{DispatchOutcome, KernelConfig, NetSphereKernel};
pub use agent::{AgentId, AgentInfo, AgentStatus};
pub use executor::Workspace;
pub use error::KernelError;

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh kernel has no installed agents (nothing is invented).
    #[tokio::test]
    async fn test_kernel_boot_lists_only_real_agents() {
        let kernel = NetSphereKernel::boot().await.expect("kernel boot failed");
        assert!(kernel.agent_list().await.is_empty());
    }

    /// A state-changing intent waits for confirmation, then executes.
    #[tokio::test]
    async fn test_kernel_spawn_requires_confirmation() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let outcome = kernel.propose_intent("spawn 2 nodes").await.unwrap();
        let DispatchOutcome::PendingConfirmation { proposal_id, action, .. } = outcome else {
            panic!("expected confirmation, got {outcome:?}");
        };
        assert_eq!(action.kind, "node.provision");

        let DispatchOutcome::Executed { result, .. } = kernel.confirm(proposal_id).await.unwrap() else {
            panic!("expected execution");
        };
        assert_eq!(result["created"].as_array().unwrap().len(), 2, "two real sub-genes");
        assert!(kernel.confirm(proposal_id).await.is_err(), "a proposal can only be confirmed once");
    }

    /// There is no routing optimizer, so the intent is refused instead of faked.
    #[tokio::test]
    async fn test_kernel_optimize_is_not_executable() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        assert!(matches!(kernel.propose_intent("optimize routes").await.unwrap(), DispatchOutcome::Denied { action: None, .. }));
    }

    /// Read-only intents run immediately (unknown text falls back to a status query).
    #[tokio::test]
    async fn test_kernel_read_intent_executes() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let outcome = kernel.propose_intent("xyzzy plugh").await.unwrap();
        assert!(matches!(outcome, DispatchOutcome::Executed { ref action, .. } if action.kind == "system.status"));
    }

    /// Requests beyond policy limits are denied, and everything is audited.
    #[tokio::test]
    async fn test_kernel_denies_and_audits() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let outcome = kernel.propose_intent("spawn 5000 nodes").await.unwrap();
        assert!(matches!(outcome, DispatchOutcome::Denied { .. }), "unexpected {outcome:?}");

        let entries = kernel.audit_entries().await.unwrap();
        assert_eq!(entries.last().unwrap().record.decision, "deny");
        netgene_policy::audit::verify_chain(&entries).unwrap();
    }

    /// With `require_user_presence`, only a valid hardware-key signature over
    /// the exact proposal confirms it (the RSA key here stands in for Windows Hello).
    #[tokio::test]
    async fn test_kernel_presence_required() {
        use ring::rand::SystemRandom;
        use ring::signature::{RsaKeyPair, RSA_PKCS1_SHA256};

        let key = RsaKeyPair::from_pkcs8(include_bytes!("../../netgene-presence/tests/fixtures/test-only-rsa2048.pk8.der")).unwrap();
        let sign = |msg: &[u8]| {
            let mut sig = vec![0u8; key.public().modulus_len()];
            key.sign(&RSA_PKCS1_SHA256, &SystemRandom::new(), msg, &mut sig).unwrap();
            sig
        };

        let mut config = KernelConfig::ephemeral().unwrap();
        config.policy.require_user_presence = true;
        config.presence_key = Some(key.public().as_ref().to_vec());
        let kernel = NetSphereKernel::boot_with(config).await.unwrap();

        let DispatchOutcome::PendingConfirmation { proposal_id, .. } = kernel.propose_intent("spawn 2 nodes").await.unwrap() else {
            panic!("expected confirmation");
        };
        assert!(kernel.confirm(proposal_id).await.is_err(), "plain confirmation is not enough");
        assert!(kernel.confirm_with_presence(proposal_id, &sign(b"wrong challenge")).await.is_err());

        let challenge = kernel.presence_challenge(proposal_id).await.unwrap();
        let outcome = kernel.confirm_with_presence(proposal_id, &sign(&challenge)).await.unwrap();
        assert!(matches!(outcome, DispatchOutcome::Executed { .. }));

        let entries = kernel.audit_entries().await.unwrap();
        assert!(entries.iter().any(|e| e.record.decision == "presence_failed"));
        assert!(entries.iter().any(|e| e.record.detail.contains("user presence verified")));
    }

    /// Capabilities granted by a (verified) token let a delegated gene act.
    #[tokio::test]
    async fn test_kernel_uses_active_grants() {
        use netgene_gene::{GeneRole, NetGene};
        let (master, master_kp) = NetGene::generate_master("root").unwrap();
        let (agent, agent_kp) = NetGene::spawn_sub_gene(&master, &master_kp, "agent", GeneRole::Agent, vec!["network.read".into()]).unwrap();

        let boot = |grants| {
            let (operator, operator_kp) = (agent.clone(), netgene_gene::crypto::GeneKeyPair::from_pkcs8(agent_kp.pkcs8_bytes().to_vec()).unwrap());
            NetSphereKernel::boot_with(KernelConfig {
                operator, operator_kp, grants, ..KernelConfig::ephemeral().unwrap()
            })
        };

        let without = boot(vec![]).await.unwrap();
        assert!(matches!(without.propose_intent("spawn 1 node").await.unwrap(), DispatchOutcome::Denied { .. }));

        let grant = netgene_policy::Grant {
            token_id: uuid::Uuid::new_v4(),
            capabilities: vec!["node.spawn".into()],
            expires_at: chrono::Utc::now() + chrono::Duration::minutes(5),
        };
        let with = boot(vec![grant.clone()]).await.unwrap();
        assert!(matches!(with.propose_intent("spawn 1 node").await.unwrap(), DispatchOutcome::PendingConfirmation { .. }));

        let expired = netgene_policy::Grant { expires_at: chrono::Utc::now() - chrono::Duration::seconds(1), ..grant };
        let stale = boot(vec![expired]).await.unwrap();
        assert!(matches!(stale.propose_intent("spawn 1 node").await.unwrap(), DispatchOutcome::Denied { .. }));
    }

    /// Rejected proposals are audited and cannot be confirmed later.
    #[tokio::test]
    async fn test_kernel_reject_proposal() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let DispatchOutcome::PendingConfirmation { proposal_id, .. } = kernel.propose_intent("spawn 1 node").await.unwrap() else {
            panic!("expected confirmation");
        };
        kernel.reject(proposal_id).await.unwrap();
        assert!(kernel.confirm(proposal_id).await.is_err());
        assert_eq!(kernel.audit_entries().await.unwrap().last().unwrap().record.decision, "rejected");
    }

    /// Kernel memory stores and retrieves events.
    #[tokio::test]
    async fn test_kernel_memory_log_event() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        {
            let mut mem = kernel.memory.lock().await;
            mem.log_event("test-event-alpha".to_string());
            mem.log_event("test-event-beta".to_string());
        }
        let mem = kernel.memory.lock().await;
        let recent = mem.recent_events(5);
        assert!(recent.iter().any(|e| e.contains("test-event-beta")));
    }

    /// Kernel memory stores typed entries.
    #[tokio::test]
    async fn test_kernel_memory_store_and_retrieve() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let id = {
            let mut mem = kernel.memory.lock().await;
            mem.store("intent", serde_json::json!({"action": "spawn_node"}), vec!["spawn".to_string()])
        };
        let mem = kernel.memory.lock().await;
        let entry = mem.get(&id);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().kind, "intent");
    }

    /// Kernel memory tag search works.
    #[tokio::test]
    async fn test_kernel_memory_search_by_tag() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        {
            let mut mem = kernel.memory.lock().await;
            mem.store("event", serde_json::json!("data"), vec!["anomaly".to_string(), "critical".to_string()]);
        }
        let mem = kernel.memory.lock().await;
        let results = mem.search_by_tag("anomaly");
        assert!(!results.is_empty());
    }

    /// Kernel graceful shutdown succeeds without panic.
    #[tokio::test]
    async fn test_kernel_shutdown() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        kernel.shutdown().await.expect("shutdown should succeed");
    }

    /// Provisioned genes are real children of the operator, saved with their keys.
    #[tokio::test]
    async fn test_provision_saves_verifiable_sub_genes() {
        use netgene_gene::storage::{list_genes, load_gene};
        let dir = tempfile::tempdir().unwrap();
        let mut config = KernelConfig::ephemeral().unwrap();
        let operator = config.operator.clone();
        config.workspace.gene_dir = Some(dir.path().join("genes"));
        let secrets = config.workspace.secrets.clone();
        let store = config.workspace.store.clone();
        let kernel = NetSphereKernel::boot_with(config).await.unwrap();

        let action = netgene_policy::Action::new("node.provision", serde_json::json!({"count": 2, "template": "edge"}));
        let DispatchOutcome::PendingConfirmation { proposal_id, .. } = kernel.propose_action("test", action, String::new()).await.unwrap() else {
            panic!("expected confirmation");
        };
        kernel.confirm(proposal_id).await.unwrap();

        let genes = list_genes(&dir.path().join("genes")).unwrap();
        assert_eq!(genes.len(), 2);
        let lineage = netgene_gene::storage::trusted_lineage(&operator, &[vec![operator.clone()], genes.clone()].concat());
        for gene in &genes {
            assert_eq!(gene.parent_id, Some(operator.id));
            assert!(lineage.iter().any(|g| g.id == gene.id), "lineage signed by the operator");
            load_gene(&gene.id.to_string(), &dir.path().join("genes"), secrets.as_ref()).expect("key stored");
        }
        assert_eq!(store.list_nodes().unwrap().len(), 2);
    }

    /// The scan samples real host metrics.
    #[tokio::test]
    async fn test_scan_samples_host_metrics() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let action = netgene_policy::Action::new("safeguard.scan", serde_json::json!({}));
        let DispatchOutcome::Executed { result, .. } = kernel.propose_action("scan", action, String::new()).await.unwrap() else {
            panic!("reads run immediately");
        };
        let samples = result["samples"].as_object().unwrap();
        assert!(samples["host.processes"].as_f64().unwrap() > 0.0);
        assert!((0.0..=100.0).contains(&samples["host.memory_percent"].as_f64().unwrap()));
    }

    /// Counts its runs in the KV store and echoes its input.
    const COUNTER_AGENT: &str = r#"(module
        (import "netgene" "kv_get" (func $get (param i32 i32 i32 i32) (result i32)))
        (import "netgene" "kv_set" (func $set (param i32 i32 i32 i32) (result i32)))
        (import "netgene" "input_read" (func $read (param i32 i32) (result i32)))
        (import "netgene" "output_write" (func $write (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (data (i32.const 0) "runs")
        (func (export "run") (result i32)
            (local $n i32) (local $len i32)
            (drop (call $get (i32.const 0) (i32.const 4) (i32.const 16) (i32.const 4)))
            (local.set $n (i32.add (i32.load (i32.const 16)) (i32.const 1)))
            (i32.store (i32.const 16) (local.get $n))
            (drop (call $set (i32.const 0) (i32.const 4) (i32.const 16) (i32.const 4)))
            (local.set $len (call $read (i32.const 64) (i32.const 1024)))
            (drop (call $write (i32.const 64) (local.get $len)))
            (local.get $n)))"#;

    async fn kernel_with_agents(policy: netgene_policy::Policy) -> (NetSphereKernel, netgene_gene::NetGene, netgene_gene::crypto::GeneKeyPair, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let mut config = KernelConfig::ephemeral().unwrap();
        config.policy = policy;
        config.workspace.agents_dir = Some(dir.path().join("agents"));
        let (operator, kp) = (config.operator.clone(), config.operator_kp.try_clone().unwrap());
        (NetSphereKernel::boot_with(config).await.unwrap(), operator, kp, dir)
    }

    async fn run_agent(kernel: &NetSphereKernel, input: &str) -> anyhow::Result<serde_json::Value> {
        let action = netgene_policy::Action::new("agent.run", serde_json::json!({"agent": "counter", "input": input}));
        let DispatchOutcome::PendingConfirmation { proposal_id, .. } = kernel.propose_action("run", action, String::new()).await? else {
            panic!("agent.run changes state, so it is confirmed");
        };
        match kernel.confirm(proposal_id).await? {
            DispatchOutcome::Executed { result, .. } => Ok(result),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_wasm_agent_runs_with_persistent_kv() {
        let (kernel, operator, kp, _dir) = kernel_with_agents(netgene_policy::Policy::default()).await;
        let wasm = wat::parse_str(COUNTER_AGENT).unwrap();
        let module = netgene_wasm::GeneModule::sign("counter", "1.0.0", &wasm, vec!["io".into(), "kv".into()], &operator, &kp).unwrap();
        kernel.install_wasm_agent(&module).await.unwrap();
        assert_eq!(kernel.agent_list().await.len(), 1);

        let first = run_agent(&kernel, "hello").await.unwrap();
        assert_eq!(first["output"], "hello");
        assert_eq!(first["return_value"], 1);
        let second = run_agent(&kernel, "again").await.unwrap();
        assert_eq!(second["return_value"], 2, "KV state persisted between runs");

        let audit = kernel.audit_entries().await.unwrap();
        assert!(audit.iter().any(|e| e.record.decision == "executed" && e.record.action.as_deref() == Some("agent.run")));
    }

    #[tokio::test]
    async fn test_untrusted_or_unpermitted_agents_refused() {
        let wasm = wat::parse_str(COUNTER_AGENT).unwrap();

        let (kernel, _, _, _dir) = kernel_with_agents(netgene_policy::Policy::default()).await;
        let (stranger, stranger_kp) = netgene_gene::NetGene::generate_master("stranger").unwrap();
        let foreign = netgene_wasm::GeneModule::sign("counter", "1.0.0", &wasm, vec!["io".into(), "kv".into()], &stranger, &stranger_kp).unwrap();
        assert!(kernel.install_wasm_agent(&foreign).await.is_err(), "author outside the operator's lineage");

        let no_kv = netgene_policy::Policy { agent_permissions: vec!["io".into()], ..Default::default() };
        let (kernel, operator, kp, _dir2) = kernel_with_agents(no_kv).await;
        let module = netgene_wasm::GeneModule::sign("counter", "1.0.0", &wasm, vec!["io".into(), "kv".into()], &operator, &kp).unwrap();
        kernel.install_wasm_agent(&module).await.unwrap();
        let err = run_agent(&kernel, "x").await.unwrap_err();
        assert!(format!("{err:#}").contains("kv"), "policy withholds kv: {err:#}");
        assert_eq!(kernel.audit_entries().await.unwrap().last().unwrap().record.decision, "failed");
    }
}
