//! WASM CLI commands.

use clap::Subcommand;
use anyhow::{Context, Result};
use std::path::PathBuf;
use netgene_gene::{
    secret::OsKeyring,
    storage::{default_gene_dir, list_genes, load_gene},
    HybridPublicKey,
};
use netgene_wasm::{GeneModule, WasmSandbox, DEFAULT_FUEL};

const MEMORY_LIMIT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Subcommand)]
pub enum WasmCommand {
    /// Sign a .wasm binary with a local gene, producing a Gene Module (JSON)
    Sign {
        /// Path to the .wasm binary
        file: PathBuf,
        /// ID of the signing gene
        #[arg(long)]
        gene: String,
        /// Module name
        #[arg(short, long)]
        name: String,
        /// Module version
        #[arg(long, default_value = "0.1.0")]
        version: String,
        /// Output path (default: <file>.gene-module.json)
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Verify and execute a signed Gene Module
    Run {
        /// Path to the Gene Module JSON
        module: PathBuf,
        /// Trusted author gene IDs (default: every gene in the local store)
        #[arg(long = "trust")]
        trust: Vec<String>,
        /// CPU budget (fuel units)
        #[arg(long, default_value_t = DEFAULT_FUEL)]
        fuel: u64,
    },
    /// Show WASM sandbox limits
    Status,
}

pub async fn run(cmd: WasmCommand) -> Result<()> {
    let dir = default_gene_dir();

    match cmd {
        WasmCommand::Sign { file, gene, name, version, out } => {
            let wasm = std::fs::read(&file).with_context(|| format!("Cannot read {}", file.display()))?;
            let (author, kp) = load_gene(&gene, &dir, &OsKeyring::new())?;
            let module = GeneModule::sign(&name, &version, &wasm, &author, &kp)?;

            let out = out.unwrap_or_else(|| file.with_extension("gene-module.json"));
            std::fs::write(&out, serde_json::to_string_pretty(&module)?)?;
            println!("✅ Signed '{}' v{} by {} → {}", name, version, author.short_fp, out.display());
        }

        WasmCommand::Run { module, trust, fuel } => {
            let module: GeneModule = serde_json::from_str(&std::fs::read_to_string(&module)?)
                .context("Invalid Gene Module JSON")?;

            let genes = list_genes(&dir)?;
            let trusted: Vec<HybridPublicKey> = genes
                .iter()
                .filter(|g| trust.is_empty() || trust.iter().any(|t| *t == g.id.to_string()))
                .map(HybridPublicKey::from_gene)
                .collect();
            if trusted.is_empty() {
                anyhow::bail!("No trusted author keys. Create a gene with `netgene gene init` or pass --trust <gene-id>.");
            }

            let sandbox = WasmSandbox::new(MEMORY_LIMIT_BYTES, trusted).with_fuel(fuel);
            println!("🧬 Executing WASM Gene Module '{}' v{}...", module.name, module.version);
            let result = sandbox.execute(&module)?;

            println!("✅ Sandbox Execution Complete:");
            println!("   Module ID:       {}", result.module_id);
            println!("   Return value:    {}", result.return_value);
            println!("   Execution Time:  {} ms", result.execution_time_ms);
            println!("   Fuel consumed:   {}", result.fuel_consumed);
        }

        WasmCommand::Status => {
            println!("⚙️  WASM Sandbox:");
            println!("   Engine:        wasmtime (no host imports, no WASI)");
            println!("   ABI:           export `run: () -> i32`");
            println!("   Memory Limit:  {} MB", MEMORY_LIMIT_BYTES / (1024 * 1024));
            println!("   Fuel (CPU):    {} units per run", DEFAULT_FUEL);
            println!("   Signatures:    hybrid Ed25519 + ML-DSA-65 by a trusted gene, mandatory");
        }
    }

    Ok(())
}
