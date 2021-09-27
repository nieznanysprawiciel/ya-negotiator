use actix::prelude::*;
use anyhow::{anyhow, bail};
use futures::future::{AbortHandle, Abortable};
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

#[derive(Debug)]
pub enum DecideReason {
    TimeElapsed,
    GoalReached,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub enum Feedback {
    Decide(DecideReason),
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

    collect_timeout_handle: Option<AbortHandle>,

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
            collect_period: Duration::from_secs(3600),
            collect_amount: 1,
            collect_timeout_handle: None,
            feedback_channel: feedback_sender,
            feedback_receiver: Some(feedback_receiver),
        }
    }

    /// Collects Proposals, that were already fully negotiated and score
    /// for them was computed.
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
                .send(Feedback::Decide(DecideReason::GoalReached))
                .map_err(|_| anyhow!("Feedback channel closed."))?;
        }

        Ok(())
    }

    /// Makes decision, which Proposals should be responded to.
    /// Rest of the Proposals is rejected and they are all placed in queue
    /// for future, in case not enough Agreements will be signed.
    pub fn decide(&mut self) -> anyhow::Result<()> {
        let goal = min(self.goal, self.awaiting.len());

        // Vector is sorted so the best elements are on the beginning.
        let accepted = self.awaiting.drain(0..goal).collect::<Vec<_>>();
        let rejected = self.awaiting.drain(..).collect::<Vec<_>>();

        // Since we will choose some Proposals, we must adjust how many we expect left.
        self.goal = self.goal - goal;

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

        // If decide call was called because of collect period timeout, we must
        // start waiting for new period. If we just reached expected number of
        // collected Proposals, we can spawn collect period anyway.
        self.spawn_collect_period();
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

    fn spawn_collect_period(&mut self) {
        // Cancel previous future notifying about collect period.
        if let Some(handle) = self.collect_timeout_handle.take() {
            handle.abort();
            self.collect_timeout_handle = None;
        }

        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        let timeout = self.collect_period.clone();
        let feedback = self.feedback_channel.clone();

        let future = async move {
            tokio::time::delay_for(timeout).await;
            feedback
                .send(Feedback::Decide(DecideReason::TimeElapsed))
                .ok();
        };

        tokio::spawn(Abortable::new(future, abort_registration));

        self.collect_timeout_handle = Some(abort_handle);
    }
}
