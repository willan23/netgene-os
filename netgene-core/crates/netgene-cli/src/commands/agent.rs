//! Agent CLI commands.

use clap::Subcommand;
use anyhow::Result;
use std::path::PathBuf;
use netgene_kernel::{DispatchOutcome, KernelConfig, NetSphereKernel};
use netgene_kernel::marketplace::{AgentManifest, Marketplace};
use netgene_gene::{secret::OsKeyring, storage::{default_gene_dir, load_gene}};
use netgene_policy::{AuditLog, Policy};

fn netgene_home() -> PathBuf {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".netgene")
}

fn audit_path() -> PathBuf {
    netgene_home().join("audit.jsonl")
}

fn load_policy() -> Result<Policy> {
    Policy::load_or_default(&netgene_home().join("policy.json"))
}

#[derive(Subcommand)]
pub enum AgentCommand {
    /// List all active agents
    List,
    /// Propose a natural-language intent; the policy decides whether it runs
    Dispatch {
        /// Intent string (e.g. "spawn 2 nodes", "optimize routes")
        intent: String,
        /// Gene acting as operator (its capabilities are checked; it signs the audit log)
        #[arg(long)]
        gene: String,
        /// Approve an action that the policy holds for confirmation
        #[arg(long)]
        confirm: bool,
    },
    /// Verify the audit log and show its latest entries
    Audit {
        /// Number of entries to show
        #[arg(short = 'n', long, default_value_t = 20)]
        last: usize,
    },
    /// Show the effective policy (~/.netgene/policy.json or the default)
    Policy,
    /// Show agent system status
    Status,
    /// Sign and publish an agent manifest to the local marketplace
    Publish {
        /// ID of the publishing gene
        #[arg(long)]
        gene: String,
        /// Agent id (a-z, 0-9, '-')
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Capabilities, comma separated
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
    },
    /// List verified agents in the local marketplace
    Market,
}

pub async fn run(cmd: AgentCommand, _json: bool) -> Result<()> {
    match cmd {
        AgentCommand::List => {
            let kernel = NetSphereKernel::boot().await?;
            println!("🤖 Netsphere Kernel Agents");
            println!("{}", "─".repeat(50));
            for agent in kernel.agent_list().await {
                println!("  • {} [{}] — {:?}", agent.name, agent.agent_type, agent.status);
            }
            kernel.shutdown().await?;
        }

        AgentCommand::Dispatch { intent, gene, confirm } => {
            let (operator, operator_kp) = load_gene(&gene, &default_gene_dir(), &OsKeyring::new())?;
            let config = KernelConfig { operator, operator_kp, policy: load_policy()?, audit_path: Some(audit_path()) };
            let kernel = NetSphereKernel::boot_with(config).await?;

            println!("📡 Proposing intent: \"{}\"", intent);
            let mut outcome = kernel.propose_intent(&intent).await?;
            println!("{}", outcome.summary());

            if let DispatchOutcome::PendingConfirmation { proposal_id, .. } = &outcome {
                if confirm {
                    outcome = kernel.confirm(*proposal_id).await?;
                    println!("{}", outcome.summary());
                } else {
                    println!("   Re-run with --confirm to approve. Nothing was executed.");
                }
            }

            // Give agents time to process
            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
            kernel.shutdown().await?;
        }

        AgentCommand::Audit { last } => {
            let path = audit_path();
            if !path.exists() {
                println!("No audit log yet ({}).", path.display());
                return Ok(());
            }
            let total = AuditLog::verify_file(&path)?;
            println!("🔏 Audit log verified: {} entries, hash chain and signatures intact ({})", total, path.display());
            let entries = AuditLog::read_entries(&path)?;
            for e in entries.iter().skip(entries.len().saturating_sub(last)) {
                println!(
                    "  #{:<4} {} {:<9} {:<17} {}",
                    e.seq,
                    e.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    e.record.decision,
                    e.record.action.as_deref().unwrap_or("-"),
                    e.record.input
                );
            }
        }

        AgentCommand::Policy => {
            println!("{}", serde_json::to_string_pretty(&load_policy()?)?);
        }

        AgentCommand::Status => {
            let kernel = NetSphereKernel::boot().await?;
            println!("🟢 Netsphere Kernel: ONLINE (in-process)");
            println!("   Agents registered: {}", kernel.agent_list().await.len());
            println!("   Bus: Tokio MPSC");
            println!("   Consensus: none (single process)");
            kernel.shutdown().await?;
        }

        AgentCommand::Publish { gene, id, name, version, description, capabilities } => {
            let (publisher, kp) = load_gene(&gene, &default_gene_dir(), &OsKeyring::new())?;
            let manifest = AgentManifest::new_signed(&id, &name, &version, &description, capabilities, &publisher, &kp)?;
            Marketplace::new().publish_manifest(&manifest)?;
            println!("✅ Published '{}' v{} signed by {}", manifest.id, manifest.version, publisher.short_fp);
        }

        AgentCommand::Market => {
            let agents = Marketplace::new().list_available_agents();
            println!("🛒 Marketplace — {} verified agent(s)", agents.len());
            for a in agents {
                println!("  • {} v{} — {} [publisher {}]", a.id, a.version, a.name, a.publisher_fingerprint());
            }
        }
    }

    Ok(())
}
