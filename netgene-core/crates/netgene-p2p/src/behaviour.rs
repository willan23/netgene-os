//! libp2p Combined NetworkBehaviour for NetGene OS.

use libp2p::{
    gossipsub, identify, kad, mdns,
    swarm::NetworkBehaviour,
};

#[derive(NetworkBehaviour)]
pub struct NetGeneBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub identify: identify::Behaviour,
}
