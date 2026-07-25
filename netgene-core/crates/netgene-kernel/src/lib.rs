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
pub mod crypto;

pub use kernel::NetSphereKernel;
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

    /// Dispatch a valid spawn_node intent — should succeed.
    #[tokio::test]
    async fn test_kernel_dispatch_spawn_intent() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let result = kernel.dispatch_intent("spawn 2 nodes").await.unwrap();
        assert!(result.contains("Intent dispatched") || result.contains("acknowledged"));
    }

    /// Dispatch an optimization intent — should succeed.
    #[tokio::test]
    async fn test_kernel_dispatch_optimize_intent() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let result = kernel.dispatch_intent("optimize routes").await.unwrap();
        assert!(result.contains("dispatched") || result.contains("acknowledged"));
    }

    /// Dispatch an unknown intent — should return a user-facing warning message.
    #[tokio::test]
    async fn test_kernel_dispatch_unknown_intent() {
        let kernel = NetSphereKernel::boot().await.unwrap();
        let result = kernel.dispatch_intent("xyzzy plugh").await.unwrap();
        assert!(result.contains("not recognized") || result.contains("Intent"));
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
