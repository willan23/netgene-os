//! Three real nodes on localhost: A ↔ B ↔ C.
//!
//! Checks that encrypted gossip from A reaches B and C (C learns A's sender
//! key through B if they are not directly connected), that private messages
//! work, and that a node outside the allowlist cannot read the mesh.

use std::collections::HashSet;
use std::time::Duration;
use chrono::Utc;
use libp2p::{Multiaddr, PeerId};
use netgene_gene::NetGene;
use netgene_p2p::{allowlist_from_genes, Inbound, MeshMessage, NetGeneP2PNode, NodeConfig, Outbound, PeerPolicy};
use tokio::sync::mpsc;

/// A port free for both TCP and UDP (QUIC), chosen by the OS.
fn free_port() -> u16 {
    loop {
        let tcp = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let port = tcp.local_addr().unwrap().port();
        if std::net::UdpSocket::bind(("0.0.0.0", port)).is_ok() {
            return port;
        }
    }
}

fn intent(text: &str) -> MeshMessage {
    MeshMessage::IntentBroadcast { sender: "test".into(), intent: text.into(), timestamp: Utc::now() }
}

struct Started {
    peer: PeerId,
    tx: mpsc::Sender<Outbound>,
    rx: mpsc::Receiver<Inbound>,
}

fn config(port: u16) -> NodeConfig {
    // No mDNS: peers only meet through the addresses each test dials.
    NodeConfig { mdns: false, ..NodeConfig::new(port) }
}

fn start(gene: (NetGene, netgene_gene::crypto::GeneKeyPair), port: u16, members: &HashSet<PeerId>, dial: Option<u16>) -> Started {
    let dial = dial.map(|p| format!("/ip4/127.0.0.1/tcp/{}", p).parse::<Multiaddr>().unwrap());
    start_with(gene, config(port), members, dial)
}

fn start_with(gene: (NetGene, netgene_gene::crypto::GeneKeyPair), config: NodeConfig, members: &HashSet<PeerId>, dial: Option<Multiaddr>) -> Started {
    let (node, tx, rx) = NetGeneP2PNode::with_config(config, gene.1, PeerPolicy::Allowlist(members.clone())).unwrap();
    let mut node = node;
    if let Some(addr) = dial {
        node.dial(addr).unwrap();
    }
    let peer = node.peer_id;
    tokio::spawn(node.run());
    Started { peer, tx, rx }
}

/// Wait for a gossip with `text`, re-broadcasting from `sender` meanwhile
/// (the gossipsub mesh needs a few heartbeats to form).
async fn await_gossip(sender: &mpsc::Sender<Outbound>, rx: &mut mpsc::Receiver<Inbound>, from: PeerId, text: &str) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    while tokio::time::Instant::now() < deadline {
        sender.send(Outbound::Broadcast(intent(text))).await.unwrap();
        let wait = tokio::time::sleep(Duration::from_millis(700));
        tokio::pin!(wait);
        loop {
            tokio::select! {
                Some(ev) = rx.recv() => {
                    if let Inbound::Gossip { from: Some(f), msg: MeshMessage::IntentBroadcast { intent, .. } } = ev {
                        if f == from && intent == text {
                            return true;
                        }
                    }
                }
                _ = &mut wait => break,
            }
        }
    }
    false
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_encrypted_gossip_and_private_messages_across_three_nodes() {
    let a = NetGene::generate_master("a").unwrap();
    let b = NetGene::generate_master("b").unwrap();
    let c = NetGene::generate_master("c").unwrap();
    let members = allowlist_from_genes([&a.0, &b.0, &c.0]).unwrap();

    let pb = free_port();
    let mut nb = start(b, pb, &members, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut na = start(a, free_port(), &members, Some(pb));
    let mut nc = start(c, free_port(), &members, Some(pb));

    assert!(await_gossip(&na.tx, &mut nb.rx, na.peer, "a->b").await, "B reads A's encrypted gossip");
    assert!(await_gossip(&na.tx, &mut nc.rx, na.peer, "a->c").await, "C reads A's encrypted gossip");
    assert!(await_gossip(&nc.tx, &mut na.rx, nc.peer, "c->a").await, "A reads C's encrypted gossip");

    // Private message B -> C over the PQ session.
    nb.tx.send(Outbound::Private { to: nc.peer, msg: intent("secret b->c") }).await.unwrap();
    let got = tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(ev) = nc.rx.recv().await {
            if let Inbound::Private { from, msg: MeshMessage::IntentBroadcast { intent, .. } } = ev {
                if from == nb.peer && intent == "secret b->c" {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(got, "C receives B's private message");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_outsider_cannot_join_or_read() {
    let a = NetGene::generate_master("a").unwrap();
    let b = NetGene::generate_master("b").unwrap();
    let outsider = NetGene::generate_master("x").unwrap();
    let members = allowlist_from_genes([&a.0, &b.0]).unwrap();
    // The outsider would accept everyone, but the members refuse it.
    let everyone = allowlist_from_genes([&a.0, &b.0, &outsider.0]).unwrap();

    let pb = free_port();
    let nb = start(b, pb, &members, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut na = start(a, free_port(), &members, Some(pb));
    let mut nx = start(outsider, free_port(), &everyone, Some(pb));

    assert!(await_gossip(&nb.tx, &mut na.rx, nb.peer, "members only").await);
    let leaked = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(ev) = nx.rx.recv().await {
            if matches!(ev, Inbound::Gossip { .. } | Inbound::Private { .. }) {
                return true;
            }
        }
        false
    })
    .await
    .unwrap_or(false);
    assert!(!leaked, "outsider must not receive mesh traffic");
}

async fn await_private(rx: &mut mpsc::Receiver<Inbound>, from: PeerId, text: &str, secs: u64) -> bool {
    tokio::time::timeout(Duration::from_secs(secs), async {
        while let Some(ev) = rx.recv().await {
            if let Inbound::Private { from: f, msg: MeshMessage::IntentBroadcast { intent, .. } } = ev {
                if f == from && intent == text {
                    return true;
                }
            }
        }
        false
    })
    .await
    .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_reach_peer_through_member_relay() {
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).with_test_writer().try_init();
    let r = NetGene::generate_master("relay").unwrap();
    let a = NetGene::generate_master("behind-nat").unwrap();
    let c = NetGene::generate_master("client").unwrap();
    let members = allowlist_from_genes([&r.0, &a.0, &c.0]).unwrap();
    let relay_peer = netgene_p2p::peer_id_for_gene(&r.0).unwrap();
    let pr = free_port();
    let relay_addr: Multiaddr = format!("/ip4/127.0.0.1/tcp/{}/p2p/{}", pr, relay_peer).parse().unwrap();

    let _nr = start_with(r, NodeConfig { relay_server: true, ..config(pr) }, &members, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut na = start_with(a, NodeConfig { relays: vec![relay_addr.clone()], ..config(free_port()) }, &members, None);
    tokio::time::sleep(Duration::from_secs(2)).await; // reservation

    // C only knows A's relayed address.
    let circuit = relay_addr.with(libp2p::multiaddr::Protocol::P2pCircuit).with(libp2p::multiaddr::Protocol::P2p(na.peer));
    let nc = start_with(c, config(free_port()), &members, Some(circuit));
    nc.tx.send(Outbound::Private { to: na.peer, msg: intent("via relay") }).await.unwrap();
    assert!(await_private(&mut na.rx, nc.peer, "via relay", 20).await, "A reached through the relay circuit");
}
