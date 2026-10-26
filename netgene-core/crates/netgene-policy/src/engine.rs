//! Policy evaluation.

use std::path::Path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use netgene_gene::{GeneRole, GeneStatus, NetGene};

use crate::action::{Action, Risk};

/// Outcome of a policy rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    Allow,
    /// Allowed only after explicit human confirmation.
    Confirm,
    Deny,
}

/// A rule; the first matching rule wins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    /// Exact action kind, a prefix pattern like `node.*`, or `*`.
    pub action: String,
    pub effect: Effect,
    /// Roles the rule applies to (empty = every role).
    #[serde(default)]
    pub roles: Vec<GeneRole>,
}

impl Rule {
    fn matches(&self, action: &Action, actor: &NetGene) -> bool {
        let action_matches = match self.action.strip_suffix('*') {
            Some(prefix) => action.kind.starts_with(prefix),
            None => action.kind == self.action,
        };
        action_matches && (self.roles.is_empty() || self.roles.contains(&actor.role))
    }
}

/// Policy document (stored as JSON, e.g. `~/.netgene/policy.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    pub version: u32,
    /// Ordered rules; when none matches, the default for the action's risk applies.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Upper bound for `node.provision` `count`.
    #[serde(default = "default_max_nodes")]
    pub max_nodes_per_request: u64,
    /// Confirmations must carry a hardware user-presence proof (e.g. Windows Hello).
    #[serde(default)]
    pub require_user_presence: bool,
    /// Sandbox permissions `agent.run` may grant (`log`, `io`, `kv`, `clock`).
    #[serde(default = "default_agent_permissions")]
    pub agent_permissions: Vec<String>,
}

fn default_max_nodes() -> u64 {
    10
}

fn default_agent_permissions() -> Vec<String> {
    ["log", "io", "kv", "clock"].map(String::from).to_vec()
}

/// Largest `agent.run` input accepted.
pub const MAX_AGENT_INPUT_BYTES: usize = 64 * 1024;

impl Default for Policy {
    /// Reads are allowed, anything that changes state needs confirmation.
    fn default() -> Self {
        Self {
            version: 1,
            rules: vec![],
            max_nodes_per_request: default_max_nodes(),
            require_user_presence: false,
            agent_permissions: default_agent_permissions(),
        }
    }
}

impl Policy {
    /// Load a policy file, or the default policy if it does not exist.
    pub fn load_or_default(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        serde_json::from_str(&text).with_context(|| format!("Invalid policy file {}", path.display()))
    }
}

/// Capabilities from a capability token the caller has already verified.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grant {
    pub token_id: uuid::Uuid,
    pub capabilities: Vec<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl Grant {
    pub fn is_active(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        now < self.expires_at
    }
}

/// Result of evaluating an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Confirm { reason: String },
    Deny { reason: String },
}

impl Decision {
    pub fn label(&self) -> &'static str {
        match self {
            Decision::Allow => "allow",
            Decision::Confirm { .. } => "confirm",
            Decision::Deny { .. } => "deny",
        }
    }
}

pub struct PolicyEngine {
    policy: Policy,
}

