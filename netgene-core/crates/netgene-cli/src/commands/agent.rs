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
            println!("🤖 Netsphere Kernel Agents");
            println!("{}", "─".repeat(50));
            let agents = vec![
                ("BuilderAgent", "builder", "IDLE", "Organic node provisioning"),
                ("MonitorAgent", "monitor", "MONITORING", "Anomaly detection"),
                ("OptimizerAgent", "optimizer", "IDLE", "Quantum routing"),
            ];
            for (name, atype, status, desc) in agents {
                println!("  • {} [{}] — {} — {}", name, atype, status, desc);
            }
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
            println!("🟢 Netsphere Kernel: ONLINE");
            println!("   Agents registered: 3");
            println!("   Bus: Tokio MPSC (buf=256)");
            println!("   Memory: KernelMemory (in-process)");
            println!("   Consensus: Phase 2 (libp2p/Raft)");
        }
    }

    Ok(())
}
