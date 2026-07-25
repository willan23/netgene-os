//! P2P CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_p2p::{NetGeneP2PNode, MeshMessage};
use chrono::Utc;

#[derive(Subcommand)]
pub enum P2pCommand {
    /// Start P2P node and join the mesh
    Listen {
        /// Port to listen on
        #[arg(short, long, default_value = "7777")]
        port: u16,
    },
    /// Connect/Dial another P2P node by multiaddr
    Connect {
        /// Peer multiaddress (e.g., /ip4/127.0.0.1/tcp/7777)
        addr: String,
        /// Local port to bind
        #[arg(short, long, default_value = "7779")]
        port: u16,
    },
    /// Broadcast a test message over the mesh
    Broadcast {
        /// Message content
        msg: String,
    },
}

pub async fn run(cmd: P2pCommand) -> Result<()> {
    match cmd {
        P2pCommand::Listen { port } => {
            let (node, _out_tx, mut in_rx) = NetGeneP2PNode::new(port)?;
            let peer_id = node.peer_id;
            println!("🌐 Starting NetGene P2P Node...");
            println!("   Peer ID:  {}", peer_id);
            println!("   Listen:   0.0.0.0:{}", port);
            println!("   Topic:    netgene-mesh-v1");
            println!("   Protocols: mDNS + Kademlia DHT + Gossipsub + Identify");
            println!();

            // Run swarm in background task
            tokio::spawn(async move {
                node.run().await;
            });

            println!("🟢 P2P Mesh node running. Listening for events... (Press Ctrl+C to stop)");

            while let Some(msg) = in_rx.recv().await {
                println!("📩 [Mesh Event]: {:?}", msg);
            }
        }

        P2pCommand::Connect { addr, port } => {
            let (mut node, _out_tx, mut in_rx) = NetGeneP2PNode::new(port)?;
            let peer_id = node.peer_id;
            let multiaddr: libp2p::Multiaddr = addr.parse()?;
            
            println!("🌐 Starting NetGene P2P Node on port {}...", port);
            println!("   Peer ID:  {}", peer_id);
            println!("   Dialing:  {}", multiaddr);
            println!();

            node.dial(multiaddr)?;

            tokio::spawn(async move {
                node.run().await;
            });

            println!("🟢 Connected to P2P Mesh. Listening for events... (Press Ctrl+C to stop)");

            while let Some(msg) = in_rx.recv().await {
                println!("📩 [Mesh Event]: {:?}", msg);
            }
        }

        P2pCommand::Broadcast { msg } => {
            let (node, out_tx, _) = NetGeneP2PNode::new(7778)?;
            tokio::spawn(async move {
                node.run().await;
            });

            let mesh_msg = MeshMessage::IntentBroadcast {
                sender: "CLI".to_string(),
                intent: msg.clone(),
                timestamp: Utc::now(),
            };

            out_tx.send(mesh_msg).await?;
            println!("📡 Broadcasted message: \"{}\"", msg);
        }
    }

    Ok(())
}
