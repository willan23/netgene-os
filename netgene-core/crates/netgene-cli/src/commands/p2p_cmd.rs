//! P2P CLI commands.

use clap::{Args, Subcommand};
use anyhow::Result;
use chrono::Utc;
use netgene_gene::{
    storage::{default_gene_dir, list_genes, load_gene, trusted_lineage},
    HybridPublicKey,
};
use netgene_p2p::{allowlist_from_genes, Inbound, MeshMessage, NetGeneP2PNode, NodeConfig, Outbound, PeerPolicy, GOSSIPSUB_TOPIC};
use tokio::sync::mpsc;

#[derive(Args)]
pub struct NodeIdentity {
    /// Gene to run as (its key becomes the libp2p identity)
    #[arg(long, env = "NETGENE_GENE_ID")]
    gene: String,
    /// Accept any peer instead of only genes of the same lineage (development only)
    #[arg(long)]
    open: bool,
    #[command(flatten)]
    net: NetOptions,
}

/// Discovery and NAT traversal.
#[derive(Args)]
pub struct NetOptions {
    /// Disable mDNS discovery on the local network
    #[arg(long)]
    no_mdns: bool,
    /// Serve as a circuit relay for members (run it on a node with a public address)
    #[arg(long)]
    relay_server: bool,
    /// Member relay to be reachable through, e.g. /ip4/203.0.113.7/tcp/7777/p2p/12D3Koo… (repeatable)
    #[arg(long = "relay")]
    relays: Vec<libp2p::Multiaddr>,
    /// Public address of this node (repeatable); a relay needs one to accept reservations
    #[arg(long = "external-addr")]
    external_addrs: Vec<libp2p::Multiaddr>,
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
    /// Print the libp2p PeerId a gene runs as (derived from its public key)
    PeerId {
        #[arg(long)]
        gene: String,
    },
    /// Broadcast a message over the mesh (encrypted with this node's sender key)
    Broadcast {
        #[command(flatten)]
        identity: NodeIdentity,
        /// Peer to join the mesh through, e.g. /ip4/127.0.0.1/tcp/7777
        #[arg(long)]
        peer: libp2p::Multiaddr,
        /// Local port to bind
        #[arg(short, long, default_value = "7780")]
        port: u16,
        /// Message content
        msg: String,
    },
}

fn build_node(identity: &NodeIdentity, port: u16) -> Result<(NetGeneP2PNode, mpsc::Sender<Outbound>, mpsc::Receiver<Inbound>)> {
    let dir = default_gene_dir();
    let (me, kp) = load_gene(&identity.gene, &dir, &netgene_gene::default_store()?)?;

    let trusted = trusted_lineage(&me, &list_genes(&dir)?);
    let policy = if identity.open {
        PeerPolicy::Open
    } else {
        println!("   Trusted genes: {}", trusted.iter().map(|g| g.short_fp.as_str()).collect::<Vec<_>>().join(", "));
        PeerPolicy::Allowlist(allowlist_from_genes(&trusted)?)
    };

    let config = NodeConfig {
        listen_port: port,
        mdns: !identity.net.no_mdns,
        relay_server: identity.net.relay_server,
        relays: identity.net.relays.clone(),
        external_addrs: identity.net.external_addrs.clone(),
    };
    let (node, out_tx, in_rx) = NetGeneP2PNode::with_config(config, kp, policy)?;
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
            println!("   Topic:    {} (encrypted, sender keys over PQ sessions)", GOSSIPSUB_TOPIC);
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

            // The private message is for the peer named at the end of the address
            // (through a relay circuit, the relay's session comes up first).
            let target = multiaddr.iter().filter_map(|p| match p {
                libp2p::multiaddr::Protocol::P2p(id) => Some(id),
                _ => None,
            }).last();
            if private.is_some() && target.is_none() {
                anyhow::bail!("--private needs an address ending in /p2p/<PeerId>");
            }
            node.dial(multiaddr)?;
            tokio::spawn(async move {
                node.run().await;
            });

            println!("🟢 Dialed peer. Listening for events... (Press Ctrl+C to stop)");
            let mut pending_private = private;
            while let Some(event) = in_rx.recv().await {
                print_inbound(&event);
                if let Inbound::SessionEstablished { peer } = &event {
                    if Some(*peer) == target {
                        if let Some(text) = pending_private.take() {
                            let msg = MeshMessage::IntentBroadcast { sender: sender.clone(), intent: text.clone(), timestamp: Utc::now() };
                            out_tx.send(Outbound::Private { to: *peer, msg }).await?;
                            println!("🔐 Sent private message to {}: \"{}\"", peer, text);
                        }
                    }
                }
            }
        }

        P2pCommand::PeerId { gene } => {
            let record = list_genes(&default_gene_dir())?
                .into_iter()
                .find(|g| g.id.to_string() == gene)
                .ok_or_else(|| anyhow::anyhow!("Gene {} not found", gene))?;
            println!("{}", netgene_p2p::peer_id_for_gene(&record)?);
        }

        P2pCommand::Broadcast { identity, peer, port, msg } => {
            let (mut node, out_tx, mut in_rx) = build_node(&identity, port)?;
            let sender = node.peer_id.to_string();
            node.dial(peer)?;
            tokio::spawn(async move {
                node.run().await;
            });

            // Peers need our sender key (sent over the PQ session) and the gossip
            // mesh needs a heartbeat or two before a publish reaches anyone.
            let session = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                while let Some(event) = in_rx.recv().await {
                    if matches!(event, Inbound::SessionEstablished { .. }) {
                        return true;
                    }
                }
                false
            });
            if !session.await.unwrap_or(false) {
                anyhow::bail!("No post-quantum session with the peer within 20 s");
            }
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;

            let mesh_msg = MeshMessage::IntentBroadcast { sender, intent: msg.clone(), timestamp: Utc::now() };
            out_tx.send(Outbound::Broadcast(mesh_msg)).await?;
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            println!("📡 Broadcasted message: \"{}\"", msg);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use netgene_gene::{GeneRole, NetGene};

    #[test]
    fn test_trusted_genes_follow_lineage() {
        let (master, master_kp) = NetGene::generate_master("root").unwrap();
        let (node_a, _) = NetGene::spawn_sub_gene(&master, &master_kp, "a", GeneRole::Node, vec![]).unwrap();
        let (stranger, _) = NetGene::generate_master("stranger").unwrap();
        let genes = vec![master.clone(), node_a.clone(), stranger.clone()];

        let ids: Vec<_> = trusted_lineage(&node_a, &genes).into_iter().map(|g| g.id).collect();
        assert!(ids.contains(&master.id));
        assert!(ids.contains(&node_a.id));
        assert!(!ids.contains(&stranger.id));
    }
}
