use actix::prelude::*;
use anyhow::{anyhow, bail};
use derive_more::Display;
use futures::future::{AbortHandle, Abortable};
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::component::ProposalView;

use ya_negotiator_component::reason::RejectReason;

#[derive(Debug)]
pub struct ProposalScore {
    pub their: ProposalView,
    pub our: ProposalView,
    pub score: f64,
}

#[derive(Debug)]
pub enum DecideReason {
    TimeElapsed,
    GoalReached,
}

/// Decision making mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecideGoal {
    /// ProposalsCollection is expected to provide limited number of Proposals.
    /// After goal is reached, no new Proposals will be chosen. Someone must
    /// set new goal to get new Proposals.
    Limit(usize),
    /// ProposalsCollection provides batches of certain size. Chosen Proposals'
    /// count isn't subtracted, so there is no limit of them.
    Batch(usize),
}

#[derive(Debug, Copy, Clone, Display)]
pub enum CollectionType {
    Agreement,
    Proposal,
}

#[derive(Debug)]
pub enum FeedbackAction {
    Decide(DecideReason),
    Accept {
        id: String,
    },
    Reject {
        id: String,
        reason: RejectReason,
        is_final: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConfig {
    /// Time period before making decision, which Proposals to choose.
    #[serde(with = "humantime_serde")]
    pub collect_period: Option<Duration>,
    /// Number of Proposals to collect, after which best of them will be accepted.
    pub collect_amount: Option<usize>,
    /// Expected number of Proposals to choose or batch size. See DecideGoal description.
    pub goal: DecideGoal,
}

#[derive(Message, Debug)]
#[rtype(result = "()")]
pub struct Feedback {
    pub action: FeedbackAction,
    pub collection_type: CollectionType,
}

/// Stores Proposals together with their Score. Triggers decision based on
/// number of collected Offers or time that elapsed.
pub struct ProposalsCollection {
    awaiting: Vec<ProposalScore>,
    /// Proposals that were rejected by us, but have still chance for being chosen
    /// when conditions will change. This list doesn't include Proposals rejected
    /// with `final` flag.
    rejected: Vec<ProposalScore>,

    /// Expected number of Proposals to choose or batch size. See DecideGoal description.
    goal: DecideGoal,

    /// Time period before making decision, which Proposals to choose.
    collect_period: Duration,
    /// Number of Proposals to collect, after which best of them will be accepted.
    collect_amount: usize,

    collect_timeout_handle: Option<AbortHandle>,

    /// This collection handles Agreements or Proposals.
    collection_type: CollectionType,

    feedback_channel: mpsc::UnboundedSender<Feedback>,
    pub feedback_receiver: Option<mpsc::UnboundedReceiver<Feedback>>,
}

impl ProposalsCollection {
    pub fn new(collection_type: CollectionType, config: CollectionConfig) -> ProposalsCollection {
        let (feedback_sender, feedback_receiver) = mpsc::unbounded_channel();

        let mut collection = ProposalsCollection {
            awaiting: vec![],
            rejected: vec![],
            collect_period: config.collect_period.unwrap_or(Duration::MAX),
            collect_amount: config.collect_amount.unwrap_or(usize::MAX),
            collect_timeout_handle: None,
            feedback_channel: feedback_sender,
            feedback_receiver: Some(feedback_receiver),
            collection_type,
            goal: config.goal,
        };

        collection.spawn_collect_period();
        collection
    }

    pub fn set_goal(&mut self, goal: DecideGoal) {
        match self.goal {
            DecideGoal::Limit(current) => match goal {
                DecideGoal::Limit(num_requested) => {
                    self.goal = DecideGoal::Limit(num_requested + current)
                }
                DecideGoal::Batch(_) => self.goal = goal,
            },
            DecideGoal::Batch(_) => {
                self.goal = goal;
            }
        }
    }

    /// Collects Proposals, that were already fully negotiated and score
    /// for them was computed.
    /// Note: id is dirty hack to display Agreement id instead of Proposal id here.
    /// ProposalViews don't contain Agreement id.
    pub fn new_scored(&mut self, new: ProposalScore, id: &str) -> anyhow::Result<()> {
        log::info!("Adding {} [{}] to choose later.", self.collection_type, id);

        if new.score.is_nan() {
            bail!("{} [{}] score was set to NaN.", self.collection_type, id);
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
            self.send_feedback(FeedbackAction::Decide(DecideReason::GoalReached))?;
        }

        Ok(())
    }

    /// Makes decision, which Proposals should be responded to.
    /// Rest of the Proposals is rejected and they are all placed in queue
    /// for future, in case not enough Agreements will be signed.
    pub fn decide(&mut self) -> anyhow::Result<()> {
        let goal = match self.goal {
            DecideGoal::Limit(expected_goal) => {
                let goal = min(expected_goal, self.awaiting.len());
                self.goal = DecideGoal::Limit(expected_goal - goal);
                goal
            }
            DecideGoal::Batch(batch_size) => min(batch_size, self.awaiting.len()),
        };

        log::debug!(
            "Deciding which {}(s) to choose. Expected count: {}",
            self.collection_type,
            goal
        );

        // Vector is sorted so the best elements are on the beginning.
        let accepted = self.awaiting.drain(0..goal).collect::<Vec<_>>();
        let rejected = self.awaiting.drain(..).collect::<Vec<_>>();

        if goal != 0 {
            log::info!("Decided to accept {} {}(s).", goal, self.collection_type);
        }

        for proposal in accepted {
            self.send_feedback(FeedbackAction::Accept {
                id: proposal.their.id,
            })
            .ok();
        }

        for proposal in rejected {
            self.send_feedback(FeedbackAction::Reject {
                id: proposal.their.id.clone(),
                reason: RejectReason::new("Node is busy.").into(),
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
            bail!(
                "{} [{}] score was set to NaN.",
                self.collection_type,
                new.their.id
            );
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
        let collection_type = self.collection_type;

        let future = async move {
            tokio::time::sleep(timeout).await;
            feedback
                .send(Feedback {
                    action: FeedbackAction::Decide(DecideReason::TimeElapsed),
                    collection_type,
                })
                .ok();
        };

        tokio::spawn(Abortable::new(future, abort_registration));

        self.collect_timeout_handle = Some(abort_handle);
    }

    fn send_feedback(&self, action: FeedbackAction) -> anyhow::Result<()> {
        Ok(self
            .feedback_channel
            .send(Feedback {
                action,
                collection_type: self.collection_type,
            })
            .map_err(|_| anyhow!("Feedback channel closed."))?)
    }
}
