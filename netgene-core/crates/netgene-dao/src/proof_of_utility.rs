//! Proof-of-Utility reward calculator for network computational & quantum contributions.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtilityRewardReceipt {
    pub receipt_id: Uuid,
    pub node_id: String,
    pub qpu_shots_contributed: usize,
    pub ebpf_packets_inspected: u64,
    pub total_utility_tokens: f64,
    pub issued_at: DateTime<Utc>,
}

pub struct ProofOfUtilityEngine;

impl ProofOfUtilityEngine {
    /// Calculate utility tokens earned for hardware QPU shots and eBPF kernel inspection
    pub fn calculate_reward(node_id: &str, qpu_shots: usize, ebpf_packets: u64) -> UtilityRewardReceipt {
        let qpu_reward = (qpu_shots as f64) * 0.05;
        let ebpf_reward = (ebpf_packets as f64) * 0.0001;
        let total = qpu_reward + ebpf_reward;

        UtilityRewardReceipt {
            receipt_id: Uuid::new_v4(),
            node_id: node_id.to_string(),
            qpu_shots_contributed: qpu_shots,
            ebpf_packets_inspected: ebpf_packets,
            total_utility_tokens: total,
            issued_at: Utc::now(),
        }
    }
}
