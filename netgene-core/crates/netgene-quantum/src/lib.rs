//! # NetGene Quantum Enhancement Module
//!
//! Quantum-inspired algorithms running on classical hardware.
//! Provides exponential-class optimization for large-scale network problems.
//!
//! ## Algorithms
//! - **QAOA** (Quantum Approximate Optimization Algorithm) — simulated
//! - **Simulated Quantum Annealing** — QUBO problem solver
//! - **Quantum-Inspired Routing** — graph optimization
//!
//! ## Roadmap
//! Phase 2: Connect to AWS Braket / IBM Q via REST API for real quantum hardware.

pub mod qaoa;
pub mod annealing;
pub mod graph;
pub mod cloud;
pub mod error;

pub use qaoa::{QAOAOptimizer, QAOAResult};
pub use annealing::{QuantumAnnealer, AnnealingResult};
pub use graph::{NetworkGraph, NetworkNode, NetworkEdge, RoutingResult};
pub use cloud::*;
pub use error::QuantumError;

/// Quantum module info.
pub fn version_info() -> &'static str {
    "NetGene Quantum v0.1 — Quantum-Inspired Classical Simulation"
}
