//! # NetGene CLI — v1.0.0 Apex Megastructure
//!
//! Main entry point for NetGene OS.
//! Provides subcommands for all 17 system layers.

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod commands;
use commands::{
    gene, agent, node, quantum_cmd, builder_cmd, store_cmd, llm_cmd, p2p_cmd,
    neural_cmd, wasm_cmd, ebpf_cmd, k8s_cmd, qpu_cmd, mobile_cmd, dao_cmd,
    cloud_cmd, lite_cmd, tpm_cmd, vault_cmd, seed_cmd,
};

/// NetGene OS — Apex Megastructure (v1.0.0)
#[derive(Parser)]
#[command(
    name = "netgene",
    version = "1.0.0",
    author = "NetGene OS Team",
    about = "🧬 NetGene OS — Living, Self-Evolving Autonomous Operating System",
    long_about = r#"
╔═══════════════════════════════════════════════════════╗
║   🧬  NetGene OS v1.0.0 — Apex Megastructure (21 Crates) ║
║   Living · Self-Evolving · Quantum-Enhanced Network   ║
╚═══════════════════════════════════════════════════════╝

NetGene OS is a self-evolving distributed operating system combining
multi-agent AI, QAOA/OpenQASM 3.0 quantum optimization, libp2p mesh,
Ollama local LLM, BCI Neural interface, eBPF kernel security, WASM sandbox,
Kubernetes CRDs Operator, Passkey WebAuthn biometrics, and P2P DAO governance.

☁️ New Extensions (Cloud, Lite, TPM, Vault, Seed) are now fully integrated!
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
    /// 🖥️  Node management
    Node {
        #[command(subcommand)]
        action: node::NodeCommand,
    },
    /// 🤖 Agent management — Netsphere Kernel agents
    Agent {
        #[command(subcommand)]
        action: agent::AgentCommand,
    },
    /// ⚛️  Quantum Enhancement Module (Simulated QAOA / SQA)
    Quantum {
        #[command(subcommand)]
        action: quantum_cmd::QuantumCommand,
    },
    /// 🏗️  Builder Engine — Organic node provisioning
    Build {
        #[command(subcommand)]
        action: builder_cmd::BuilderCommand,
    },
    /// 📂 Persistent Store (Sled DB)
    Store {
        #[command(subcommand)]
        action: store_cmd::StoreCommand,
    },
    /// 🧠 Local LLM & Intent Engine 2.0 (Ollama)
    Llm {
        #[command(subcommand)]
        action: llm_cmd::LlmCommand,
    },
    /// 🌐 P2P Mesh Network (libp2p)
    P2p {
        #[command(subcommand)]
        action: p2p_cmd::P2pCommand,
    },
    /// 🧠 Neural & BCI Intent Adapter
    Neural {
        #[command(subcommand)]
        action: neural_cmd::NeuralCommand,
    },
    /// ⚙️  WASM Sandbox & Organic Code Replication
    Wasm {
        #[command(subcommand)]
        action: wasm_cmd::WasmCommand,
    },
    /// 🛡️ eBPF Kernel Security Probes
    Ebpf {
        #[command(subcommand)]
        action: ebpf_cmd::EbpfCommand,
    },
    /// ☸️  Kubernetes Operator & CRD Manifest Generator
    K8s {
        #[command(subcommand)]
        action: k8s_cmd::K8sCommand,
    },
    /// ⚛️ Physical QPU Connectors & OpenQASM 3.0 Transpiler
    Qpu {
        #[command(subcommand)]
        action: qpu_cmd::QpuCommand,
    },
    /// 📱 Mobile PWA & WebAuthn Passkey Engine
    Mobile {
        #[command(subcommand)]
        action: mobile_cmd::MobileCommand,
    },
    /// 🏛️ P2P Autonomous Governance & Proof-of-Utility DAO
    Dao {
        #[command(subcommand)]
        action: dao_cmd::DaoCommand,
    },
    /// ☁️  Cloud P2P Mesh Node
    Cloud {
        #[command(subcommand)]
        action: cloud_cmd::CloudCommand,
    },
    /// 📡 IoT Lite Node Configuration
    Lite {
        #[command(subcommand)]
        action: lite_cmd::LiteCommand,
    },
    /// 🔒 TPM Hardware Enclave Integration
    Tpm {
        #[command(subcommand)]
        action: tpm_cmd::TpmCommand,
    },
    /// 📦 Encrypted P2P Vault
    Vault {
        #[command(subcommand)]
        action: vault_cmd::VaultCommand,
    },
    /// 🌱 Seed real-world demo data for 100% completion
    Seed {
        #[command(subcommand)]
        action: seed_cmd::SeedCommand,
    },
    /// 📟 Launch interactive TUI dashboard
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(filter))
        .with_target(false)
        .compact()
        .init();

    match cli.command {
        Commands::Gene { action } => gene::run(action, cli.json).await?,
        Commands::Node { action } => node::run(action).await?,
        Commands::Agent { action } => agent::run(action, cli.json).await?,
        Commands::Quantum { action } => quantum_cmd::run(action, cli.json).await?,
        Commands::Build { action } => builder_cmd::run(action).await?,
        Commands::Store { action } => store_cmd::run(action, cli.json).await?,
        Commands::Llm { action } => llm_cmd::run(action).await?,
        Commands::P2p { action } => p2p_cmd::run(action).await?,
        Commands::Neural { action } => neural_cmd::run(action).await?,
        Commands::Wasm { action } => wasm_cmd::run(action).await?,
        Commands::Ebpf { action } => ebpf_cmd::run(action).await?,
        Commands::K8s { action } => k8s_cmd::run(action).await?,
        Commands::Qpu { action } => qpu_cmd::run(action).await?,
        Commands::Mobile { action } => mobile_cmd::run(action).await?,
        Commands::Dao { action } => dao_cmd::run(action).await?,
        Commands::Cloud { action } => cloud_cmd::run(action).await?,
        Commands::Lite { action } => lite_cmd::run(action).await?,
        Commands::Tpm { action } => tpm_cmd::run(action).await?,
        Commands::Vault { action } => vault_cmd::run(action).await?,
        Commands::Seed { action } => seed_cmd::run(action).await?,
        Commands::Tui => {
            println!("🧬 Launching NetGene TUI Dashboard...");
            netgene_tui::run_tui().await?;
        }
    }

    Ok(())
}
