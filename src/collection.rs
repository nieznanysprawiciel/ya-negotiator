use anyhow::bail;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::component::ProposalView;
use crate::ProposalAction;

use ya_client_model::market::Reason;

pub struct ProposalScore {
    pub proposal: ProposalView,
    pub prev: ProposalView,
    pub score: f64,
}

pub struct ProposalsCollection {
    awaiting: Vec<ProposalScore>,
    /// Proposals that were rejected by us, but have still chance for being chosen
    /// when conditions will change. This list doesn't include Proposals rejected
    /// with final flag.
    rejected: Vec<ProposalScore>,

    /// Expected number of Proposals to choose.
    goal: usize,
    /// Time period before making decision, which Proposals to choose.
    collect_period: Duration,
    /// Number of Proposals to collect, after which best of them will be accepted.
    collect_amount: usize,

    proposal_channel: mpsc::UnboundedSender<ProposalAction>,
}

impl ProposalsCollection {
    pub fn new(proposal_channel: mpsc::UnboundedSender<ProposalAction>) -> ProposalsCollection {
        ProposalsCollection {
            awaiting: vec![],
            rejected: vec![],
            goal: 1,
            collect_period: Default::default(),
            collect_amount: 1,
            proposal_channel,
        }
    }

    pub fn new_scored(&mut self, new: ProposalScore) -> anyhow::Result<()> {
        if new.score.is_nan() {
            bail!("Proposal [{}] score was set to NaN.", new.proposal.id);
        }

        // Keep vector sorted.
        let idx = match self
            .awaiting
            .binary_search_by(|proposal| new.score.partial_cmp(&proposal.score).unwrap())
        {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };

        self.awaiting.insert(idx, new);

        // Check if we reached number of Proposals, by which we should make
        // decision immediately without waiting `collect_period`.
        if self.awaiting.len() >= self.collect_amount {
            self.decide();
        }

        Ok(())
    }

    pub fn decide(&mut self) {
        // Vector is sorted so the best elements are on the beginning.
        let accepted = self.awaiting.drain(0..self.goal).collect::<Vec<_>>();
        let rejected = self.awaiting.drain(..).collect::<Vec<_>>();

        for proposal in accepted {
            self.proposal_channel
                .send(ProposalAction::AcceptProposal {
                    id: proposal.proposal.id,
                })
                .ok();
        }

        for proposal in rejected {
            self.proposal_channel
                .send(ProposalAction::RejectProposal {
                    id: proposal.proposal.id.clone(),
                    reason: Some(Reason::new("Node is busy.")),
                })
                .ok();

            // Proposals with wrong score won't be added.
            self.add_rejected(proposal).ok();
        }
    }

    fn add_rejected(&mut self, new: ProposalScore) -> anyhow::Result<()> {
        if new.score.is_nan() {
            bail!("Proposal [{}] score was set to NaN.", new.proposal.id);
        }

        // Keep vector sorted.
        let idx = match self
            .rejected
            .binary_search_by(|proposal| new.score.partial_cmp(&proposal.score).unwrap())
        {
            Ok(idx) => idx + 1,
            Err(idx) => idx,
        };

        self.rejected.insert(idx, new);
        Ok(())
    }
}
