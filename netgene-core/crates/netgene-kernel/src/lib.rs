//! # Netsphere Kernel
//!
//! The collective intelligence layer of NetGene OS.
//! Orchestrates specialized AI agents via an async message bus.
//!
//! ## Architecture
//! ```text
//! ┌──────────────────────────────────────────┐
//! │              Netsphere Kernel             │
//! │  ┌──────────┐ ┌──────────┐ ┌──────────┐ │
//! │  │ Builder  │ │ Monitor  │ │Optimizer │ │
//! │  │  Agent   │ │  Agent   │ │  Agent   │ │
//! │  └────┬─────┘ └────┬─────┘ └────┬─────┘ │
//! │       └────────────┼────────────┘        │
//! │              Message Bus                 │
//! │           (Tokio channels)               │
//! └──────────────────────────────────────────┘
//! ```

pub mod agent;
pub mod bus;
pub mod intent;
pub mod memory;
pub mod kernel;
pub mod error;
pub mod marketplace;
pub mod reputation;

pub use kernel::{DispatchOutcome, KernelConfig, NetSphereKernel};
pub use agent::{Agent, AgentId, AgentStatus, AgentMessage};
pub use bus::MessageBus;
pub use intent::{Intent, IntentParser, IntentAction};
pub use error::KernelError;

#[cfg(test)]
mod tests {
    use super::*;

    /// Kernel boots with 5 agents and returns a valid agent list.
    #[tokio::test]
    async fn test_kernel_boot_has_five_agents() {
        let kernel = NetSphereKernel::boot().await.expect("kernel boot failed");
        let agents = kernel.agent_list().await;
        assert_eq!(agents.len(), 5, "Expected exactly 5 agents");
        let names: Vec<_> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"BuilderAgent"));
        assert!(names.contains(&"MonitorAgent"));
        assert!(names.contains(&"OptimizerAgent"));
        assert!(names.contains(&"NetworkAgent"));
        assert!(names.contains(&"EvolutionAgent"));
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

        let confirmed = kernel.confirm(proposal_id).await.unwrap();
        assert!(matches!(confirmed, DispatchOutcome::Executed { .. }));
        assert!(kernel.confirm(proposal_id).await.is_err(), "a proposal can only be confirmed once");
    }

    /// Optimisation also changes state, so it is held for confirmation.
    #[tokio::test]
    async fn test_kernel_optimize_is_pending() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let result = kernel.dispatch_intent("optimize routes").await.unwrap();
        assert!(result.contains("Confirmation required"), "unexpected result: {result}");
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
                operator, operator_kp, policy: netgene_policy::Policy::default(), audit_path: None, grants, presence_key: None,
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
}
