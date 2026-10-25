//! # NetGene CLI
//!
//! Main entry point for NetGene OS.
//! Each subcommand maps to one core crate of the workspace.

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod commands;
use commands::{gene, agent, store_cmd, llm_cmd, p2p_cmd, wasm_cmd, vault_cmd};

/// NetGene OS — private agent mesh
#[derive(Parser)]
#[command(
    name = "netgene",
    version,
    author = "NetGene OS contributors",
    about = "🧬 NetGene OS — private mesh of AI agents with cryptographic identity",
    long_about = r#"
NetGene OS — private mesh of AI agents with cryptographic identity.

Pre-alpha software. Only the commands listed here exist; see README.md
for the real status of each module before relying on it.
"#,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// JSON output format
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// 🔑 Gene Layer — Identity and cryptographic key management
    Gene {
        #[command(subcommand)]
        action: gene::GeneCommand,
    },
    /// 🤖 Agent management — Netsphere Kernel agents
    Agent {
        #[command(subcommand)]
        action: agent::AgentCommand,
    },
    /// 📂 Persistent Store (redb)
    Store {
        #[command(subcommand)]
        action: store_cmd::StoreCommand,
    },
    /// 🧠 Local LLM & Intent Engine (Ollama)
    Llm {
        #[command(subcommand)]
        action: llm_cmd::LlmCommand,
    },
    /// 🌐 P2P Mesh Network (libp2p)
    P2p {
        #[command(subcommand)]
        action: p2p_cmd::P2pCommand,
    },
    /// ⚙️  WASM Sandbox (experimental)
    Wasm {
        #[command(subcommand)]
        action: wasm_cmd::WasmCommand,
    },
    /// 📦 Encrypted local Vault (experimental)
    Vault {
        #[command(subcommand)]
        action: vault_cmd::VaultCommand,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .with_writer(std::io::stderr)
        .compact()
        .init();

    match cli.command {
        Commands::Gene { action } => gene::run(action, cli.json).await?,
        Commands::Agent { action } => agent::run(action, cli.json).await?,
        Commands::Store { action } => store_cmd::run(action, cli.json).await?,
        Commands::Llm { action } => llm_cmd::run(action).await?,
        Commands::P2p { action } => p2p_cmd::run(action).await?,
        Commands::Wasm { action } => wasm_cmd::run(action).await?,
        Commands::Vault { action } => vault_cmd::run(action).await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    /// Catches clap definition conflicts (duplicate flags, bad defaults) in every subcommand.
    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }
}
