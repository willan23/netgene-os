//! Agent CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_kernel::NetSphereKernel;
use netgene_kernel::marketplace::{AgentManifest, Marketplace};
use netgene_gene::{secret::OsKeyring, storage::{default_gene_dir, load_gene}};

#[derive(Subcommand)]
pub enum AgentCommand {
    /// List all active agents
    List,
    /// Dispatch a natural language intent to agents
    Dispatch {
        /// Intent string (e.g. "spawn 2 nodes", "optimize routes")
        intent: String,
    },
    /// Show agent system status
    Status,
    /// Sign and publish an agent manifest to the local marketplace
    Publish {
        /// ID of the publishing gene
        #[arg(long)]
        gene: String,
        /// Agent id (a-z, 0-9, '-')
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Capabilities, comma separated
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
    },
    /// List verified agents in the local marketplace
    Market,
}

pub async fn run(cmd: AgentCommand, _json: bool) -> Result<()> {
    match cmd {
        AgentCommand::List => {
            let kernel = NetSphereKernel::boot().await?;
            println!("🤖 Netsphere Kernel Agents");
            println!("{}", "─".repeat(50));
            for agent in kernel.agent_list().await {
                println!("  • {} [{}] — {:?}", agent.name, agent.agent_type, agent.status);
            }
            kernel.shutdown().await?;
        }

        AgentCommand::Dispatch { intent } => {
            println!("🧠 Booting Netsphere Kernel...");
            let kernel = NetSphereKernel::boot().await?;

            println!("📡 Dispatching intent: \"{}\"", intent);
            let result = kernel.dispatch_intent(&intent).await?;
            println!("{}", result);

            // Give agents time to process
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

            kernel.shutdown().await?;
        }

        AgentCommand::Status => {
            let kernel = NetSphereKernel::boot().await?;
            println!("🟢 Netsphere Kernel: ONLINE (in-process)");
            println!("   Agents registered: {}", kernel.agent_list().await.len());
            println!("   Bus: Tokio MPSC");
            println!("   Consensus: none (single process)");
            kernel.shutdown().await?;
        }

        AgentCommand::Publish { gene, id, name, version, description, capabilities } => {
            let (publisher, kp) = load_gene(&gene, &default_gene_dir(), &OsKeyring::new())?;
            let manifest = AgentManifest::new_signed(&id, &name, &version, &description, capabilities, &publisher, &kp)?;
            Marketplace::new().publish_manifest(&manifest)?;
            println!("✅ Published '{}' v{} signed by {}", manifest.id, manifest.version, publisher.short_fp);
        }

        AgentCommand::Market => {
            let agents = Marketplace::new().list_available_agents();
            println!("🛒 Marketplace — {} verified agent(s)", agents.len());
            for a in agents {
                println!("  • {} v{} — {} [publisher {}]", a.id, a.version, a.name, a.publisher_fingerprint());
            }
        }
    }

    Ok(())
}
