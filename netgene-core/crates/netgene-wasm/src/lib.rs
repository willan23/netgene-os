//! # NetGene WASM
//!
//! Sandbox for signed WebAssembly agents — core modules or Component Model
//! components — bounded by fuel and memory, with permission-gated host imports.

pub mod sandbox;
pub mod component;
pub use sandbox::*;
