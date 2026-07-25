use clap::Subcommand;
use anyhow::Result;
use netgene_cloud::MeshNode;
use std::sync::Arc;

#[derive(Subcommand)]
pub enum CloudCommand {
    /// Start a Cloud P2P Mesh node
    Start {
        #[arg(short, long, default_value_t = 9001)]
        port: u16,
    },
    /// Connect to a peer
    Connect {
        #[arg(short, long)]
        address: String,
    },
}

pub async fn run(command: CloudCommand) -> Result<()> {
    match command {
        CloudCommand::Start { port } => {
            println!("☁️ Starting NetGene Cloud on port {}", port);
            let node = Arc::new(MeshNode::new(port));
            node.start().await?;
        }
        CloudCommand::Connect { address } => {
            println!("🔗 Connecting to Cloud Peer at {}", address);
            let node = Arc::new(MeshNode::new(9002));
            node.connect_to_peer(&address).await?;
        }
    }
    Ok(())
}
