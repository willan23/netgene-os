//! # NetGene QPU Connector (`netgene-qpu`)

pub mod qasm;
pub mod client;

pub use qasm::OpenQasmTranspiler;
pub use client::{QpuClient, QpuProvider, QpuExecutionResult};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openqasm_transpilation() {
        let qasm = OpenQasmTranspiler::transpile_qaoa(4, 2, 0.45, 0.25);
        assert!(qasm.contains("OPENQASM 3.0;"));
        assert!(qasm.contains("qubit[4] q;"));
        assert!(qasm.contains("rzz(0.4500)"));
    }
}
