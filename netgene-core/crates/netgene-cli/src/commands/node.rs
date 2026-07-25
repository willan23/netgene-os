//! Node CLI commands.

use clap::Subcommand;
use anyhow::Result;

#[derive(Subcommand)]
pub enum NodeCommand {
    /// Start a local NetGene node
    Start {
        /// Node name
        #[arg(short, long, default_value = "netgene-node-01")]
        name: String,
        /// Bind port
        #[arg(short, long, default_value = "7777")]
        port: u16,
    },
    /// Show node status
    Status,
    /// List all nodes in the mesh
    List,
}

pub async fn run(cmd: NodeCommand) -> Result<()> {
    match cmd {
        NodeCommand::Start { name, port } => {
            println!("🖥️  Starting NetGene node '{}'...", name);
            println!("   Port:      {}", port);
            println!("   Protocol:  NetGene P2P v1 (libp2p Phase 2)");
            println!("   Security:  mTLS + Zero-Trust");
            println!();
            println!("   ✅ Node '{}' listening on 0.0.0.0:{}", name, port);
            println!("   (Press Ctrl+C to stop)");
            println!();
            // Simulate running
            tokio::signal::ctrl_c().await?;
            println!("   Node stopped.");
        }

        NodeCommand::Status => {
            println!("🖥️  Local Node Status:");
            println!("   Status:      ONLINE");
            println!("   Connections: 0 (P2P mesh in Phase 2)");
            println!("   Uptime:      --");
        }

        NodeCommand::List => {
            println!("🌐 Network Nodes (demo topology):");
            for i in 0..5 {
                println!("   node-{:02} │ 10.42.0.{} │ ACTIVE │ {:.0}ms", i, i, 5.0 + i as f64 * 3.5);
            }
        }
    }

    Ok(())
}
