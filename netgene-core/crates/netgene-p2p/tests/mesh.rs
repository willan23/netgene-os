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
use netgene_p2p::{allowlist_from_genes, Inbound, MeshMessage, NetGeneP2PNode, Outbound, PeerPolicy};
use tokio::sync::mpsc;

fn port(offset: u16) -> u16 {
    40000 + (std::process::id() % 2000) as u16 * 10 + offset
}

fn intent(text: &str) -> MeshMessage {
    MeshMessage::IntentBroadcast { sender: "test".into(), intent: text.into(), timestamp: Utc::now() }
}

struct Started {
    peer: PeerId,
    tx: mpsc::Sender<Outbound>,
    rx: mpsc::Receiver<Inbound>,
}

fn start(gene: (NetGene, netgene_gene::crypto::GeneKeyPair), port: u16, members: &HashSet<PeerId>, dial: Option<u16>) -> Started {
    let (node, tx, rx) = NetGeneP2PNode::new(port, gene.1, PeerPolicy::Allowlist(members.clone())).unwrap();
    let mut node = node;
    if let Some(p) = dial {
        node.dial(format!("/ip4/127.0.0.1/tcp/{}", p).parse::<Multiaddr>().unwrap()).unwrap();
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

    let mut nb = start(b, port(1), &members, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut na = start(a, port(0), &members, Some(port(1)));
    let mut nc = start(c, port(2), &members, Some(port(1)));

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

    let nb = start(b, port(5), &members, None);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let mut na = start(a, port(4), &members, Some(port(5)));
    let mut nx = start(outsider, port(6), &everyone, Some(port(5)));

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
