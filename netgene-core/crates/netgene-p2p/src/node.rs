//! Swarm manager and P2P Node implementation.

use anyhow::Result;
use libp2p::{
    core::upgrade,
    gossipsub, identify, identity, kad, mdns, noise,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use std::time::Duration;
use tokio::sync::mpsc;
use libp2p::futures::StreamExt;
use tracing::{info, warn};

use crate::behaviour::{NetGeneBehaviour, NetGeneBehaviourEvent};
use crate::events::MeshMessage;

pub const GOSSIPSUB_TOPIC: &str = "netgene-mesh-v1";

pub struct NetGeneP2PNode {
    pub peer_id: PeerId,
    swarm: Swarm<NetGeneBehaviour>,
    msg_tx: mpsc::Sender<MeshMessage>,
    msg_rx: mpsc::Receiver<MeshMessage>,
}

impl NetGeneP2PNode {
    pub fn new(listen_port: u16) -> Result<(Self, mpsc::Sender<MeshMessage>, mpsc::Receiver<MeshMessage>)> {
        // Generate identity keypair for libp2p peer ID
        let local_key = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(local_key.public());
        info!("🌐 Initializing P2P node with PeerId: {}", peer_id);

        // Transport setup: TCP + Noise + Yamux
        let transport = tcp::tokio::Transport::default()
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::Config::new(&local_key)?)
            .multiplex(yamux::Config::default())
            .boxed();

        // Gossipsub setup
        let gossipsub_config = gossipsub::ConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(1))
            .validation_mode(gossipsub::ValidationMode::Permissive)
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
            "/netgene/0.2.0".to_string(),
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
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                        for (peer, addr) in list {
                            info!("🔎 mDNS discovered peer {} at {}", peer, addr);
                            self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
                            self.swarm.behaviour_mut().kademlia.add_address(&peer, addr);
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Gossipsub(gossipsub::Event::Message { message, .. })) => {
                        if let Ok(msg) = serde_json::from_slice::<MeshMessage>(&message.data) {
                            let _ = self.msg_tx.send(msg).await;
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
