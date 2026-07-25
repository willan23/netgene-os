//! Neural CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_neural::NeuralStreamAdapter;

#[derive(Subcommand)]
pub enum NeuralCommand {
    /// Stream mock BCI signals & convert to kernel intents
    Stream {
        /// Focus target node or subsystem
        #[arg(short, long, default_value = "node-01")]
        target: String,
        /// Beta wave power (active thought 0.0 - 1.0)
        #[arg(short, long, default_value = "0.75")]
        beta: f64,
        /// Gamma wave power (cognitive load 0.0 - 1.0)
        #[arg(short, long, default_value = "0.60")]
        gamma: f64,
    },
    /// Show neural interface hardware status
    Status,
}

pub async fn run(cmd: NeuralCommand) -> Result<()> {
    match cmd {
        NeuralCommand::Stream { target, beta, gamma } => {
            let (adapter, _rx) = NeuralStreamAdapter::new();
            println!("🧠 Neural & BCI Intent Stream Adapter");
            println!("   Target: {}", target);
            println!("   Beta Wave (Active Thought):  {:.2}", beta);
            println!("   Gamma Wave (Cognitive Load): {:.2}", gamma);
            println!();

            let event = adapter.process_signal(&target, beta, gamma).await?;

            println!("✅ Neural Intent Generated:");
            println!("   Event ID:         {}", event.event_id);
            println!("   Cognitive Load:   {:.1}%", event.cognitive_load * 100.0);
            println!("   Converted Action: {}", event.converted_action);
            println!("   Signal Quality:   {:.1}%", event.raw_signals.signal_quality * 100.0);
        }

        NeuralCommand::Status => {
            println!("🧠 Neural BCI Interface Status:");
            println!("   Device:    Simulated OpenBCI Cyton 8-channel EEG");
            println!("   Bands:     Alpha (8-12Hz), Beta (12-30Hz), Gamma (30-100Hz)");
            println!("   Pipeline:  Direct Thought-to-Action Zero-Latency Pipeline");
            println!("   Status:    🟢 ONLINE — Calibrated");
        }
    }

    Ok(())
}
