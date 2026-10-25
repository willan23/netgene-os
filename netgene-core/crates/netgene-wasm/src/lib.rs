//! # NetGene WASM
//!
//! Sandbox for signed WebAssembly modules (fuel- and memory-bounded, no host imports).

pub mod sandbox;
pub use sandbox::*;
