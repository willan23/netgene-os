//! libp2p Combined NetworkBehaviour for NetGene OS.

use libp2p::{
    gossipsub, identify, kad, mdns, request_response,
    swarm::NetworkBehaviour,
};
use serde::{Deserialize, Serialize};

use crate::pqsession::{HandshakeInit, HandshakeResp, SealedFrame};

/// Protocol id of the post-quantum private channel.
pub const PQ_PROTOCOL: &str = "/netgene/pq/1";

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
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
    pub pq: request_response::json::Behaviour<PqRequest, PqResponse>,
}
