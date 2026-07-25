//! LLM-backed Intent Parser for NetGene OS.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::client::OllamaClient;
use crate::prompt::INTENT_PARSER_SYSTEM_PROMPT;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmParsedIntent {
    pub action: String,
    pub parameters: serde_json::Value,
    pub explanation: String,
}

#[derive(Clone)]
pub struct LlmIntentEngine {
    client: OllamaClient,
}

impl LlmIntentEngine {
    pub fn new(client: OllamaClient) -> Self {
        Self { client }
    }

    /// Parse user natural language input into structured intent
    pub async fn parse(&self, input: &str) -> Result<LlmParsedIntent> {
        if self.client.ping().await {
            info!("🧠 Processing intent with local Ollama ({})", self.client.model());
            match self.client.chat(INTENT_PARSER_SYSTEM_PROMPT, input).await {
                Ok(raw_resp) => {
                    // Extract JSON string from response
                    let json_str = extract_json(&raw_resp);
                    match serde_json::from_str::<LlmParsedIntent>(&json_str) {
                        Ok(intent) => return Ok(intent),
                        Err(e) => warn!("Failed to parse LLM JSON response: {}. Falling back to rule-based engine.", e),
                    }
                }
                Err(e) => warn!("Ollama chat request failed: {}. Falling back to rule-based engine.", e),
            }
        } else {
            info!("⚡ Ollama offline or unreachable. Using fallback rule-based Intent Engine.");
        }

        // Fallback rule-based parsing
        self.fallback_parse(input)
    }

    fn fallback_parse(&self, input: &str) -> Result<LlmParsedIntent> {
        let input_lower = input.to_lowercase();

        if input_lower.contains("spawn") || input_lower.contains("create") || input_lower.contains("provision") {
            let count = extract_number(&input_lower).unwrap_or(1);
            let template = if input_lower.contains("quantum") {
                "quantum"
            } else if input_lower.contains("core") {
                "core"
            } else if input_lower.contains("gateway") {
                "gateway"
            } else {
                "edge"
            };

            Ok(LlmParsedIntent {
                action: "provision_nodes".to_string(),
                parameters: serde_json::json!({
                    "count": count,
                    "template": template
                }),
                explanation: format!("Fallback: provision {} {} node(s)", count, template),
            })
        } else if input_lower.contains("optimize") || input_lower.contains("route") || input_lower.contains("quantum") {
            let nodes = extract_number(&input_lower).unwrap_or(8);
            Ok(LlmParsedIntent {
                action: "optimize_network".to_string(),
                parameters: serde_json::json!({ "nodes": nodes }),
                explanation: format!("Fallback: optimize routing for network of {} nodes", nodes),
            })
        } else if input_lower.contains("scan") || input_lower.contains("safeguard") || input_lower.contains("heal") {
            Ok(LlmParsedIntent {
                action: "trigger_anomaly_scan".to_string(),
                parameters: serde_json::json!({}),
                explanation: "Fallback: run safeguard anomaly scan".to_string(),
            })
        } else {
            Ok(LlmParsedIntent {
                action: "system_status".to_string(),
                parameters: serde_json::json!({}),
                explanation: "Fallback: query system status".to_string(),
            })
        }
    }
}

fn extract_json(raw: &str) -> String {
    if let Some(start) = raw.find('{') {
        if let Some(end) = raw.rfind('}') {
            return raw[start..=end].to_string();
        }
    }
    raw.to_string()
}

fn extract_number(s: &str) -> Option<u32> {
    s.split_whitespace()
        .find_map(|word| word.parse::<u32>().ok())
}
