//! Agent CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_kernel::NetSphereKernel;

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
    }

    Ok(())
}
