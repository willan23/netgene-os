//! P2P Autonomous Governance, Proposals & Code Mutation Consensus.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::Result;
use tracing::info;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProposalType {
    KernelMutation { code_hash: String },
    QuantumWeightUpdate { default_layers: usize },
    NodePolicyUpdate { max_nodes: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaoProposal {
    pub proposal_id: Uuid,
    pub title: String,
    pub description: String,
    pub proposer_gene_id: String,
    pub proposal_type: ProposalType,
    pub votes_yes: u64,
    pub votes_no: u64,
    pub voters: HashSet<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

pub struct GovernanceEngine {
    quorum_percentage: f64,
}

impl GovernanceEngine {
    pub fn new(quorum_percentage: f64) -> Self {
        Self { quorum_percentage }
    }

    /// Submit a new evolutionary governance proposal to P2P network
    pub fn submit_proposal(
        &self, title: &str, description: &str, proposer: &str, proposal_type: ProposalType
    ) -> DaoProposal {
        let mut voters = HashSet::new();
        voters.insert(proposer.to_string());

        let proposal = DaoProposal {
            proposal_id: Uuid::new_v4(),
            title: title.to_string(),
            description: description.to_string(),
            proposer_gene_id: proposer.to_string(),
            proposal_type,
            votes_yes: 1, // Proposer vote
            votes_no: 0,
            voters,
            status: "ACTIVE".to_string(),
            created_at: Utc::now(),
        };

        info!("🏛️ DAO Proposal Submitted: '{}' by '{}' ({})", proposal.title, proposal.proposer_gene_id, proposal.proposal_id);
        proposal
    }

    /// Cast weighted vote for active DAO proposal with double-voting prevention
    pub fn vote(&self, proposal: &mut DaoProposal, voter: &str, approve: bool, weight: u64) -> Result<()> {
        if proposal.status != "ACTIVE" {
            anyhow::bail!("Security Warning: Proposal is no longer active");
        }

        if weight == 0 {
            anyhow::bail!("Security Warning: Voting weight must be greater than zero");
        }

        if proposal.voters.contains(voter) {
            anyhow::bail!("Security Violation: Voter '{}' has already voted on this proposal", voter);
        }

        proposal.voters.insert(voter.to_string());

        if approve {
            proposal.votes_yes += weight;
        } else {
            proposal.votes_no += weight;
        }

        info!(
            "🏛️ Vote cast by '{}' on Proposal '{}': approve={}, weight={}. Total YES={}, NO={}",
            voter, proposal.title, approve, weight, proposal.votes_yes, proposal.votes_no
        );

        if proposal.votes_yes >= 100 {
            proposal.status = "PASSED".to_string();
            info!("🏛️ DAO Proposal '{}' PASSED quorum threshold!", proposal.title);
        }

        Ok(())
    }

    pub fn quorum_percentage(&self) -> f64 {
        self.quorum_percentage
    }
}
