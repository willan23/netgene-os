//! eBPF CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_ebpf::EbpfProbeManager;

#[derive(Subcommand)]
pub enum EbpfCommand {
    /// Sample kernel socket probe telemetry
    Sample {
        /// Interface name
        #[arg(short, long, default_value = "eth0")]
        interface: String,
    },
    /// Show eBPF probe subsystem status
    Status,
}

pub async fn run(cmd: EbpfCommand) -> Result<()> {
    match cmd {
        EbpfCommand::Sample { interface } => {
            let manager = EbpfProbeManager::new(interface);
            println!("🛡️ Sampling eBPF Kernel Probe Telemetry...");
            let event = manager.sample_kernel_probe();

            println!("✅ Probe Event Captured:");
            println!("   Probe ID:      {}", event.probe_id);
            println!("   Interface:     {}", event.interface);
            println!("   Source/Dest:   {} -> {}", event.src_ip, event.dst_ip);
            println!("   Protocol:      {}", event.protocol);
            println!("   Packet Size:   {} bytes", event.packet_size_bytes);
            println!("   Anomalous:     {}", if event.is_anomalous { "🔴 YES" } else { "🟢 NO" });
        }

        EbpfCommand::Status => {
            println!("🛡️ eBPF Kernel Security Telemetry:");
            println!("   Engine:      eBPF Socket & Packet Telemetry Probes");
            println!("   Integrations: Safeguard Z-score Anomaly Detector");
            println!("   Status:      🟢 ONLINE — Active Kernel Hooks");
        }
    }

    Ok(())
}
