use actix::prelude::*;
use anyhow::{anyhow, bail};
use std::cmp::min;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::component::ProposalView;

use ya_client_model::market::Reason;

#[derive(Debug)]
pub struct ProposalScore {
    pub proposal: ProposalView,
    pub prev: ProposalView,
    pub score: f64,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub enum Feedback {
    Decide,
    Accept {
        id: String,
    },
    Reject {
        id: String,
        reason: Option<Reason>,
        is_final: bool,
    },
}

/// Stores Proposals together with their Score. Triggers decision based on
/// number of collected Offers or time that elapsed.
pub struct ProposalsCollection {
    awaiting: Vec<ProposalScore>,
    /// Proposals that were rejected by us, but have still chance for being chosen
    /// when conditions will change. This list doesn't include Proposals rejected
    /// with `final` flag.
    rejected: Vec<ProposalScore>,

    /// Expected number of Proposals to choose.
    goal: usize,
    /// Time period before making decision, which Proposals to choose.
    collect_period: Duration,
    /// Number of Proposals to collect, after which best of them will be accepted.
    collect_amount: usize,

    feedback_channel: mpsc::UnboundedSender<Feedback>,
    pub feedback_receiver: Option<mpsc::UnboundedReceiver<Feedback>>,
}

impl ProposalsCollection {
    pub fn new() -> ProposalsCollection {
        let (feedback_sender, feedback_receiver) = mpsc::unbounded_channel();
        ProposalsCollection {
            awaiting: vec![],
            rejected: vec![],
            goal: 1,
            collect_period: Default::default(),
            collect_amount: 1,
            feedback_channel: feedback_sender,
            feedback_receiver: Some(feedback_receiver),
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
            self.feedback_channel
                .send(Feedback::Decide)
                .map_err(|_| anyhow!("Feedback channel closed."))?;
        }

        Ok(())
    }

    pub fn decide(&mut self) -> anyhow::Result<()> {
        let goal = min(self.goal, self.awaiting.len());

        // Vector is sorted so the best elements are on the beginning.
        let accepted = self.awaiting.drain(0..goal).collect::<Vec<_>>();
        let rejected = self.awaiting.drain(..).collect::<Vec<_>>();

        // We already have as many Offers as we wanted.
        self.goal = 0;

        for proposal in accepted {
            self.feedback_channel
                .send(Feedback::Accept {
                    id: proposal.proposal.id,
                })
                .ok();
        }

        for proposal in rejected {
            self.feedback_channel
                .send(Feedback::Reject {
                    id: proposal.proposal.id.clone(),
                    reason: Some(Reason::new("Node is busy.")),
                    is_final: false,
                })
                .ok();

            // We collect Proposals with too low score.
            // Proposals with invalid score won't be added.
            self.add_rejected(proposal).ok();
        }
        Ok(())
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
