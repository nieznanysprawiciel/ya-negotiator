use ya_agreement_utils::AgreementView;
use ya_negotiators::{Action, AgreementAction};

use ya_client_model::market::{NewProposal, Proposal, Reason};
use ya_client_model::NodeId;

use crate::error::NegotiatorError;

use backtrace::Backtrace;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NegotiationStage {
    Proposal(Action),
    Agreement(AgreementAction),
    Error(String),
    InfiniteLoop,
    Timeout,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiationResult {
    pub stage: Vec<NegotiationStage>,
    pub proposals: Vec<Proposal>,
    pub agreement: Option<AgreementView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, derive_more::Display)]
#[display(fmt = "{}-{}", _0, _1)]
pub struct NodePair(NodeId, NodeId);

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiationRecord {
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub results: HashMap<NodePair, NegotiationResult>,
    pub proposals: HashMap<String, Proposal>,
    pub agreements: HashMap<String, AgreementView>,

    pub errors: HashMap<NodeId, Vec<String>>,

    max_steps: usize,
}

#[derive(Clone, Debug)]
pub struct NegotiationRecordSync(pub Arc<Mutex<NegotiationRecord>>);

impl NegotiationRecordSync {
    pub fn new(max_steps: usize) -> NegotiationRecordSync {
        NegotiationRecordSync(Arc::new(Mutex::new(NegotiationRecord {
            results: Default::default(),
            proposals: Default::default(),
            agreements: Default::default(),
            errors: Default::default(),
            max_steps,
        })))
    }

    /// Error between Provider and Requestor.
    pub fn error(&self, owner_node: NodeId, with_node: NodeId, e: anyhow::Error) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .get_mut(&NodePair(owner_node, with_node))
            .unwrap();

        negotiation
            .stage
            .push(NegotiationStage::Error(e.to_string()));
    }

    /// Node error, that cannot be assinged to any negotiations pair.
    pub fn node_error(&self, owner_node: NodeId, e: anyhow::Error) {
        let mut record = self.0.lock().unwrap();
        record
            .errors
            .entry(owner_node)
            .or_insert(vec![])
            .push(e.to_string())
    }

    pub fn accept(&self, counter_proposal: Proposal, with_node: NodeId) {
        let mut record = self.0.lock().unwrap();
        let max_steps = record.max_steps;

        let negotiation = record
            .results
            .entry(NodePair(counter_proposal.issuer_id, with_node))
            .or_insert(NegotiationResult::new());

        negotiation
            .stage
            .push(NegotiationStage::Proposal(Action::AcceptProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
            }));

        negotiation.proposals.push(counter_proposal.clone());

        if negotiation.proposals.len() > max_steps {
            negotiation.stage.push(NegotiationStage::InfiniteLoop);
        }

        record
            .proposals
            .insert(counter_proposal.proposal_id.clone(), counter_proposal);
    }

    pub fn counter(&self, counter_proposal: Proposal, with_node: NodeId) {
        let mut record = self.0.lock().unwrap();
        let max_steps = record.max_steps;

        let negotiation = record
            .results
            .entry(NodePair(counter_proposal.issuer_id, with_node))
            .or_insert(NegotiationResult::new());

        negotiation
            .stage
            .push(NegotiationStage::Proposal(Action::CounterProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
                proposal: NewProposal {
                    properties: counter_proposal.properties.clone(),
                    constraints: counter_proposal.constraints.clone(),
                },
            }));

        negotiation.proposals.push(counter_proposal.clone());

        if negotiation.proposals.len() > max_steps {
            negotiation.stage.push(NegotiationStage::InfiniteLoop);
        }

        record
            .proposals
            .insert(counter_proposal.proposal_id.clone(), counter_proposal);
    }

    pub fn reject(&self, owner_node: NodeId, rejected_proposal: Proposal, reason: Option<Reason>) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .entry(NodePair(owner_node, rejected_proposal.issuer_id))
            .or_insert(NegotiationResult::new());

        negotiation
            .stage
            .push(NegotiationStage::Proposal(Action::RejectProposal {
                id: rejected_proposal.prev_proposal_id.unwrap(),
                reason,
            }));
    }

    pub fn approve(&self, agreement: AgreementView) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .entry(NodePair(
                agreement.requestor_id().unwrap().clone(),
                agreement.provider_id().unwrap().clone(),
            ))
            .or_insert(NegotiationResult::new());

        negotiation.stage.push(NegotiationStage::Agreement(
            AgreementAction::ApproveAgreement {
                id: agreement.id.clone(),
            },
        ));

        negotiation.agreement = Some(agreement.clone());
        record.agreements.insert(agreement.id.clone(), agreement);
    }

    pub fn reject_agreement(&self, agreement: AgreementView, reason: Option<Reason>) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .entry(NodePair(
                agreement.requestor_id().unwrap().clone(),
                agreement.provider_id().unwrap().clone(),
            ))
            .or_insert(NegotiationResult::new());

        negotiation.stage.push(NegotiationStage::Agreement(
            AgreementAction::RejectAgreement {
                id: agreement.id.clone(),
                reason,
            },
        ));
    }

    pub fn propose_agreement(&self, agreement: AgreementView) {
        let mut record = self.0.lock().unwrap();
        record.agreements.insert(agreement.id.clone(), agreement);
    }

    pub fn get_proposal(&self, id: &String) -> Result<Proposal, NegotiatorError> {
        self.0.lock().unwrap().get_proposal(id)
    }

    pub fn get_agreement(&self, id: &String) -> Result<AgreementView, NegotiatorError> {
        self.0.lock().unwrap().get_agreement(id)
    }

    pub fn add_proposal(&self, proposal: Proposal) {
        self.0
            .lock()
            .unwrap()
            .proposals
            .insert(proposal.proposal_id.clone(), proposal);
    }

    pub fn is_finished(&self) -> bool {
        let record = self.0.lock().unwrap();
        record
            .results
            .iter()
            .all(|(_, result)| result.is_finished())
    }
}

