//! WASM CLI commands.

use clap::Subcommand;
use anyhow::{Context, Result};
use std::path::PathBuf;
use netgene_gene::{
    storage::{default_gene_dir, list_genes, load_gene},
    HybridPublicKey,
};
use netgene_wasm::{ExecutionInput, GeneModule, WasmSandbox, DEFAULT_FUEL};

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
        /// Host permissions the module needs: log, io, kv, clock (comma separated)
        #[arg(long, value_delimiter = ',')]
        permissions: Vec<String>,
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
        /// Permissions this run may grant (comma separated); nothing is granted by default
        #[arg(long, value_delimiter = ',')]
        allow: Vec<String>,
        /// Input bytes for modules with the `io` permission
        #[arg(long, default_value = "")]
        input: String,
    },
    /// Show WASM sandbox limits
    Status,
}

pub async fn run(cmd: WasmCommand) -> Result<()> {
    let dir = default_gene_dir();

    match cmd {
        WasmCommand::Sign { file, gene, name, version, out, permissions } => {
            let wasm = std::fs::read(&file).with_context(|| format!("Cannot read {}", file.display()))?;
            let (author, kp) = load_gene(&gene, &dir, &netgene_gene::default_store()?)?;
            let module = GeneModule::sign(&name, &version, &wasm, permissions, &author, &kp)?;

            let out = out.unwrap_or_else(|| file.with_extension("gene-module.json"));
            std::fs::write(&out, serde_json::to_string_pretty(&module)?)?;
            println!("✅ Signed '{}' v{} by {} [permissions: {}] → {}", name, version, author.short_fp,
                if module.permissions.is_empty() { "none".to_string() } else { module.permissions.join(", ") }, out.display());
        }

        WasmCommand::Run { module, trust, fuel, allow, input } => {
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

            let sandbox = WasmSandbox::new(MEMORY_LIMIT_BYTES, trusted).with_fuel(fuel).allow_permissions(allow);
            println!("🧬 Executing WASM Gene Module '{}' v{} [permissions: {}]...", module.name, module.version,
                if module.permissions.is_empty() { "none".to_string() } else { module.permissions.join(", ") });
            let result = sandbox.execute(&module, ExecutionInput { input: input.into_bytes(), ..Default::default() })?;

            println!("✅ Sandbox Execution Complete:");
            println!("   Module ID:       {}", result.module_id);
            println!("   Return value:    {}", result.return_value);
            println!("   Execution Time:  {} ms", result.execution_time_ms);
            println!("   Fuel consumed:   {}", result.fuel_consumed);
            for line in &result.logs {
                println!("   📝 log: {}", line);
            }
            if !result.output.is_empty() {
                println!("   Output:          {}", String::from_utf8_lossy(&result.output));
            }
        }

        WasmCommand::Status => {
            println!("⚙️  WASM Sandbox:");
            println!("   Engine:        wasmtime (no WASI, no filesystem, no network)");
            println!("   ABI:           export `run: () -> i32`; host imports from `netgene` per permission");
            println!("   Permissions:   log, io, kv, clock (declared + signed by the module, granted with --allow)");
            println!("   Memory Limit:  {} MB", MEMORY_LIMIT_BYTES / (1024 * 1024));
            println!("   Fuel (CPU):    {} units per run", DEFAULT_FUEL);
            println!("   Signatures:    hybrid Ed25519 + ML-DSA-65 by a trusted gene, mandatory");
        }
    }

    Ok(())
}