impl PolicyEngine {
    pub fn new(policy: Policy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Decide whether `actor` may perform `action`.
    pub fn evaluate(&self, actor: &NetGene, action: &Action) -> Decision {
        self.evaluate_with_grants(actor, &[], action)
    }

    /// Like [`Self::evaluate`], also counting capabilities granted to the
    /// actor by verified, unexpired capability tokens.
    pub fn evaluate_with_grants(&self, actor: &NetGene, granted: &[String], action: &Action) -> Decision {
        let Some(spec) = action.spec() else {
            return Decision::Deny { reason: format!("'{}' is not in the action catalog", action.kind) };
        };

        if actor.status != GeneStatus::Active {
            return Decision::Deny { reason: format!("actor gene is {}", actor.status) };
        }
        // The Master Gene holds every capability; delegated genes need it explicitly or via a token.
        let has_capability = actor.role == GeneRole::Master
            || actor.has_capability(spec.capability)
            || granted.iter().any(|c| c == spec.capability);
        if !has_capability {
            return Decision::Deny { reason: format!("actor lacks capability '{}'", spec.capability) };
        }

        if action.kind == "node.provision" {
            let count = action.params.get("count").and_then(|c| c.as_u64()).unwrap_or(1);
            if count == 0 || count > self.policy.max_nodes_per_request {
                return Decision::Deny {
                    reason: format!("node count {} outside 1..={}", count, self.policy.max_nodes_per_request),
                };
            }
        }

        if action.kind == "agent.run" {
            let input = action.params.get("input").and_then(|i| i.as_str()).unwrap_or_default();
            if input.len() > MAX_AGENT_INPUT_BYTES {
                return Decision::Deny { reason: format!("agent input larger than {} bytes", MAX_AGENT_INPUT_BYTES) };
            }
            if action.params.get("agent").and_then(|a| a.as_str()).is_none() {
                return Decision::Deny { reason: "agent.run needs an 'agent' name".into() };
            }
        }

        let effect = self
            .policy
            .rules
            .iter()
            .find(|r| r.matches(action, actor))
            .map(|r| r.effect)
            .unwrap_or(match spec.risk {
                Risk::Read => Effect::Allow,
                Risk::Write | Risk::Destructive => Effect::Confirm,
            });

        match effect {
            Effect::Allow => Decision::Allow,
            Effect::Confirm => Decision::Confirm { reason: format!("'{}' changes state ({:?} risk)", spec.kind, spec.risk) },
            Effect::Deny => Decision::Deny { reason: format!("policy rule denies '{}'", spec.kind) },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn master() -> NetGene {
        NetGene::generate_master("operator").unwrap().0
    }

    fn observer() -> NetGene {
        let (m, kp) = NetGene::generate_master("root").unwrap();
        NetGene::spawn_sub_gene(&m, &kp, "obs", GeneRole::Observer, vec!["network.read".into()]).unwrap().0
    }

    #[test]
    fn test_default_policy_reads_allowed_writes_confirmed() {
        let engine = PolicyEngine::new(Policy::default());
        let m = master();
        assert_eq!(engine.evaluate(&m, &Action::new("system.status", json!({}))), Decision::Allow);
        assert!(matches!(engine.evaluate(&m, &Action::new("node.provision", json!({"count": 2}))), Decision::Confirm { .. }));
    }

    #[test]
    fn test_unknown_action_denied() {
        let engine = PolicyEngine::new(Policy::default());
        assert!(matches!(engine.evaluate(&master(), &Action::new("shell.exec", json!({}))), Decision::Deny { .. }));
    }

    #[test]
    fn test_missing_capability_denied() {
        let engine = PolicyEngine::new(Policy::default());
        let obs = observer();
        assert_eq!(engine.evaluate(&obs, &Action::new("safeguard.scan", json!({}))), Decision::Allow);
        assert!(matches!(engine.evaluate(&obs, &Action::new("agent.run", json!({"agent": "x"}))), Decision::Deny { .. }));
    }

    #[test]
    fn test_granted_capability_allows() {
        let engine = PolicyEngine::new(Policy::default());
        let obs = observer();
        let action = Action::new("agent.run", json!({"agent": "x"}));
        assert!(matches!(engine.evaluate(&obs, &action), Decision::Deny { .. }));
        assert!(matches!(
            engine.evaluate_with_grants(&obs, &["agent.run".to_string()], &action),
            Decision::Confirm { .. }
        ));
    }

    #[test]
    fn test_revoked_actor_denied() {
        let engine = PolicyEngine::new(Policy::default());
        let mut m = master();
        m.revoke();
        assert!(matches!(engine.evaluate(&m, &Action::new("system.status", json!({}))), Decision::Deny { .. }));
    }

    #[test]
    fn test_node_count_bounded() {
        let engine = PolicyEngine::new(Policy::default());
        assert!(matches!(
            engine.evaluate(&master(), &Action::new("node.provision", json!({"count": 10_000}))),
            Decision::Deny { .. }
        ));
    }

    #[test]
    fn test_agent_run_params_checked() {
        let engine = PolicyEngine::new(Policy::default());
        let m = master();
        assert!(matches!(engine.evaluate(&m, &Action::new("agent.run", json!({}))), Decision::Deny { .. }));
        let big = "x".repeat(MAX_AGENT_INPUT_BYTES + 1);
        assert!(matches!(engine.evaluate(&m, &Action::new("agent.run", json!({"agent": "a", "input": big}))), Decision::Deny { .. }));
        assert!(matches!(engine.evaluate(&m, &Action::new("agent.run", json!({"agent": "a"}))), Decision::Confirm { .. }));
    }

    #[test]
    fn test_rules_first_match_wins() {
        let policy = Policy {
            rules: vec![
                Rule { action: "node.*".into(), effect: Effect::Allow, roles: vec![GeneRole::Master] },
                Rule { action: "*".into(), effect: Effect::Deny, roles: vec![] },
            ],
            ..Policy::default()
        };
        let engine = PolicyEngine::new(policy);
        let m = master();
        assert_eq!(engine.evaluate(&m, &Action::new("node.provision", json!({"count": 1}))), Decision::Allow);
        assert!(matches!(engine.evaluate(&m, &Action::new("system.status", json!({}))), Decision::Deny { .. }));
    }

    #[test]
    fn test_policy_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        assert_eq!(Policy::load_or_default(&path).unwrap(), Policy::default());

        let custom = Policy { max_nodes_per_request: 3, ..Policy::default() };
        std::fs::write(&path, serde_json::to_string(&custom).unwrap()).unwrap();
        assert_eq!(Policy::load_or_default(&path).unwrap().max_nodes_per_request, 3);
    }
}
