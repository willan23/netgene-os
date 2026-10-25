//! Swarm manager and P2P Node implementation.

use anyhow::Result;
use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, mdns, noise, relay, request_response,
    multiaddr::Protocol,
    swarm::{Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;
use tokio::sync::mpsc;
use libp2p::futures::StreamExt;
use tracing::{info, warn};
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::HybridPublicKey;

use crate::behaviour::{NetGeneBehaviour, NetGeneBehaviourEvent, PqRequest, PqResponse, PrivatePayload, PQ_PROTOCOL};
use crate::events::MeshMessage;
use crate::groupcrypt::{EncryptedGossip, GroupCrypto, SenderKeyAnnouncement};
use crate::identity::{libp2p_keypair_from_gene, peer_id_for_key};
use crate::pqsession::{self, PendingInitiator, Session};

/// v2: payloads are [`EncryptedGossip`] (sender keys over PQ sessions).
pub const GOSSIPSUB_TOPIC: &str = "netgene-mesh-v2";
/// Gossip kept per author while its sender key has not arrived yet.
const MAX_PENDING_GOSSIP: usize = 64;

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

/// How a node listens and reaches peers behind NAT.
#[derive(Debug, Clone)]
pub struct NodeConfig {
    /// TCP and QUIC (UDP) port.
    pub listen_port: u16,
    /// Discover peers on the local network with mDNS.
    pub mdns: bool,
    /// Act as a circuit relay for members (a node with a public address).
    pub relay_server: bool,
    /// Relays to reserve a slot on (`/ip4/…/tcp/7777/p2p/<relay PeerId>`), so
    /// members can reach this node through them; a direct connection is then
    /// attempted by hole punching (DCUtR).
    pub relays: Vec<Multiaddr>,
    /// Publicly reachable addresses of this node. A relay only serves once it
    /// has one; without any, a relay announces its listen addresses.
    pub external_addrs: Vec<Multiaddr>,
}

impl NodeConfig {
    pub fn new(listen_port: u16) -> Self {
        Self { listen_port, mdns: true, relay_server: false, relays: vec![], external_addrs: vec![] }
    }
}

/// Relay limits for members (libp2p's defaults, 2 min / 128 KiB, suit public relays).
fn member_relay_config() -> relay::Config {
    relay::Config {
        max_circuit_duration: Duration::from_secs(60 * 60),
        max_circuit_bytes: 1 << 30,
        ..Default::default()
    }
}

/// What the application asks the node to send.
#[derive(Debug, Clone)]
pub enum Outbound {
    /// Gossip to every member: signed by the author and encrypted with its
    /// sender key, which members only receive over post-quantum sessions.
    Broadcast(MeshMessage),
    /// End-to-end message over a hybrid X25519 + ML-KEM-768 session.
    Private { to: PeerId, msg: MeshMessage },
}

/// What the node reports to the application.
#[derive(Debug, Clone)]
pub enum Inbound {
    Gossip { from: Option<PeerId>, msg: MeshMessage },
    Private { from: PeerId, msg: MeshMessage },
    SessionEstablished { peer: PeerId },
}

pub struct NetGeneP2PNode {
    pub peer_id: PeerId,
    swarm: Swarm<NetGeneBehaviour>,
    policy: PeerPolicy,
    gene_kp: GeneKeyPair,
    /// Expected Gene key per peer; a handshake presenting another key is refused.
    pinned_keys: HashMap<PeerId, HybridPublicKey>,
    sessions: HashMap<PeerId, Session>,
    pending: HashMap<PeerId, PendingInitiator>,
    outbox: HashMap<PeerId, Vec<PrivatePayload>>,
    group: GroupCrypto,
    pending_gossip: HashMap<PeerId, VecDeque<EncryptedGossip>>,
    /// Relays to reserve a slot on, by PeerId; removed once the reservation is requested.
    relays: HashMap<PeerId, Multiaddr>,
    /// Relay without configured external addresses: announce listen addresses instead.
    announce_listen_addrs: bool,
    inbound_tx: mpsc::Sender<Inbound>,
    outbound_rx: mpsc::Receiver<Outbound>,
}

impl NetGeneP2PNode {
    /// Create a node running as the Gene owning `gene_kp`; its libp2p
    /// identity is derived from the Gene key.
    pub fn new(
        listen_port: u16,
        gene_kp: GeneKeyPair,
        policy: PeerPolicy,
    ) -> Result<(Self, mpsc::Sender<Outbound>, mpsc::Receiver<Inbound>)> {
        Self::with_config(NodeConfig::new(listen_port), gene_kp, policy)
    }

    pub fn with_config(
        config: NodeConfig,
        gene_kp: GeneKeyPair,
        policy: PeerPolicy,
    ) -> Result<(Self, mpsc::Sender<Outbound>, mpsc::Receiver<Inbound>)> {
        let local_key = libp2p_keypair_from_gene(&gene_kp)?;
        let peer_id = PeerId::from(local_key.public());
        info!("🌐 Initializing P2P node with PeerId: {}", peer_id);
        match &policy {
            PeerPolicy::Allowlist(peers) => info!("🔒 Peer allowlist active ({} peers)", peers.len()),
            PeerPolicy::Open => warn!("⚠️ Open peer policy: any peer on the network can join"),
        }

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

        let mdns = if config.mdns { Some(mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?) } else { None };
        let relay_server = config.relay_server.then(|| relay::Behaviour::new(peer_id, member_relay_config()));

        // TCP + QUIC, Noise + Yamux, plus the relay client transport (`/p2p-circuit`).
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key.clone())
            .with_tokio()
            .with_tcp(tcp::Config::default().nodelay(true), noise::Config::new, yamux::Config::default)?
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default)?
            .with_behaviour(|key, relay_client| NetGeneBehaviour {
                gossipsub,
                mdns: mdns.into(),
                kademlia: kad::Behaviour::new(peer_id, kad::store::MemoryStore::new(peer_id)),
                identify: identify::Behaviour::new(identify::Config::new(
                    format!("/netgene/{}", env!("CARGO_PKG_VERSION")),
                    key.public(),
                )),
                pq: request_response::json::Behaviour::new(
                    [(StreamProtocol::new(PQ_PROTOCOL), request_response::ProtocolSupport::Full)],
                    request_response::Config::default(),
                ),
                relay_client,
                relay_server: relay_server.into(),
                dcutr: dcutr::Behaviour::new(peer_id),
                autonat: autonat::Behaviour::new(peer_id, autonat::Config::default()),
            })
            .map_err(|e| anyhow::anyhow!("Behaviour init error: {}", e))?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", config.listen_port).parse()?)?;
        swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", config.listen_port).parse()?)?;
        if config.relay_server {
            info!("🛰️  Relay server enabled (members only)");
        }
        let mut relays = HashMap::new();
        for addr in &config.external_addrs {
            swarm.add_external_address(addr.clone());
        }
        let announce_listen_addrs = config.relay_server && config.external_addrs.is_empty();
        for relay_addr in &config.relays {
            let relay_peer = relay_addr.iter().find_map(|p| match p {
                Protocol::P2p(id) => Some(id),
                _ => None,
            });
            let Some(relay_peer) = relay_peer else {
                anyhow::bail!("Relay address {} must end with /p2p/<relay PeerId>", relay_addr);
            };
            if !policy.allows(&relay_peer) {
                anyhow::bail!("Relay {} is not a trusted member", relay_peer);
            }
            // The reservation is requested once the relay is connected (see `run`).
            swarm.dial(relay_addr.clone())?;
            relays.insert(relay_peer, relay_addr.clone());
        }

        let (out_tx, out_rx) = mpsc::channel(100);
        let (in_tx, in_rx) = mpsc::channel(100);

        let group = GroupCrypto::new(&gene_kp, chrono::Utc::now())?;
        let node = Self {
            peer_id,
            swarm,
            policy,
            group,
            pending_gossip: HashMap::new(),
            relays,
            announce_listen_addrs,
            gene_kp,
            pinned_keys: HashMap::new(),
            sessions: HashMap::new(),
            pending: HashMap::new(),
            outbox: HashMap::new(),
            inbound_tx: in_tx,
            outbound_rx: out_rx,
        };

        Ok((node, out_tx, in_rx))
    }

    /// Pin the expected Gene keys of known peers (both halves are compared).
    pub fn with_pinned_keys(mut self, keys: impl IntoIterator<Item = HybridPublicKey>) -> Result<Self> {
        for key in keys {
            self.pinned_keys.insert(peer_id_for_key(&key)?, key);
        }
        Ok(self)
    }

    /// Dial a remote peer multiaddr
    pub fn dial(&mut self, addr: Multiaddr) -> Result<()> {
        info!("🔗 Dialing P2P peer at {}", addr);
        self.swarm.dial(addr)?;
        Ok(())
    }

    fn check_pinned(&self, peer: &PeerId, key: &HybridPublicKey) -> Result<()> {
        match self.pinned_keys.get(peer) {
            Some(pinned) if pinned != key => anyhow::bail!("Peer {} presented a Gene key that differs from the pinned one", peer),
            _ => Ok(()),
        }
    }

    fn start_handshake(&mut self, peer: PeerId) {
        if self.sessions.contains_key(&peer) || self.pending.contains_key(&peer) {
            return;
        }
        match pqsession::initiate(&self.gene_kp) {
            Ok((pending, init)) => {
                self.pending.insert(peer, pending);
                self.swarm.behaviour_mut().pq.send_request(&peer, PqRequest::Init(init));
            }
            Err(e) => warn!("Cannot start PQ handshake with {}: {}", peer, e),
        }
    }

    async fn session_ready(&mut self, peer: PeerId) {
        info!("🔐 Post-quantum session established with {} (X25519 + ML-KEM-768)", peer);
        let _ = self.inbound_tx.send(Inbound::SessionEstablished { peer }).await;
        // The peer gets every sender key we know, so it can read the mesh's gossip.
        let keys = self.group.all_announcements(chrono::Utc::now());
        self.send_private(peer, PrivatePayload::SenderKeys(keys));
        for payload in self.outbox.remove(&peer).unwrap_or_default() {
            self.send_private(peer, payload);
        }
    }

    fn send_private(&mut self, peer: PeerId, payload: PrivatePayload) {
        let Some(session) = self.sessions.get_mut(&peer) else {
            self.outbox.entry(peer).or_default().push(payload);
            self.start_handshake(peer);
            return;
        };
        match serde_json::to_vec(&payload).map_err(anyhow::Error::from).and_then(|bytes| session.seal(&bytes)) {
            Ok(frame) => {
                self.swarm.behaviour_mut().pq.send_request(&peer, PqRequest::Data(frame));
            }
            Err(e) => warn!("Cannot encrypt message for {}: {}", peer, e),
        }
    }

    /// Send sender keys to every session except `except`.
    fn forward_keys(&mut self, keys: Vec<SenderKeyAnnouncement>, except: &[PeerId]) {
        if keys.is_empty() {
            return;
        }
        let peers: Vec<PeerId> = self.sessions.keys().filter(|p| !except.contains(p)).copied().collect();
        for peer in peers {
            self.send_private(peer, PrivatePayload::SenderKeys(keys.clone()));
        }
    }

    /// Store new sender keys from `from`, forward them, and release buffered gossip they unlock.
    fn accept_sender_keys(&mut self, from: PeerId, keys: Vec<SenderKeyAnnouncement>) -> Vec<Inbound> {
        let now = chrono::Utc::now();
        let mut fresh = Vec::new();
        for ann in keys {
            let Ok(author) = crate::identity::peer_id_for_key(&ann.author_key) else { continue };
            if !self.policy.allows(&author) {
                warn!("🚫 Ignoring sender key of non-member {} (via {})", author, from);
                continue;
            }
            match self.group.accept_announcement(&ann, now) {
                Ok(true) => fresh.push((author, ann)),
                Ok(false) => {}
                Err(e) => warn!("🚫 Sender key from {} refused: {}", from, e),
            }
        }
        let mut delivered = Vec::new();
        for (author, _) in &fresh {
            let queued: Vec<EncryptedGossip> = self.pending_gossip.remove(author).map(Vec::from).unwrap_or_default();
            let mut still_waiting = VecDeque::new();
            for msg in queued {
                if self.group.has_key(author, msg.key_id) {
                    delivered.extend(self.open_gossip(*author, &msg));
                } else {
                    still_waiting.push_back(msg);
                }
            }
            if !still_waiting.is_empty() {
                self.pending_gossip.insert(*author, still_waiting);
            }
        }
        let authors: Vec<PeerId> = fresh.iter().map(|(a, _)| *a).collect();
        let mut except = authors;
        except.push(from);
        self.forward_keys(fresh.into_iter().map(|(_, ann)| ann).collect(), &except);
        delivered
    }

    fn open_gossip(&mut self, author: PeerId, msg: &EncryptedGossip) -> Option<Inbound> {
        let opened = self
            .group
            .open(&author, msg, chrono::Utc::now())
            .and_then(|bytes| Ok(serde_json::from_slice::<MeshMessage>(&bytes)?));
        match opened {
            Ok(msg) => Some(Inbound::Gossip { from: Some(author), msg }),
            Err(e) => {
                warn!("🚫 Gossip from {} dropped: {}", author, e);
                None
            }
        }
    }

    fn broadcast(&mut self, topic: &gossipsub::IdentTopic, msg: &MeshMessage) -> Result<()> {
        if let Some(ann) = self.group.rotate_if_due(&self.gene_kp, chrono::Utc::now())? {
            info!("🔄 Rotated gossip sender key ({})", ann.key_id);
            self.forward_keys(vec![ann], &[]);
        }
        let sealed = self.group.seal(&serde_json::to_vec(msg)?)?;
        self.swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), serde_json::to_vec(&sealed)?)
            .map_err(|e| anyhow::anyhow!("Gossipsub publish error: {}", e))?;
        Ok(())
    }

    fn handle_request(&mut self, peer: PeerId, request: PqRequest) -> (PqResponse, Vec<Inbound>, bool) {
        if !self.policy.allows(&peer) {
            return (PqResponse::Reject("peer not allowed".into()), vec![], false);
        }
        match request {
            PqRequest::Init(init) => {
                // Simultaneous handshakes: the lower PeerId's Init wins.
                if self.pending.contains_key(&peer) && self.peer_id < peer {
                    return (PqResponse::Reject("handshake already in progress".into()), vec![], false);
                }
                if let Err(e) = self.check_pinned(&peer, &init.initiator_key) {
                    return (PqResponse::Reject(e.to_string()), vec![], false);
                }
                match pqsession::respond(&self.gene_kp, &init, &peer) {
                    Ok((session, resp)) => {
                        self.pending.remove(&peer);
                        self.sessions.insert(peer, session);
                        (PqResponse::Accept(resp), vec![], true)
                    }
                    Err(e) => (PqResponse::Reject(e.to_string()), vec![], false),
                }
            }
            PqRequest::Data(frame) => {
                let Some(session) = self.sessions.get_mut(&peer) else {
                    return (PqResponse::Reject("no session".into()), vec![], false);
                };
                let opened = session.open(&frame).and_then(|bytes| Ok(serde_json::from_slice::<PrivatePayload>(&bytes)?));
                match opened {
                    Ok(PrivatePayload::Mesh(msg)) => (PqResponse::Ack, vec![Inbound::Private { from: peer, msg }], false),
                    Ok(PrivatePayload::SenderKeys(keys)) => (PqResponse::Ack, self.accept_sender_keys(peer, keys), false),
                    Err(e) => (PqResponse::Reject(e.to_string()), vec![], false),
                }
            }
        }
    }

    /// Runs the P2P event loop in background task
    pub async fn run(mut self) {
        let topic = gossipsub::IdentTopic::new(GOSSIPSUB_TOPIC);

        loop {
            tokio::select! {
                Some(outbound) = self.outbound_rx.recv() => match outbound {
                    Outbound::Broadcast(msg) => {
                        if let Err(e) = self.broadcast(&topic, &msg) {
                            warn!("Broadcast failed: {}", e);
                        }
                    }
                    Outbound::Private { to, msg } => self.send_private(to, PrivatePayload::Mesh(msg)),
                },

                event = self.swarm.select_next_some() => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        info!("📡 Listening on P2P multiaddr: {}", address);
                        let unspecified = address.iter().any(|p| matches!(p, Protocol::Ip4(ip) if ip.is_unspecified()));
                        if self.announce_listen_addrs && !unspecified && !address.iter().any(|p| p == Protocol::P2pCircuit) {
                            self.swarm.add_external_address(address);
                        }
                    }
                    SwarmEvent::ListenerError { error, .. } => warn!("Listener error: {}", error),
                    SwarmEvent::ListenerClosed { addresses, reason: Err(e), .. } => {
                        warn!("Listener on {:?} closed: {}", addresses, e);
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        warn!("Dial to {:?} failed: {}", peer_id, error);
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                        if let Some(relay_addr) = self.relays.remove(&peer_id) {
                            match self.swarm.listen_on(relay_addr.clone().with(Protocol::P2pCircuit)) {
                                Ok(_) => info!("🛰️  Requesting a relay slot on {}", relay_addr),
                                Err(e) => warn!("Cannot listen through relay {}: {}", relay_addr, e),
                            }
                        }
                        if !self.policy.allows(&peer_id) {
                            warn!("🚫 Rejecting connection from peer {} (not in allowlist)", peer_id);
                            let _ = self.swarm.disconnect_peer_id(peer_id);
                        } else if self.peer_id < peer_id || self.outbox.contains_key(&peer_id) {
                            // Deterministic initiator avoids duplicate handshakes; a node
                            // with queued private messages starts one itself (ties are
                            // resolved in handle_request).
                            self.start_handshake(peer_id);
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, num_established: 0, .. } => {
                        self.sessions.remove(&peer_id);
                        self.pending.remove(&peer_id);
                        self.outbox.remove(&peer_id);
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
                        // Relaying only needs authenticity (signed by a member, well-formed);
                        // reading needs the author's sender key.
                        let author = message.source.filter(|s| self.policy.allows(s));
                        let parsed = serde_json::from_slice::<EncryptedGossip>(&message.data);
                        let accept = self.policy.allows(&propagation_source) && author.is_some() && parsed.is_ok();

                        let verdict = if accept { gossipsub::MessageAcceptance::Accept } else { gossipsub::MessageAcceptance::Reject };
                        let _ = self.swarm.behaviour_mut().gossipsub
                            .report_message_validation_result(&message_id, &propagation_source, verdict);

                        match (author, parsed) {
                            (Some(author), Ok(sealed)) if accept => {
                                if self.group.has_key(&author, sealed.key_id) {
                                    if let Some(inbound) = self.open_gossip(author, &sealed) {
                                        let _ = self.inbound_tx.send(inbound).await;
                                    }
                                } else {
                                    let queue = self.pending_gossip.entry(author).or_default();
                                    if queue.len() >= MAX_PENDING_GOSSIP {
                                        queue.pop_front();
                                    }
                                    queue.push_back(sealed);
                                }
                            }
                            _ => warn!("🚫 Rejected mesh message from {}", propagation_source),
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Pq(request_response::Event::Message { peer, message, .. })) => match message {
                        request_response::Message::Request { request, channel, .. } => {
                            let (response, inbound, established) = self.handle_request(peer, request);
                            if let PqResponse::Reject(reason) = &response {
                                warn!("🚫 PQ request from {} rejected: {}", peer, reason);
                            }
                            let _ = self.swarm.behaviour_mut().pq.send_response(channel, response);
                            for inbound in inbound {
                                let _ = self.inbound_tx.send(inbound).await;
                            }
                            if established {
                                self.session_ready(peer).await;
                            }
                        }
                        request_response::Message::Response { response, .. } => match response {
                            PqResponse::Accept(resp) => {
                                let Some(pending) = self.pending.remove(&peer) else { continue };
                                let result = self
                                    .check_pinned(&peer, &resp.responder_key)
                                    .and_then(|_| pqsession::complete(pending, &resp, &peer));
                                match result {
                                    Ok(session) => {
                                        self.sessions.insert(peer, session);
                                        self.session_ready(peer).await;
                                    }
                                    Err(e) => warn!("🚫 PQ handshake with {} failed: {}", peer, e),
                                }
                            }
                            PqResponse::Ack => {}
                            PqResponse::Reject(reason) => {
                                self.pending.remove(&peer);
                                warn!("🚫 Peer {} rejected PQ request: {}", peer, reason);
                            }
                        },
                    },
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Pq(request_response::Event::OutboundFailure { peer, error, .. })) => {
                        // E.g. not connected yet: forget the attempt so the next connection retries.
                        if self.pending.remove(&peer).is_some() {
                            warn!("PQ handshake with {} could not be sent ({}); retrying on next connection", peer, error);
                        } else {
                            warn!("PQ message to {} failed: {}", peer, error);
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
                        info!("🔑 Identified peer {} with agent protocol {}", peer_id, info.protocol_version);
                        if self.policy.allows(&peer_id) {
                            for addr in info.listen_addrs {
                                self.swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::RelayClient(relay::client::Event::ReservationReqAccepted { relay_peer_id, .. })) => {
                        info!("🛰️  Reachable through relay {}", relay_peer_id);
                    }
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Dcutr(dcutr::Event { remote_peer_id, result })) => match result {
                        Ok(_) => info!("🕳️  Hole punch to {} succeeded: direct connection", remote_peer_id),
                        Err(e) => warn!("Hole punch to {} failed, staying on the relay: {}", remote_peer_id, e),
                    },
                    SwarmEvent::Behaviour(NetGeneBehaviourEvent::Autonat(autonat::Event::StatusChanged { new, .. })) => {
                        info!("🌍 NAT status: {:?}", new);
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
