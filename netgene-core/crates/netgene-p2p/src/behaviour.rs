//! libp2p Combined NetworkBehaviour for NetGene OS.

use libp2p::{
    autonat, dcutr, gossipsub, identify, kad, mdns, relay, request_response,
    swarm::{behaviour::toggle::Toggle, NetworkBehaviour},
};
use serde::{Deserialize, Serialize};

use crate::events::MeshMessage;
use crate::groupcrypt::SenderKeyAnnouncement;
use crate::pqsession::{HandshakeInit, HandshakeResp, SealedFrame};

/// Protocol id of the post-quantum private channel (v2: payloads are [`PrivatePayload`]).
pub const PQ_PROTOCOL: &str = "/netgene/pq/2";

/// What a [`SealedFrame`] decrypts to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrivatePayload {
    Mesh(MeshMessage),
    /// Gossip sender keys (own or forwarded), see [`crate::groupcrypt`].
    SenderKeys(Vec<SenderKeyAnnouncement>),
}

/// Requests on the private channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PqRequest {
    Init(HandshakeInit),
    Data(SealedFrame),
}

/// Responses on the private channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PqResponse {
    Accept(HandshakeResp),
    Ack,
    Reject(String),
}

#[derive(NetworkBehaviour)]
pub struct NetGeneBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: Toggle<mdns::tokio::Behaviour>,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub pq: request_response::json::Behaviour<PqRequest, PqResponse>,
    /// NAT traversal: reservations on relays, hole punching, reachability probes.
    pub relay_client: relay::client::Behaviour,
    pub relay_server: Toggle<relay::Behaviour>,
    pub dcutr: dcutr::Behaviour,
    pub autonat: autonat::Behaviour,
}
