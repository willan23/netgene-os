//! LLM CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_llm::{OllamaClient, LlmIntentEngine, NETGENE_ASSISTANT_SYSTEM_PROMPT};

#[derive(Subcommand)]
pub enum LlmCommand {
    /// Check local Ollama status
    Status,
    /// Parse a natural language intent with LLM
    Parse {
        /// Intent prompt string (optional, defaults to demo intent)
        #[arg(default_value = "spawn 3 quantum nodes with high availability")]
        prompt: String,
    },
    /// Interactive chat with local NetGene LLM
    Chat {
        /// Message to send (optional, defaults to greeting)
        #[arg(default_value = "Olá! Apresenta-te e explica resumidamente a tua função no NetGene OS.")]
        message: String,
    },
}

pub async fn run(cmd: LlmCommand) -> Result<()> {
    let client = OllamaClient::new(None, None);

    match cmd {
        LlmCommand::Status => {
            let online = client.ping().await;
            println!("🧠 Local Ollama Status:");
            println!("   Endpoint: {}", client.base_url());
            println!("   Model:    {}", client.model());
            if online {
                println!("   Status:   🟢 ONLINE");
            } else {
                println!("   Status:   🔴 OFFLINE (Falling back to rule-based intent engine)");
            }
        }

        LlmCommand::Parse { prompt } => {
            let engine = LlmIntentEngine::new(client);
            println!("🧠 Parsing intent: \"{}\"...", prompt);
            let intent = engine.parse(&prompt).await?;
            println!();
            println!("   Action:      {}", intent.action);
            println!("   Explanation: {}", intent.explanation);
            println!("   Parameters:  {}", serde_json::to_string_pretty(&intent.parameters)?);
        }

        LlmCommand::Chat { message } => {
            if !client.ping().await {
                println!("❌ Ollama local server is offline at {}. Start Ollama (`ollama serve`) to chat.", client.base_url());
                return Ok(());
            }

            println!("🧬 Requesting response from Ollama ({})...", client.model());
            let resp = client.chat(NETGENE_ASSISTANT_SYSTEM_PROMPT, &message).await?;
            println!();
            println!("{}", resp);
        }
    }

    Ok(())
}
