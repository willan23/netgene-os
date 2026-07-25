//! # NetGene Gene Layer
//!
//! Cryptographic identity and hierarchical key management for NetGene OS.
//!
//! ## Architecture
//! - **Master Gene**: Root identity of the system owner
//! - **Sub-Genes**: Delegated identities for teams/nodes
//! - **Gene Tokens**: Signed authorization tokens
//!
//! ## Security
//! Uses Ed25519 signatures (ring) + SHA3-256 fingerprints.
//! Designed to be upgraded to Post-Quantum (ML-KEM/Dilithium) in Phase 2.

pub mod crypto;
pub mod identity;
pub mod token;
pub mod storage;
pub mod error;

pub use identity::{NetGene, GeneRole, GeneStatus};
pub use error::GeneError;
pub use token::GeneToken;

/// NetGene OS version string
pub const NETGENE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Gene Layer protocol version
pub const GENE_PROTOCOL_VERSION: u32 = 1;
