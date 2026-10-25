//! Swarm manager and P2P Node implementation.

use anyhow::Result;
use libp2p::{
    core::upgrade,
    gossipsub, identify, identity, kad, mdns, noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use libp2p::futures::StreamExt;
use tracing::{info, warn};

use crate::behaviour::{NetGeneBehaviour, NetGeneBehaviourEvent};
use crate::events::MeshMessage;

pub const GOSSIPSUB_TOPIC: &str = "netgene-mesh-v1";

/// Which peers this node talks to.
#[derive(Debug, Clone)]
pub enum PeerPolicy {
    /// Only these peers may connect or relay messages.
    Allowlist(HashSet<PeerId>),
    /// Any peer (development only).
    Open,
}

impl PeerPolicy {
    pub fn allows(&self, peer: &PeerId) -> bool {
        match self {
            PeerPolicy::Allowlist(peers) => peers.contains(peer),
            PeerPolicy::Open => true,
        }
    }
}

pub struct NetGeneP2PNode {
    pub peer_id: PeerId,
    swarm: Swarm<NetGeneBehaviour>,
    policy: PeerPolicy,
    msg_tx: mpsc::Sender<MeshMessage>,
    msg_rx: mpsc::Receiver<MeshMessage>,
}

impl NetGeneP2PNode {
    /// Create a node with the given libp2p identity (normally derived from a
    /// Gene, see [`crate::identity::libp2p_keypair_from_gene`]).
    pub fn new(
        listen_port: u16,
        local_key: identity::Keypair,
        policy: PeerPolicy,
    ) -> Result<(Self, mpsc::Sender<MeshMessage>, mpsc::Receiver<MeshMessage>)> {
        let peer_id = PeerId::from(local_key.public());
        info!("🌐 Initializing P2P node with PeerId: {}", peer_id);
        match &policy {
            PeerPolicy::Allowlist(peers) => info!("🔒 Peer allowlist active ({} peers)", peers.len()),
            PeerPolicy::Open => warn!("⚠️ Open peer policy: any peer on the network can join"),
        }

        // Transport setup: TCP + Noise + Yamux
        let transport = tcp::tokio::Transport::default()
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Gossipsub: signed messages, strict validation, application-level acceptance.
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(gossipsub::ValidationMode::Strict)
            .validate_messages()
            .build()
            .map_err(|e| anyhow::anyhow!("Gossipsub config error: {}", e))?;

        let mut gossipsub = gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key.clone()),
            gossipsub_config,
        ).map_err(|e| anyhow::anyhow!("Gossipsub init error: {}", e))?;

        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);
        gossipsub.subscribe(&topic)?;

        // mDNS setup
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;

        // Kademlia DHT setup
        let store = kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, store);

        // Identify setup
        let identify = identify::Behaviour::new(identify::Config::new(
            format!("/netgene/{}", env!("CARGO_PKG_VERSION")),
            local_key.public(),
        ));

        let behaviour = NetGeneBehaviour {
            gossipsub,
            mdns,
            kademlia,
            identify,
        };

        let mut swarm = Swarm::new(
            transport,
            behaviour,
            peer_id,
            libp2p::swarm::Config::with_tokio_executor(),
        );

        let listen_addr: Multiaddr = format!("/ip4/0.0.0.0/tcp/{}", listen_port).parse()?;
        swarm.listen_on(listen_addr)?;

        // Channels for outbound/inbound mesh messages
        let (out_tx, out_rx) = mpsc::channel(100);
        let (in_tx, in_rx) = mpsc::channel(100);

        let node = Self {
            peer_id,
            swarm,
            policy,
            msg_tx: in_tx,
            msg_rx: out_rx,
        };

        Ok((node, out_tx, in_rx))
    }

    /// Dial a remote peer multiaddr
    pub fn dial(&mut self, addr: Multiaddr) -> Result<()> {
        info!("🔗 Dialing P2P peer at {}", addr);
        self.swarm.dial(addr)?;
        Ok(())
    }

    /// Runs the P2P event loop in background task
    pub async fn run(mut self) {
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);

        loop {
            tokio::select! {
                // Outbound message to broadcast on Gossipsub mesh
                Some(msg) = self.msg_rx.recv() => {
                    if let Ok(bytes) = serde_json::to_vec(&msg) {
                        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), bytes) {
                            warn!("Gossipsub publish error: {}", e);
                        }
                    }
                }

                // Swarm network events
                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("📡 Listening on P2P multiaddr: {}", address);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        if !self.policy.allows(&peer_id) {
                            warn!("🚫 Rejecting connection from peer {} (not in allowlist)", peer_id);
                            let _ = self.swarm.disconnect_peer_id(peer_id);
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer, addr) in list {
                            if !self.policy.allows(&peer) {
                                continue;
                            }
                            info!("🔎 mDNS discovered peer {} at {}", peer, addr);
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                            self.swarm.behaviour_mut().kademlia.add_address(&peer, addr);
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source, message_id, message,
                    })) => {
                        let author_allowed = message.source.as_ref().is_some_and(|s| self.policy.allows(s));
                        let parsed = serde_json::from_slice::<MeshMessage>(&message.data);
                        let accept = self.policy.allows(&propagation_source) && author_allowed && parsed.is_ok();

                        let verdict = if accept { gossipsub::MessageAcceptance::Accept } else { gossipsub::MessageAcceptance::Reject };
                        let _ = self.swarm.behaviour_mut().gossipsub
                            .report_message_validation_result(&message_id, &propagation_source, verdict);

                        match parsed {
                            Ok(msg) if accept => { let _ = self.msg_tx.send(msg).await; }
                            _ => warn!("🚫 Rejected mesh message from {}", propagation_source),
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
                        info!("🔑 Identified peer {} with agent protocol {}", peer_id, info.protocol_version);
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_policy() {
        let allowed = PeerId::random();
        let stranger = PeerId::random();
        let policy = PeerPolicy::Allowlist(HashSet::from([allowed]));
        assert!(policy.allows(&allowed));
        assert!(!policy.allows(&stranger));
        assert!(PeerPolicy::Open.allows(&stranger));
    }
}
