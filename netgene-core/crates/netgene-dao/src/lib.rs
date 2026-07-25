//! # NetGene P2P Autonomous Governance & DAO Engine (`netgene-dao`)

pub mod governance;
pub mod proof_of_utility;

pub use governance::{GovernanceEngine, DaoProposal, ProposalType};
pub use proof_of_utility::{ProofOfUtilityEngine, UtilityRewardReceipt};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dao_proposal_and_voting() -> anyhow::Result<()> {
        let engine = GovernanceEngine::new(66.0);
        let mut proposal = engine.submit_proposal(
            "Upgrade QAOA Layers to 5",
            "Enhance quantum routing accuracy",
            "gene-master-01",
            ProposalType::QuantumWeightUpdate { default_layers: 5 },
        );

        assert_eq!(proposal.status, "ACTIVE");
        engine.vote(&mut proposal, "voter-node-02", true, 100)?;
        assert_eq!(proposal.status, "PASSED");

        Ok(())
    }

    #[test]
    fn test_proof_of_utility_reward() {
        let receipt = ProofOfUtilityEngine::calculate_reward("node-01", 1000, 50000);
        assert!(receipt.total_utility_tokens > 50.0);
    }

    #[test]
    fn test_dao_double_voting_prevention() -> anyhow::Result<()> {
        let engine = GovernanceEngine::new(66.0);
        let mut proposal = engine.submit_proposal(
            "Test Double Voting",
            "Description",
            "proposer-01",
            ProposalType::NodePolicyUpdate { max_nodes: 50 },
        );

        // Proposer already voted during submission
        assert!(engine.vote(&mut proposal, "proposer-01", true, 50).is_err());
        Ok(())
    }
}
