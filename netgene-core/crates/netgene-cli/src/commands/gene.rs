//! Gene Layer CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_gene::{
    identity::NetGene,
    secret::OsKeyring,
    storage::{default_gene_dir, save_gene, list_genes},
};

#[derive(Subcommand)]
pub enum GeneCommand {
    /// Generate a new Master Gene (root identity)
    Init {
        /// Gene name (e.g., your name or node name)
        #[arg(short, long, default_value = "NetGene Master")]
        name: String,
    },
    /// Show all stored genes
    Show,
    /// Spawn a Sub-Gene from the master
    Spawn {
        /// Parent gene ID
        #[arg(long)]
        parent: String,
        /// New gene name
        #[arg(short, long)]
        name: String,
        /// Role: node | agent | observer
        #[arg(short, long, default_value = "node")]
        role: String,
    },
    /// Verify a gene by ID
    Verify {
        /// Gene ID to verify
        id: String,
    },
    /// Print a gene's public identity (JSON) to share with other nodes
    Export {
        /// Gene ID
        id: String,
    },
    /// Import another node's public identity (JSON produced by `gene export`)
    Import {
        /// Path to the exported JSON
        file: std::path::PathBuf,
    },
}

pub async fn run(cmd: GeneCommand, json: bool) -> Result<()> {
    let dir = default_gene_dir();
    let secrets = OsKeyring::new();

    match cmd {
        GeneCommand::Init { name } => {
            if !json {
                println!("🧬 Generating Master Gene for '{}'...", name);
            }
            let (gene, kp) = NetGene::generate_master(&name)?;
            save_gene(&gene, &kp, &dir, &secrets)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&gene)?);
            } else {
                println!();
                println!("╔══════════════════════════════════════════════════════╗");
                println!("║          🧬  Master Gene Generated                   ║");
                println!("╠══════════════════════════════════════════════════════╣");
                println!("║  ID:          {}", gene.id);
                println!("║  Name:        {}", gene.name);
                println!("║  Role:        {}", gene.role);
                println!("║  Status:      {}", gene.status);
                println!("║  Fingerprint: {}", gene.fingerprint);
                println!("║  Short FP:    {}", gene.short_fp);
                println!("║  Algorithm:   {}", gene.algorithm);
                println!("║  Capabilities:");
                for cap in &gene.capabilities {
                    println!("║    • {}", cap);
                }
                println!("╠══════════════════════════════════════════════════════╣");
                println!("║  ✅ Public identity: {}", dir.display());
                println!("║  🔐 Private keys:    OS keystore (service 'netgene-os')");
                println!("╚══════════════════════════════════════════════════════╝");
            }
        }

        GeneCommand::Show => {
            let genes = list_genes(&dir)?;
            if genes.is_empty() {
                println!("No genes found. Run `netgene gene init` to create one.");
                return Ok(());
            }

            if json {
                println!("{}", serde_json::to_string_pretty(&genes)?);
                return Ok(());
            }

            println!();
            println!("🧬 NetGene Identity Store — {} gene(s)", genes.len());
            println!("{}", "─".repeat(70));
            for gene in &genes {
                println!(
                    "  {} │ {} │ {} │ {} │ fp:{}",
                    gene.role, gene.name, gene.id, gene.status, gene.short_fp
                );
                if !gene.capabilities.is_empty() {
                    println!("    Caps: {}", gene.capabilities.join(", "));
                }
                if let Some(parent) = &gene.parent_id {
                    println!("    Parent: {}", parent);
                }
                println!();
            }
        }

        GeneCommand::Spawn { parent, name, role } => {
            use netgene_gene::identity::GeneRole;
            use netgene_gene::storage::load_gene;

            let gene_role = match role.as_str() {
                "agent" => GeneRole::Agent,
                "observer" => GeneRole::Observer,
                _ => GeneRole::Node,
            };

            let (parent_gene, parent_kp) = load_gene(&parent, &dir, &secrets)?;
            let caps = match gene_role {
                GeneRole::Node => vec!["node.spawn".to_string(), "network.read".to_string()],
                GeneRole::Agent => vec!["agent.run".to_string(), "network.read".to_string()],
                GeneRole::Observer => vec!["network.read".to_string()],
                _ => vec![],
            };

            let (sub, kp) = NetGene::spawn_sub_gene(&parent_gene, &parent_kp, &name, gene_role, caps)?;
            save_gene(&sub, &kp, &dir, &secrets)?;

            println!("✅ Sub-Gene spawned:");
            println!("   ID:   {}", sub.id);
            println!("   Name: {}", sub.name);
            println!("   Role: {}", sub.role);
            println!("   FP:   {}", sub.short_fp);
        }

        GeneCommand::Verify { id } => {
            use netgene_gene::storage::load_gene;
            match load_gene(&id, &dir, &secrets) {
                Ok((gene, kp)) => {
                    let probe = b"netgene-verify";
                    let sig = kp.sign(probe)?;
                    if !netgene_gene::crypto::verify_signature(&gene.public_key_bytes()?, probe, &sig) {
                        anyhow::bail!("Private key does not sign for gene {}", gene.id);
                    }
                    println!("✅ Gene verified (private key matches public key):");
                    println!("   {}", gene.display_line());

                    if let Some(parent_id) = gene.parent_id {
                        let parent = list_genes(&dir)?.into_iter().find(|g| g.id == parent_id);
                        match parent {
                            Some(p) if gene.verify_lineage(&p) => println!("   ✅ Lineage signed by parent {}", p.short_fp),
                            Some(_) => println!("   ❌ Lineage signature INVALID"),
                            None => println!("   ⚠️  Parent {} not found locally; lineage not checked", parent_id),
                        }
                    }
                }
                Err(e) => println!("❌ Verification failed: {}", e),
            }
        }

        GeneCommand::Export { id } => {
            let gene = list_genes(&dir)?
                .into_iter()
                .find(|g| g.id.to_string() == id)
                .ok_or_else(|| anyhow::anyhow!("Gene {} not found", id))?;
            println!("{}", serde_json::to_string_pretty(&gene)?);
        }

        GeneCommand::Import { file } => {
            let gene: NetGene = serde_json::from_str(&std::fs::read_to_string(&file)?)?;
            gene.public_key_bytes()?;
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!("{}.gene.json", gene.id));
            if path.exists() {
                anyhow::bail!("Gene {} already exists locally", gene.id);
            }
            std::fs::write(&path, serde_json::to_string_pretty(&gene)?)?;
            println!("✅ Imported public identity {} ({})", gene.name, gene.short_fp);
            println!("   It joins your mesh allowlist if its lineage is signed by your Master Gene.");
        }
    }

    Ok(())
}