impl NegotiationRecord {
    pub fn get_proposal(&self, id: &String) -> Result<Proposal, NegotiatorError> {
        self.proposals
            .get(id)
            .cloned()
            .ok_or(NegotiatorError::ProposalNotFound {
                id: id.to_string(),
                trace: format!("{:?}", Backtrace::new()),
            })
    }

    pub fn get_agreement(&self, id: &String) -> Result<AgreementView, NegotiatorError> {
        self.agreements
            .get(id)
            .cloned()
            .ok_or(NegotiatorError::AgreementNotFound {
                id: id.to_string(),
                trace: format!("{:?}", Backtrace::new()),
            })
    }
}

impl NegotiationResult {
    pub fn is_finished(&self) -> bool {
        if self.agreement.is_some() {
            return true;
        }

        match self.stage.last() {
            Some(stage) => match stage {
                NegotiationStage::Proposal(Action::RejectProposal { .. }) => true,
                NegotiationStage::Agreement(AgreementAction::RejectAgreement { .. }) => true,
                NegotiationStage::Error(_) => true,
                NegotiationStage::InfiniteLoop => true,
                NegotiationStage::Timeout => true,
                _ => false,
            },
            None => false,
        }
    }
}

impl PartialOrd for NodePair {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let ord1 = self.clone().ordered();
        let ord2 = other.clone().ordered();

        match compare_ids(ord1.0, ord2.0) {
            Ordering::Less => Some(Ordering::Less),
            Ordering::Greater => Some(Ordering::Greater),
            Ordering::Equal => Some(compare_ids(ord1.1, ord2.1)),
        }
    }
}

impl PartialEq for NodePair {
    fn eq(&self, other: &Self) -> bool {
        let ord1 = self.clone().ordered();
        let ord2 = other.clone().ordered();

        ord1.0 == ord2.0 && ord1.1 == ord2.1
    }
}

impl Eq for NodePair {}

impl NodePair {
    pub fn ordered(self) -> NodePair {
        match self.0.to_string().cmp(&self.1.to_string()) {
            Ordering::Less => NodePair(self.0, self.1),
            Ordering::Greater => NodePair(self.1, self.0),
            Ordering::Equal => NodePair(self.0, self.1),
        }
    }
}

impl Hash for NodePair {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let pair = self.clone().ordered();
        pair.0.hash(state);
        pair.1.hash(state);
    }
}

impl FromStr for NodePair {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ids: Vec<&str> = s.split('-').collect();

        let id1 = ids[0].parse::<NodeId>()?;
        let id2 = ids[1].parse::<NodeId>()?;

        Ok(NodePair(id1, id2))
    }
}

fn compare_ids(id1: NodeId, id2: NodeId) -> Ordering {
    if id1.into_array() < id2.into_array() {
        Ordering::Less
    } else if id1.into_array() > id2.into_array() {
        return Ordering::Greater;
    } else {
        Ordering::Equal
    }
}

impl fmt::Display for NegotiationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string_pretty(&self).unwrap())
    }
}

impl fmt::Display for NegotiationRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_json::to_string_pretty(&self).unwrap())
    }
}

impl NegotiationResult {
    pub fn new() -> NegotiationResult {
        NegotiationResult {
            stage: vec![],
            proposals: vec![],
            agreement: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_node_pair_order() {
        let id1 = NodeId::from_str("0x33796f397a554a6c33675976683031774f637a37").unwrap();
        let id2 = NodeId::from_str("0x4c684d736d3157416a6e494145776833584b4339").unwrap();

        let pair1 = NodePair(id1.clone(), id2.clone());
        let pair2 = NodePair(id2.clone(), id1.clone());

        assert_eq!(pair1, pair2);
        assert_eq!(pair1.partial_cmp(&pair2).unwrap(), Ordering::Equal);

        let mut map = HashMap::<NodePair, Vec<String>>::new();
        map.entry(pair1).or_insert(vec![]).push("dupa1".to_string());
        map.entry(pair2).or_insert(vec![]).push("dupa2".to_string());

        assert_eq!(map.len(), 1);
    }
}
