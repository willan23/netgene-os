//! Natural language intent parsing.
//!
//! Converts free-form user commands into structured `Intent` actions
//! that the kernel can route to the appropriate agents.

use serde::{Deserialize, Serialize};

/// Structured action derived from parsed intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentAction {
    SpawnNode { count: u32 },
    OptimizeRoutes,
    StatusReport,
    SpawnAgent { agent_type: String },
    ShowNetwork,
    RunQuantum { problem: String },
    HealNetwork,
    Unknown { raw: String },
}

/// A parsed user intent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    /// Original text input.
    pub raw: String,
    /// Parsed action.
    pub action: IntentAction,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f32,
}

/// Intent parser that maps natural language to structured actions.
pub struct IntentParser;

impl IntentParser {
    /// Parse a user command string into an Intent.
    pub fn parse(input: &str) -> Intent {
        let lower = input.to_lowercase();

        // Spawn node variants
        if lower.contains("spawn") || lower.contains("criar") || lower.contains("create") {
            if lower.contains("node") || lower.contains("nó") || lower.contains("no") {
                let count = Self::extract_number(&lower).unwrap_or(1);
                return Intent {
                    raw: input.to_string(),
                    action: IntentAction::SpawnNode { count },
                    confidence: 0.92,
                };
            }
            if lower.contains("agent") || lower.contains("agente") {
                let agent_type = if lower.contains("monitor") { "monitor" }
                    else if lower.contains("optim") { "optimizer" }
                    else { "builder" };
                return Intent {
                    raw: input.to_string(),
                    action: IntentAction::SpawnAgent { agent_type: agent_type.to_string() },
                    confidence: 0.88,
                };
            }
        }

        // Optimize / quantum
        if lower.contains("optim") || lower.contains("route") || lower.contains("rota") {
            if lower.contains("quantum") || lower.contains("quântic") {
                return Intent {
                    raw: input.to_string(),
                    action: IntentAction::RunQuantum { problem: "routing".to_string() },
                    confidence: 0.85,
                };
            }
            return Intent {
                raw: input.to_string(),
                action: IntentAction::OptimizeRoutes,
                confidence: 0.90,
            };
        }

        // Status
        if lower.contains("status") || lower.contains("estado") || lower.contains("info") || lower.contains("list") {
            return Intent {
                raw: input.to_string(),
                action: IntentAction::StatusReport,
                confidence: 0.95,
            };
        }

        // Network view
        if lower.contains("rede") || lower.contains("network") || lower.contains("topology") || lower.contains("topologia") {
            return Intent {
                raw: input.to_string(),
                action: IntentAction::ShowNetwork,
                confidence: 0.87,
            };
        }

        // Healing
        if lower.contains("heal") || lower.contains("recover") || lower.contains("fix") || lower.contains("reparar") {
            return Intent {
                raw: input.to_string(),
                action: IntentAction::HealNetwork,
                confidence: 0.83,
            };
        }

        Intent {
            raw: input.to_string(),
            action: IntentAction::Unknown { raw: input.to_string() },
            confidence: 0.0,
        }
    }

    /// Extract the first number from a string.
    fn extract_number(s: &str) -> Option<u32> {
        s.split_whitespace()
            .find_map(|word| word.parse::<u32>().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_spawn_node() {
        let intent = IntentParser::parse("criar 3 nós na rede");
        assert!(matches!(intent.action, IntentAction::SpawnNode { count: 3 }));
        assert!(intent.confidence > 0.5);
    }

    #[test]
    fn test_parse_optimize() {
        let intent = IntentParser::parse("optimize routes now");
        assert_eq!(intent.action, IntentAction::OptimizeRoutes);
    }

    #[test]
    fn test_parse_quantum() {
        let intent = IntentParser::parse("run quantum optimization");
        assert!(matches!(intent.action, IntentAction::RunQuantum { .. }));
    }

    #[test]
    fn test_parse_status() {
        let intent = IntentParser::parse("status");
        assert_eq!(intent.action, IntentAction::StatusReport);
    }
}
