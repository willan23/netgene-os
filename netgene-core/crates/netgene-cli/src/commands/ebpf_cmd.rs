//! eBPF CLI commands (Linux only).

use clap::Subcommand;
use anyhow::Result;
use netgene_ebpf::{totals, XdpMonitor};
use netgene_safeguard::anomaly::AnomalyDetector;

#[derive(Subcommand)]
pub enum EbpfCommand {
    /// Attach the XDP flow counter to an interface and report traffic per second
    Watch {
        /// Network interface (e.g. eth0, lo)
        #[arg(long, default_value = "eth0")]
        iface: String,
        /// How long to watch
        #[arg(long, default_value_t = 10)]
        seconds: u64,
    },
}

pub async fn run(cmd: EbpfCommand) -> Result<()> {
    match cmd {
        EbpfCommand::Watch { iface, seconds } => {
            let monitor = XdpMonitor::attach(&iface)?;
            println!("🐧 XDP flow counter attached to {} (generic mode, traffic is never dropped)", iface);

            // Learn the baseline for a few samples before flagging anomalies.
            let mut detector = AnomalyDetector::new(10, 3.0);
            let mut previous = 0u64;
            for second in 1..=seconds {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let flows = monitor.snapshot()?;
                let (packets, bytes) = totals(&flows);
                let rate = packets.saturating_sub(previous);
                previous = packets;

                let top = flows
                    .iter()
                    .take(3)
                    .map(|f| format!("{} {}p", f.source, f.packets))
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  t={:>3}s  {:>6} pkt/s  total {} pkts / {} B  top: {}", second, rate, packets, bytes, top);

                if let Some(event) = detector.ingest("packets_per_sec", rate as f64) {
                    println!("  🛡️ Safeguard anomaly: {} = {} (z = {:.1}, {:?})", event.metric, event.value, event.z_score, event.severity);
                }
            }
            println!("✅ Detached from {}", iface);
        }
    }
    Ok(())
}
