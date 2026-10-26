//! # NetGene LLM
//!
//! Local Ollama client integration and natural language Intent Engine 2.0.

pub mod client;
pub mod prompt;
pub mod intent;

pub use client::OllamaClient;
pub use prompt::*;
pub use intent::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> LlmIntentEngine {
        // Point to a non-existent Ollama endpoint to force fallback
        let client = OllamaClient::new(Some("http://localhost:59999"), None);
        LlmIntentEngine::new(client)
    }

    #[tokio::test]
    async fn test_fallback_provision_nodes() {
        let e = engine();
        let intent = e.parse("spawn 3 gateway nodes with HA").await.unwrap();
        assert_eq!(intent.action, "provision_nodes");
        assert_eq!(intent.parameters["count"], 3);
        assert_eq!(intent.parameters["template"], "gateway");
    }

    #[tokio::test]
    async fn test_fallback_optimize_is_not_executable() {
        let e = engine();
        let intent = e.parse("optimize routes for 10 nodes").await.unwrap();
        assert_eq!(intent.action, "unknown");
    }

    #[tokio::test]
    async fn test_fallback_agents() {
        let e = engine();
        assert_eq!(e.parse("list agents").await.unwrap().action, "list_agents");
        let run = e.parse("run agent echo-bot with Hello World").await.unwrap();
        assert_eq!(run.action, "run_agent");
        assert_eq!(run.parameters["agent"], "echo-bot");
        assert_eq!(run.parameters["input"], "Hello World");
    }

    #[tokio::test]
    async fn test_fallback_anomaly_scan() {
        let e = engine();
        let intent = e.parse("heal the network now").await.unwrap();
        assert_eq!(intent.action, "trigger_anomaly_scan");
    }

    #[tokio::test]
    async fn test_fallback_system_status() {
        let e = engine();
        let intent = e.parse("what is the system doing?").await.unwrap();
        assert_eq!(intent.action, "system_status");
    }

    #[tokio::test]
    async fn test_fallback_create_gateway() {
        let e = engine();
        let intent = e.parse("create 2 gateway nodes").await.unwrap();
        assert_eq!(intent.action, "provision_nodes");
        assert_eq!(intent.parameters["template"], "gateway");
        assert_eq!(intent.parameters["count"], 2);
    }

}
