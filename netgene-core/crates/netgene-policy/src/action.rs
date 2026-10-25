//! Catalog of actions agents may request.

use serde::{Deserialize, Serialize};

/// How much damage an action can do if it is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    /// Observes state only.
    Read,
    /// Changes state in a recoverable way.
    Write,
    /// Hard or impossible to undo.
    Destructive,
}

/// Static description of an action kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionSpec {
    /// Stable action id, e.g. `node.provision`.
    pub kind: &'static str,
    /// Gene capability required to perform it.
    pub capability: &'static str,
    pub risk: Risk,
    pub description: &'static str,
}

/// Every action the kernel can execute. Anything else is denied.
pub const ACTION_CATALOG: &[ActionSpec] = &[
    ActionSpec { kind: "system.status", capability: "network.read", risk: Risk::Read, description: "Report kernel and agent status" },
    ActionSpec { kind: "safeguard.scan", capability: "network.read", risk: Risk::Read, description: "Run an anomaly scan" },
    ActionSpec { kind: "network.optimize", capability: "network.admin", risk: Risk::Write, description: "Recompute routing preferences" },
    ActionSpec { kind: "node.provision", capability: "node.spawn", risk: Risk::Write, description: "Provision new mesh nodes" },
];

impl ActionSpec {
    pub fn lookup(kind: &str) -> Option<&'static ActionSpec> {
        ACTION_CATALOG.iter().find(|s| s.kind == kind)
    }
}

/// A concrete action proposed by an agent or the LLM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub kind: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl Action {
    pub fn new(kind: impl Into<String>, params: serde_json::Value) -> Self {
        Self { kind: kind.into(), params }
    }

    /// Map the intent names produced by `netgene-llm` to catalog actions.
    pub fn from_llm_intent(name: &str, params: serde_json::Value) -> Option<Self> {
        let kind = match name {
            "provision_nodes" => "node.provision",
            "optimize_network" => "network.optimize",
            "trigger_anomaly_scan" => "safeguard.scan",
            "system_status" => "system.status",
            _ => return None,
        };
        Some(Self::new(kind, params))
    }

    pub fn spec(&self) -> Option<&'static ActionSpec> {
        ActionSpec::lookup(&self.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_intents_map_to_catalog() {
        for name in ["provision_nodes", "optimize_network", "trigger_anomaly_scan", "system_status"] {
            let action = Action::from_llm_intent(name, serde_json::json!({})).unwrap();
            assert!(action.spec().is_some(), "{name} must map to a catalog action");
        }
        assert!(Action::from_llm_intent("rm_rf", serde_json::json!({})).is_none());
    }
}
