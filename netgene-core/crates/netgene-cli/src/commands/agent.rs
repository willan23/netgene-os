//! Agent CLI commands.

use clap::Subcommand;
use anyhow::Result;
use std::path::PathBuf;
use netgene_kernel::{DispatchOutcome, KernelConfig, NetSphereKernel};
use netgene_kernel::marketplace::{AgentManifest, Marketplace};
use netgene_kernel::reputation::{AgentReview, ReviewStore};
use netgene_gene::{storage::{default_gene_dir, list_genes, load_gene, trusted_lineage}, HybridPublicKey};
use netgene_policy::{AuditLog, Policy};
use super::grants::{grants_for, netgene_home, presence_dir};
use netgene_presence::PresenceEnrollment;

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
    /// Verify the audit log and show its latest entries (or export it for a SIEM)
    Audit {
        /// Number of entries to show
        #[arg(short = 'n', long, default_value_t = 20)]
        last: usize,
        /// Export every entry: ocsf (JSON lines) or cef
        #[arg(long)]
        export: Option<String>,
        /// Export destination (default: stdout)
        #[arg(long)]
        out: Option<PathBuf>,
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
    /// List verified agents in the local marketplace, with reputation
    Market {
        /// Gene whose lineage defines which reviewers are trusted
        #[arg(long)]
        gene: Option<String>,
    },
    /// Rate a marketplace agent (signed review of its exact version)
    Review {
        /// Reviewing gene
        #[arg(long)]
        gene: String,
        /// Agent id
        #[arg(long)]
        agent: String,
        /// 1 to 5
        #[arg(long)]
        rating: u8,
        #[arg(long, default_value = "")]
        comment: String,
    },
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
            let (operator, operator_kp) = load_gene(&gene, &default_gene_dir(), &netgene_gene::default_store()?)?;
            let grants = grants_for(&operator, &list_genes(&default_gene_dir())?)?;
            for g in &grants {
                println!("🎫 Token {} grants [{}] until {}", g.token_id, g.capabilities.join(", "), g.expires_at.format("%Y-%m-%d %H:%M UTC"));
            }
            let enrollment = PresenceEnrollment::load(&presence_dir(), operator.id)?;
            let presence_key = enrollment.as_ref().map(|e| e.public_key()).transpose()?;
            let config = KernelConfig {
                operator, operator_kp, policy: load_policy()?, audit_path: Some(audit_path()), grants, presence_key,
            };
            let kernel = NetSphereKernel::boot_with(config).await?;

            println!("📡 Proposing intent: \"{}\"", intent);
            let mut outcome = kernel.propose_intent(&intent).await?;
            println!("{}", outcome.summary());

            if let DispatchOutcome::PendingConfirmation { proposal_id, .. } = &outcome {
                if confirm && kernel.requires_user_presence() {
                    let enrollment = enrollment.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("Policy requires user presence: run `netgene gene enroll-presence --gene {}` first", gene)
                    })?;
                    let challenge = kernel.presence_challenge(*proposal_id).await?;
                    println!("🔐 Waiting for Windows Hello approval...");
                    let signed = tokio::task::spawn_blocking({
                        let enrollment = enrollment.clone();
                        move || netgene_presence::sign(&enrollment, &challenge)
                    })
                    .await?;
                    let signature = match signed {
                        Ok(signature) => signature,
                        Err(e) => {
                            // Cancelled or failed: record the rejection so the audit log shows it.
                            kernel.reject(*proposal_id).await?;
                            kernel.shutdown().await?;
                            return Err(e);
                        }
                    };
                    outcome = kernel.confirm_with_presence(*proposal_id, &signature).await?;
                    println!("{}", outcome.summary());
                } else if confirm {
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

        AgentCommand::Audit { last, export, out } => {
            let path = audit_path();
            if !path.exists() {
                println!("No audit log yet ({}).", path.display());
                return Ok(());
            }
            let total = AuditLog::verify_file(&path)?;
            if let Some(format) = export {
                let entries = AuditLog::read_entries(&path)?;
                let lines: Vec<String> = match format.as_str() {
                    "ocsf" => entries.iter().map(|e| netgene_policy::export::to_ocsf(e).to_string()).collect(),
                    "cef" => entries.iter().map(netgene_policy::export::to_cef).collect(),
                    other => anyhow::bail!("Unknown export format '{}' (use ocsf or cef)", other),
                };
                let body = lines.join("\n") + "\n";
                match out {
                    Some(file) => {
                        std::fs::write(&file, body)?;
                        eprintln!("📤 Exported {} verified entries as {} to {}", total, format, file.display());
                    }
                    None => print!("{}", body),
                }
                return Ok(());
            }
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
            let (publisher, kp) = load_gene(&gene, &default_gene_dir(), &netgene_gene::default_store()?)?;
            let manifest = AgentManifest::new_signed(&id, &name, &version, &description, capabilities, &publisher, &kp)?;
            Marketplace::new().publish_manifest(&manifest)?;
            println!("✅ Published '{}' v{} signed by {}", manifest.id, manifest.version, publisher.short_fp);
        }

        AgentCommand::Market { gene } => {
            let agents = Marketplace::new().list_available_agents();
            let trusted: Vec<HybridPublicKey> = match gene {
                Some(id) => {
                    let genes = list_genes(&default_gene_dir())?;
                    let me = genes.iter().find(|g| g.id.to_string() == id).ok_or_else(|| anyhow::anyhow!("Gene {} not found", id))?;
                    trusted_lineage(me, &genes).iter().map(HybridPublicKey::from_gene).collect()
                }
                None => vec![],
            };
            let reputations = ReviewStore::default().reputations(&agents, &trusted)?;
            println!("🛒 Marketplace — {} verified agent(s)", agents.len());
            for a in agents {
                let rep = &reputations[&a.id];
                let score = rep.score.map(|s| format!("★ {:.1} ({} trusted)", s, rep.trusted_reviews)).unwrap_or_else(|| "★ — (no trusted reviews)".into());
                println!("  • {} v{} — {} [publisher {}] {} | untrusted: {}, rejected: {}",
                    a.id, a.version, a.name, a.publisher_fingerprint(), score, rep.untrusted_reviews, rep.rejected_reviews);
            }
        }

        AgentCommand::Review { gene, agent, rating, comment } => {
            let (reviewer, kp) = load_gene(&gene, &default_gene_dir(), &netgene_gene::default_store()?)?;
            let manifest = Marketplace::new().install_agent(&agent)?;
            let review = AgentReview::new_signed(&manifest, rating, &comment, &reviewer, &kp)?;
            ReviewStore::default().add(&review)?;
            println!("⭐ Review {} saved: {} v{} rated {}/5 by {}", review.review_id, manifest.id, manifest.version, rating, reviewer.short_fp);
        }
    }

    Ok(())
}
