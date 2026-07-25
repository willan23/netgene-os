use clap::Subcommand;
use anyhow::Result;
use netgene_lite::{LiteNode, LiteNodeMode};

#[derive(Subcommand)]
pub enum LiteCommand {
    /// Start a Lite Node in Relay mode
    Relay {
        #[arg(short, long)]
        id: String,
    },
    /// Start a Lite Node in Sensor mode
    Sensor {
        #[arg(short, long)]
        id: String,
    },
}

pub async fn run(command: LiteCommand) -> Result<()> {
    match command {
        LiteCommand::Relay { id } => {
            println!("📡 Starting Lite Relay Node '{}'", id);
            let node = LiteNode::new(&id, LiteNodeMode::Relay);
            let telemetry = node.generate_mock_telemetry();
            println!("📡 Generated mock telemetry: {:?}", telemetry);
        }
        LiteCommand::Sensor { id } => {
            println!("🌡️ Starting Lite Sensor Node '{}'", id);
            let node = LiteNode::new(&id, LiteNodeMode::Sensor);
            let telemetry = node.generate_mock_telemetry();
            println!("🌡️ Generated mock telemetry: {:?}", telemetry);
        }
    }
    Ok(())
}
