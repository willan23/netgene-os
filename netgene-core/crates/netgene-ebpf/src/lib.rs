//! # NetGene eBPF
//!
//! Real kernel telemetry on Linux: an XDP program (`bpf/netgene_xdp.c`)
//! counts packets and bytes per IPv4 source on an interface and always
//! passes traffic through. User space reads the counters from a BPF map.
//!
//! Loading needs root (or `CAP_BPF` + `CAP_NET_ADMIN`). On other operating
//! systems every call returns an explicit "not supported" error; nothing is
//! simulated.

use std::net::Ipv4Addr;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Counters for one IPv4 source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowStat {
    pub source: Ipv4Addr,
    pub packets: u64,
    pub bytes: u64,
}

/// Total packets/bytes across flows.
pub fn totals(flows: &[FlowStat]) -> (u64, u64) {
    flows.iter().fold((0, 0), |(p, b), f| (p + f.packets, b + f.bytes))
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use anyhow::Context;
    use aya::maps::HashMap as BpfHashMap;
    use aya::programs::{Xdp, XdpMode};
    use aya::Ebpf;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct RawStats {
        packets: u64,
        bytes: u64,
    }

    // SAFETY: plain-old-data with the same layout as `struct flow_stats` in the C program.
    unsafe impl aya::Pod for RawStats {}

    static PROGRAM: &[u8] = aya::include_bytes_aligned!(concat!(env!("OUT_DIR"), "/netgene_xdp.o"));

    /// A loaded and attached XDP flow counter; detached on drop.
    pub struct XdpMonitor {
        ebpf: Ebpf,
        pub interface: String,
    }

    impl XdpMonitor {
        /// Load the program and attach it to `interface` in generic (SKB) mode,
        /// which works on every driver.
        pub fn attach(interface: &str) -> Result<Self> {
            let mut ebpf = Ebpf::load(PROGRAM).context("Cannot load the XDP program (are you root?)")?;
            let program: &mut Xdp = ebpf
                .program_mut("netgene_count")
                .context("XDP program `netgene_count` missing")?
                .try_into()?;
            program.load().context("The kernel rejected the XDP program")?;
            program
                .attach(interface, XdpMode::Skb)
                .with_context(|| format!("Cannot attach XDP to interface '{}'", interface))?;
            Ok(Self { ebpf, interface: interface.to_string() })
        }

        /// Current counters, busiest first.
        pub fn snapshot(&self) -> Result<Vec<FlowStat>> {
            let map: BpfHashMap<_, u32, RawStats> = BpfHashMap::try_from(self.ebpf.map("flows").context("map `flows` missing")?)?;
            let mut flows: Vec<FlowStat> = map
                .iter()
                .filter_map(|item| item.ok())
                .map(|(addr, stats)| FlowStat {
                    // The key is the address in network byte order.
                    source: Ipv4Addr::from(u32::from_be(addr)),
                    packets: stats.packets,
                    bytes: stats.bytes,
                })
                .collect();
            flows.sort_by(|a, b| b.packets.cmp(&a.packets));
            Ok(flows)
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::XdpMonitor;

/// Stand-in on non-Linux systems: attaching always fails.
#[cfg(not(target_os = "linux"))]
pub struct XdpMonitor {
    pub interface: String,
}

#[cfg(not(target_os = "linux"))]
impl XdpMonitor {
    pub fn attach(_interface: &str) -> Result<Self> {
        anyhow::bail!("eBPF/XDP telemetry requires Linux (run the node on Linux or in WSL2)")
    }

    pub fn snapshot(&self) -> Result<Vec<FlowStat>> {
        anyhow::bail!("eBPF/XDP telemetry requires Linux")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_totals() {
        let flows = [
            FlowStat { source: Ipv4Addr::LOCALHOST, packets: 3, bytes: 300 },
            FlowStat { source: Ipv4Addr::new(10, 0, 0, 1), packets: 2, bytes: 100 },
        ];
        assert_eq!(totals(&flows), (5, 400));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn test_non_linux_is_explicit() {
        assert!(XdpMonitor::attach("eth0").is_err());
    }
}
