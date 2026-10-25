//! Agent CLI commands.

use clap::Subcommand;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use netgene_kernel::{DispatchOutcome, KernelConfig, NetSphereKernel, Workspace};
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

fn workspace() -> Result<Workspace> {
    let secrets: Arc<dyn netgene_gene::SecretStore> = Arc::from(netgene_gene::default_store()?);
    Ok(Workspace::at(&netgene_home(), default_gene_dir(), secrets, netgene_store::NetGeneStore::open(None)?))
}

/// Boot a kernel acting as `gene`, with its tokens, presence key, policy and audit log.
async fn boot_as(gene: &str) -> Result<(NetSphereKernel, Option<PresenceEnrollment>)> {
    let (operator, operator_kp) = load_gene(gene, &default_gene_dir(), &netgene_gene::default_store()?)?;
    let grants = grants_for(&operator, &list_genes(&default_gene_dir())?)?;
    for g in &grants {
        println!("🎫 Token {} grants [{}] until {}", g.token_id, g.capabilities.join(", "), g.expires_at.format("%Y-%m-%d %H:%M UTC"));
    }
    let enrollment = PresenceEnrollment::load(&presence_dir(), operator.id)?;
    let presence_key = enrollment.as_ref().map(|e| e.public_key()).transpose()?;
    let config = KernelConfig {
        operator, operator_kp, policy: load_policy()?, audit_path: Some(audit_path()), grants, presence_key, workspace: workspace()?,
    };
    Ok((NetSphereKernel::boot_with(config).await?, enrollment))
}

/// Approve a pending proposal when `confirm` is set (with Windows Hello if the policy says so).
async fn resolve(kernel: &NetSphereKernel, outcome: DispatchOutcome, confirm: bool, enrollment: Option<&PresenceEnrollment>, gene: &str) -> Result<()> {
    println!("{}", outcome.summary());
    let DispatchOutcome::PendingConfirmation { proposal_id, .. } = outcome else { return Ok(()) };
    if !confirm {
        println!("   Re-run with --confirm to approve. Nothing was executed.");
        return Ok(());
    }
    let outcome = if kernel.requires_user_presence() {
        let enrollment = enrollment.ok_or_else(|| {
            anyhow::anyhow!("Policy requires user presence: run `netgene gene enroll-presence --gene {}` first", gene)
        })?;
        let challenge = kernel.presence_challenge(proposal_id).await?;
        println!("🔐 Waiting for Windows Hello approval...");
        let signed = tokio::task::spawn_blocking({
            let enrollment = enrollment.clone();
            move || netgene_presence::sign(&enrollment, &challenge)
        })
        .await?;
        match signed {
            Ok(signature) => kernel.confirm_with_presence(proposal_id, &signature).await?,
            Err(e) => {
                // Cancelled or failed: record the rejection so the audit log shows it.
                kernel.reject(proposal_id).await?;
                return Err(e);
            }
        }
    } else {
        kernel.confirm(proposal_id).await?
    };
    println!("{}", outcome.summary());
    Ok(())
}

#[derive(Subcommand)]
pub enum AgentCommand {
    /// List installed WASM agents and whether their author is trusted
    List {
        /// Gene whose lineage defines trusted authors (default: nothing is trusted)
        #[arg(long)]
        gene: Option<String>,
    },
    /// Install a signed Gene Module (see `wasm sign`) as an agent
    Install {
        /// Gene Module JSON
        module: PathBuf,
        /// Gene whose lineage must have signed it
        #[arg(long)]
        gene: String,
    },
    /// Run an installed agent through the policy (agent.run)
    Run {
        /// Agent name
        agent: String,
        /// Input text handed to the agent (`io` permission)
        #[arg(long, default_value = "")]
        input: String,
        #[arg(long)]
        gene: String,
        /// Approve the run if the policy holds it for confirmation
        #[arg(long)]
        confirm: bool,
    },
    /// Propose a natural-language intent; the policy decides whether it runs
    Dispatch {
        /// Intent string (e.g. "spawn 2 nodes", "run agent echo with hi")
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
        AgentCommand::List { gene } => {
            let kernel = match gene {
                Some(gene) => boot_as(&gene).await?.0,
                None => NetSphereKernel::boot_with(KernelConfig { workspace: workspace()?, ..KernelConfig::ephemeral()? }).await?,
            };
            let agents = kernel.agent_list().await;
            println!("🤖 Installed agents ({})", agents.len());
            for agent in agents {
                println!("  • {} [{}] — {}", agent.name, agent.agent_type, agent.status);
            }
        }

        AgentCommand::Install { module, gene } => {
            let module: netgene_wasm::GeneModule = serde_json::from_slice(&std::fs::read(&module)?)?;
            let (kernel, _) = boot_as(&gene).await?;
            let path = kernel.install_wasm_agent(&module).await?;
            println!("✅ Installed agent '{}' v{} [{}] at {}", module.name, module.version, module.permissions.join(", "), path.display());
        }

        AgentCommand::Run { agent, input, gene, confirm } => {
            let (kernel, enrollment) = boot_as(&gene).await?;
            let action = netgene_policy::Action::new("agent.run", serde_json::json!({ "agent": agent, "input": input }));
            let outcome = kernel.propose_action(&format!("agent run {}", agent), action, "requested from the CLI".into()).await?;
            resolve(&kernel, outcome, confirm, enrollment.as_ref(), &gene).await?;
        }

        AgentCommand::Dispatch { intent, gene, confirm } => {
            let (kernel, enrollment) = boot_as(&gene).await?;
            println!("📡 Proposing intent: \"{}\"", intent);
            let outcome = kernel.propose_intent(&intent).await?;
            resolve(&kernel, outcome, confirm, enrollment.as_ref(), &gene).await?;
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
            let kernel = NetSphereKernel::boot_with(KernelConfig { workspace: workspace()?, ..KernelConfig::ephemeral()? }).await?;
            let policy = load_policy()?;
            println!("🟢 Netsphere Kernel (in-process, starts per command)");
            println!("   Installed agents:  {}", kernel.agent_list().await.len());
            println!("   Actions:           {}", netgene_policy::ACTION_CATALOG.iter().map(|a| a.kind).collect::<Vec<_>>().join(", "));
            println!("   Agent permissions: {}", policy.agent_permissions.join(", "));
            println!("   Audit log:         {}", audit_path().display());
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
