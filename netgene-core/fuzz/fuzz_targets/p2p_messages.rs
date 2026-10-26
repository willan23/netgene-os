//! Everything a peer can send: PQ handshake requests and responses, private
//! frames, sender-key announcements and encrypted gossip.
#![no_main]

use std::cell::RefCell;
use libfuzzer_sys::fuzz_target;
use libp2p::PeerId;
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::NetGene;
use netgene_p2p::groupcrypt::{EncryptedGossip, GroupCrypto, SenderKeyAnnouncement};
use netgene_p2p::pqsession::{self, HandshakeResp, SealedFrame, Session};
use netgene_p2p::{PqRequest, PqResponse, PrivatePayload};

struct Fixture {
    me: GeneKeyPair,
    peer: PeerId,
    session: Session,
    group: GroupCrypto,
}

fn fixture() -> Fixture {
    let (_, me) = NetGene::generate_master("fuzz-me").unwrap();
    let (peer_gene, peer_kp) = NetGene::generate_master("fuzz-peer").unwrap();
    let peer = netgene_p2p::peer_id_for_gene(&peer_gene).unwrap();
    let me_peer = netgene_p2p::peer_id_for_key(&me.hybrid_public_key()).unwrap();
    let (pending, init) = pqsession::initiate(&peer_kp).unwrap();
    let (session, resp) = pqsession::respond(&me, &init, &peer).unwrap();
    pqsession::complete(pending, &resp, &me_peer).unwrap();
    let group = GroupCrypto::new(&me, chrono::Utc::now()).unwrap();
    Fixture { me, peer, session, group }
}

thread_local! {
    static FIXTURE: RefCell<Fixture> = RefCell::new(fixture());
}

fuzz_target!(|data: &[u8]| {
    FIXTURE.with(|f| {
        let mut f = f.borrow_mut();
        let now = chrono::Utc::now();
        if let Ok(request) = serde_json::from_slice::<PqRequest>(data) {
            match request {
                PqRequest::Init(init) => {
                    let peer = f.peer;
                    let _ = pqsession::respond(&f.me, &init, &peer);
                }
                PqRequest::Data(frame) => {
                    if let Ok(plain) = f.session.open(&frame) {
                        let _ = serde_json::from_slice::<PrivatePayload>(&plain);
                    }
                }
            }
        }
        if let Ok(PqResponse::Accept(resp)) = serde_json::from_slice::<PqResponse>(data) {
            let (pending, _) = pqsession::initiate(&f.me).unwrap();
            let _ = pqsession::complete(pending, &resp, &f.peer);
        }
        let _ = serde_json::from_slice::<HandshakeResp>(data);
        if let Ok(frame) = serde_json::from_slice::<SealedFrame>(data) {
            let _ = f.session.open(&frame);
        }
        if let Ok(ann) = serde_json::from_slice::<SenderKeyAnnouncement>(data) {
            let _ = f.group.accept_announcement(&ann, now);
        }
        if let Ok(msg) = serde_json::from_slice::<EncryptedGossip>(data) {
            let peer = f.peer;
            let _ = f.group.open(&peer, &msg, now);
        }
    });
});
