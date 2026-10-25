//! P2P CLI commands.

use clap::{Args, Subcommand};
use anyhow::Result;
use chrono::Utc;
use netgene_gene::{
    secret::OsKeyring,
    storage::{default_gene_dir, list_genes, load_gene},
    HybridPublicKey, NetGene,
};
use netgene_p2p::{allowlist_from_genes, Inbound, MeshMessage, NetGeneP2PNode, Outbound, PeerPolicy};
use tokio::sync::mpsc;

#[derive(Args)]
pub struct NodeIdentity {
    /// Gene to run as (its key becomes the libp2p identity)
    #[arg(long)]
    gene: String,
    /// Accept any peer instead of only genes of the same lineage (development only)
    #[arg(long)]
    open: bool,
}

#[derive(Subcommand)]
pub enum P2pCommand {
    /// Start P2P node and join the mesh
    Listen {
        #[command(flatten)]
        identity: NodeIdentity,
        /// Port to listen on
        #[arg(short, long, default_value = "7777")]
        port: u16,
    },
    /// Connect/Dial another P2P node by multiaddr
    Connect {
        #[command(flatten)]
        identity: NodeIdentity,
        /// Peer multiaddress (e.g., /ip4/127.0.0.1/tcp/7777)
        addr: String,
        /// Local port to bind
        #[arg(short, long, default_value = "7779")]
        port: u16,
        /// Send this text as a private (post-quantum encrypted) message once the session is up
        #[arg(long)]
        private: Option<String>,
    },
    /// Broadcast a test message over the mesh
    Broadcast {
        #[command(flatten)]
        identity: NodeIdentity,
        /// Message content
        msg: String,
    },
}

/// Genes allowed in the mesh: the root of `me`'s lineage plus every local
/// gene whose lineage signature verifies against that root.
fn trusted_genes(me: &NetGene, genes: &[NetGene]) -> Vec<NetGene> {
    let root = match me.parent_id {
        None => Some(me.clone()),
        Some(parent_id) => genes.iter().find(|g| g.id == parent_id && me.verify_lineage(g)).cloned(),
    };
    let Some(root) = root else { return vec![me.clone()] };

    let mut trusted: Vec<NetGene> = genes.iter().filter(|g| g.verify_lineage(&root)).cloned().collect();
    trusted.push(root);
    trusted
}

fn build_node(identity: &NodeIdentity, port: u16) -> Result<(NetGeneP2PNode, mpsc::Sender<Outbound>, mpsc::Receiver<Inbound>)> {
    let dir = default_gene_dir();
    let (me, kp) = load_gene(&identity.gene, &dir, &OsKeyring::new())?;

    let trusted = trusted_genes(&me, &list_genes(&dir)?);
    let policy = if identity.open {
        PeerPolicy::Open
    } else {
        println!("   Trusted genes: {}", trusted.iter().map(|g| g.short_fp.as_str()).collect::<Vec<_>>().join(", "));
        PeerPolicy::Allowlist(allowlist_from_genes(&trusted)?)
    };

    let (node, out_tx, in_rx) = NetGeneP2PNode::new(port, kp, policy)?;
    // Peers must present exactly the (hybrid) key of their local Gene record.
    let node = node.with_pinned_keys(trusted.iter().map(HybridPublicKey::from_gene))?;
    Ok((node, out_tx, in_rx))
}

fn print_inbound(event: &Inbound) {
    match event {
        Inbound::Gossip { from, msg } => println!("📩 [gossip from {}]: {:?}", from.map(|p| p.to_string()).unwrap_or_default(), msg),
        Inbound::Private { from, msg } => println!("🔐 [private from {}]: {:?}", from, msg),
        Inbound::SessionEstablished { peer } => println!("🔐 Post-quantum session (X25519 + ML-KEM-768) established with {}", peer),
    }
}

pub async fn run(cmd: P2pCommand) -> Result<()> {
    match cmd {
        P2pCommand::Listen { identity, port } => {
            println!("🌐 Starting NetGene P2P Node...");
            let (node, _out_tx, mut in_rx) = build_node(&identity, port)?;
            println!("   Peer ID:  {}", node.peer_id);
            println!("   Listen:   0.0.0.0:{}", port);
            println!("   Topic:    netgene-mesh-v1");
            println!();

            tokio::spawn(async move {
                node.run().await;
            });

            println!("🟢 P2P Mesh node running. Listening for events... (Press Ctrl+C to stop)");
            while let Some(event) = in_rx.recv().await {
                print_inbound(&event);
            }
        }

        P2pCommand::Connect { identity, addr, port, private } => {
            let multiaddr: libp2p::Multiaddr = addr.parse()?;
            println!("🌐 Starting NetGene P2P Node on port {}...", port);
            let (mut node, out_tx, mut in_rx) = build_node(&identity, port)?;
            let sender = node.peer_id.to_string();
            println!("   Peer ID:  {}", node.peer_id);
            println!("   Dialing:  {}", multiaddr);
            println!();

            node.dial(multiaddr)?;
            tokio::spawn(async move {
                node.run().await;
            });

            println!("🟢 Dialed peer. Listening for events... (Press Ctrl+C to stop)");
            let mut pending_private = private;
            while let Some(event) = in_rx.recv().await {
                print_inbound(&event);
                if let (Inbound::SessionEstablished { peer }, Some(text)) = (&event, pending_private.take()) {
                    let msg = MeshMessage::IntentBroadcast { sender: sender.clone(), intent: text.clone(), timestamp: Utc::now() };
                    out_tx.send(Outbound::Private { to: *peer, msg }).await?;
                    println!("🔐 Sent private message to {}: \"{}\"", peer, text);
                }
            }
        }

        P2pCommand::Broadcast { identity, msg } => {
            let (node, out_tx, _) = build_node(&identity, 0)?;
            let sender = node.peer_id.to_string();
            tokio::spawn(async move {
                node.run().await;
            });

            let mesh_msg = MeshMessage::IntentBroadcast {
                sender,
                intent: msg.clone(),
                timestamp: Utc::now(),
            };

            out_tx.send(Outbound::Broadcast(mesh_msg)).await?;
            println!("📡 Broadcasted message: \"{}\"", msg);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use netgene_gene::GeneRole;

    #[test]
    fn test_trusted_genes_follow_lineage() {
        let (master, master_kp) = NetGene::generate_master("root").unwrap();
        let (node_a, _) = NetGene::spawn_sub_gene(&master, &master_kp, "a", GeneRole::Node, vec![]).unwrap();
        let (stranger, _) = NetGene::generate_master("stranger").unwrap();
        let genes = vec![master.clone(), node_a.clone(), stranger.clone()];

        let ids: Vec<_> = trusted_genes(&node_a, &genes).into_iter().map(|g| g.id).collect();
        assert!(ids.contains(&master.id));
        assert!(ids.contains(&node_a.id));
        assert!(!ids.contains(&stranger.id));
    }
}
