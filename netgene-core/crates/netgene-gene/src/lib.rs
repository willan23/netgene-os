//! # NetGene Gene Layer
//!
//! Cryptographic identity and hierarchical key management for NetGene OS.
//!
//! ## Architecture
//! - **Master Gene**: Root identity of the system owner
//! - **Sub-Genes**: Delegated identities for teams/nodes
//! - **Capability tokens**: time-limited, revocable, hybrid-signed delegation
//!
//! ## Security
//! Uses Ed25519 signatures (ring) + SHA3-256 fingerprints. Private keys live in
//! the OS keystore (see [`secret`]). Hybrid post-quantum signatures are planned
//! for Phase 2 (see ROADMAP.md).

pub mod crypto;
pub mod identity;
pub mod token;
pub mod storage;
pub mod secret;
pub mod pq;
pub mod hybrid;
pub mod error;

pub use identity::{NetGene, GeneRole, GeneStatus};
pub use error::GeneError;
pub use token::{CapabilityClaims, CapabilityToken, RevocationList};
pub use secret::{SecretStore, OsKeyring, MemorySecretStore};
pub use hybrid::{HybridPublicKey, HybridSignature, verify_hybrid};

/// NetGene OS version string
pub const NETGENE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Gene Layer protocol version
pub const GENE_PROTOCOL_VERSION: u32 = 1;
