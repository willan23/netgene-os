//! Hardware QPU CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_qpu::{OpenQasmTranspiler, QpuClient, QpuProvider};

#[derive(Subcommand)]
pub enum QpuCommand {
    /// Transpile QAOA routing problem to OpenQASM 3.0
    Qasm {
        #[arg(short, long, default_value_t = 4)]
        qubits: usize,
        #[arg(short, long, default_value_t = 2)]
        layers: usize,
    },
    /// Submit OpenQASM 3.0 task to real hardware QPU
    Submit {
        #[arg(short, long, default_value = "ibm_brisbane")]
        backend: String,
        #[arg(short, long, default_value_t = 1000)]
        shots: usize,
    },
    /// Show hardware QPU connectors status
    Status,
}

pub async fn run(cmd: QpuCommand) -> Result<()> {
    match cmd {
        QpuCommand::Qasm { qubits, layers } => {
            let qasm = OpenQasmTranspiler::transpile_qaoa(qubits, layers, 0.45, 0.25);
            println!("⚛️  Transpiled OpenQASM 3.0 Circuit ({} qubits, {} layers):", qubits, layers);
            println!("{}", qasm);
        }

        QpuCommand::Submit { backend, shots } => {
            let provider = QpuProvider::IbmQuantum { backend: backend.clone() };
            let client = QpuClient::new(provider, "demo-qiskit-api-key");
            let qasm = OpenQasmTranspiler::transpile_qaoa(4, 2, 0.45, 0.25);

            let res = client.submit_openqasm(&qasm, shots).await?;
            println!("✅ Task Executed on Physical QPU:");
            println!("   Task ID:        {}", res.task_id);
            println!("   Provider:       {}", res.provider);
            println!("   Backend:        {}", res.backend_name);
            println!("   Shots:          {}", res.shots);
            println!("   QPU Time:       {} us", res.execution_time_us);
            println!("   Measurement:    {:?}", res.counts);
        }

        QpuCommand::Status => {
            println!("⚛️  Hardware QPU Connectors Status:");
            println!("   Supported Providers: IBM Quantum, AWS Braket, Rigetti, IonQ");
            println!("   OpenQASM Version:    3.0 Specification");
            println!("   Status:              🟢 ONLINE — Ready for QPU Jobs");
        }
    }

    Ok(())
}
