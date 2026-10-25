//! Binding between Gene identities and libp2p peer identities.
//!
//! A node's libp2p keypair is derived from its Gene's Ed25519 key, so its
//! PeerId can be computed from the Gene's public key alone. That lets an
//! allowlist be built from Gene records (public JSON), without exchanging
//! extra secrets.

use std::collections::HashSet;
use anyhow::{Context, Result};
use libp2p::{identity, PeerId};
use netgene_gene::crypto::GeneKeyPair;
use netgene_gene::NetGene;

/// libp2p keypair derived from a Gene's Ed25519 key.
pub fn libp2p_keypair_from_gene(kp: &GeneKeyPair) -> Result<identity::Keypair> {
    let mut seed = kp.ed25519_seed()?;
    let keypair = identity::Keypair::ed25519_from_bytes(seed.as_mut())
        .context("Gene key is not a valid Ed25519 secret")?;
    if keypair.public().try_into_ed25519().map(|pk| pk.to_bytes().to_vec()).ok().as_deref()
        != Some(kp.public_key_bytes())
    {
        anyhow::bail!("Derived libp2p key does not match the Gene public key");
    }
    Ok(keypair)
}

/// PeerId a node running as `gene` will have.
pub fn peer_id_for_gene(gene: &NetGene) -> Result<PeerId> {
    let pk = identity::ed25519::PublicKey::try_from_bytes(&gene.public_key_bytes()?)
        .context("Gene public key is not a valid Ed25519 key")?;
    Ok(PeerId::from_public_key(&identity::PublicKey::from(pk)))
}

/// PeerIds of the given genes.
pub fn allowlist_from_genes<'a>(genes: impl IntoIterator<Item = &'a NetGene>) -> Result<HashSet<PeerId>> {
    genes.into_iter().map(peer_id_for_gene).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_id_matches_derived_keypair() {
        let (gene, kp) = NetGene::generate_master("node").unwrap();
        let keypair = libp2p_keypair_from_gene(&kp).unwrap();
        assert_eq!(PeerId::from(keypair.public()), peer_id_for_gene(&gene).unwrap());
    }

    #[test]
    fn test_distinct_genes_give_distinct_peers() {
        let (a, _) = NetGene::generate_master("a").unwrap();
        let (b, _) = NetGene::generate_master("b").unwrap();
        let allow = allowlist_from_genes([&a, &b]).unwrap();
        assert_eq!(allow.len(), 2);
    }
}
