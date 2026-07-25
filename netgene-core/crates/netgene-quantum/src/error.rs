use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuantumError {
    #[error("Empty problem matrix")]
    EmptyMatrix,
    #[error("Quantum solver failed: {0}")]
    SolverError(String),
    #[error("No feasible solution found")]
    NoSolution,
}
