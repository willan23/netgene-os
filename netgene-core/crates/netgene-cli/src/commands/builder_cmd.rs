//! Builder CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_builder::{BuilderEngine, NodeTemplate};

#[derive(Subcommand)]
pub enum BuilderCommand {
    /// Provision nodes from a template
    Provision {
        /// Node template: edge | core | gateway | quantum
        #[arg(short, long, default_value = "edge")]
        template: String,
        /// Number of nodes to provision
        #[arg(short, long, default_value = "1")]
        count: u32,
    },
    /// Execute a natural language build intent
    Intent {
        /// Intent string (e.g. "create 3 quantum nodes with HA")
        intent: String,
    },
    /// List all provisioned nodes
    List,
}

pub async fn run(cmd: BuilderCommand) -> Result<()> {
    let mut engine = BuilderEngine::new();

    match cmd {
        BuilderCommand::Provision { template, count } => {
            let tmpl = match template.as_str() {
                "core" => NodeTemplate::Core,
                "gateway" => NodeTemplate::Gateway,
                "quantum" => NodeTemplate::Quantum,
                _ => NodeTemplate::Edge,
            };

            println!("🏗️  Provisioning {} {} node(s)...", count, template);
            let nodes = engine.provision(tmpl, count).await?;
            println!();
            println!("   ✅ Provisioned {} node(s):", nodes.len());
            for node in &nodes {
                println!("   • {} │ {} │ {} │ {}", node.id, node.ip, node.port, node.status);
            }
        }

        BuilderCommand::Intent { intent } => {
            println!("🧠 Builder Intent: \"{}\"", intent);
            let result = engine.from_intent(&intent).await?;
            println!("{}", result);
        }

        BuilderCommand::List => {
            let nodes = engine.list();
            if nodes.is_empty() {
                println!("No nodes provisioned yet. Run `netgene build provision`.");
            } else {
                println!("🏗️  Provisioned Nodes ({}):", nodes.len());
                for node in nodes {
                    println!("   • {} │ {}:{} │ {}", node.id, node.ip, node.port, node.status);
                }
            }
        }
    }

    Ok(())
}
