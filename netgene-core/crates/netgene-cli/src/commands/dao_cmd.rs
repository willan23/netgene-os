//! P2P DAO & Governance CLI commands.

use clap::Subcommand;
use anyhow::Result;
use netgene_dao::{GovernanceEngine, ProposalType, ProofOfUtilityEngine};

#[derive(Subcommand)]
pub enum DaoCommand {
    /// Submit evolutionary code mutation proposal
    Propose {
        #[arg(short, long, default_value = "Upgrade QAOA to Layer 5")]
        title: String,
        #[arg(short, long, default_value = "gene-master-01")]
        proposer: String,
    },
    /// Calculate Proof-of-Utility token rewards
    Reward {
        #[arg(short, long, default_value = "node-alpha-01")]
        node: String,
        #[arg(short, long, default_value_t = 1000)]
        qpu_shots: usize,
        #[arg(short, long, default_value_t = 50000)]
        packets: u64,
    },
    /// Show P2P DAO governance status
    Status,
}

pub async fn run(cmd: DaoCommand) -> Result<()> {
    match cmd {
        DaoCommand::Propose { title, proposer } => {
            let engine = GovernanceEngine::new(66.0);
            let mut proposal = engine.submit_proposal(
                &title,
                "Auto-generated evolutionary proposal",
                &proposer,
                ProposalType::QuantumWeightUpdate { default_layers: 5 },
            );

            println!("🏛️  P2P DAO Proposal Submitted:");
            println!("   Proposal ID: {}", proposal.proposal_id);
            println!("   Title:       {}", proposal.title);
            println!("   Proposer:    {}", proposal.proposer_gene_id);
            println!("   Status:      {}", proposal.status);

            engine.vote(&mut proposal, "voter-node-02", true, 100)?;
            println!("   Final Status after voting: {}", proposal.status);
        }

        DaoCommand::Reward { node, qpu_shots, packets } => {
            let receipt = ProofOfUtilityEngine::calculate_reward(&node, qpu_shots, packets);

            println!("🏛️  Proof-of-Utility Reward Receipt:");
            println!("   Receipt ID:   {}", receipt.receipt_id);
            println!("   Node ID:      {}", receipt.node_id);
            println!("   QPU Shots:    {}", receipt.qpu_shots_contributed);
            println!("   Packets:      {}", receipt.ebpf_packets_inspected);
            println!("   Tokens:       {:.4} GENE", receipt.total_utility_tokens);
        }

        DaoCommand::Status => {
            println!("🏛️  P2P Autonomous Governance Status:");
            println!("   Consensus:   Proof-of-Utility (PoU)");
            println!("   Quorum:      66% Weighted Majority");
            println!("   Status:      🟢 ONLINE — Decentralized Mesh Active");
        }
    }

    Ok(())
}
