use ya_agreement_utils::AgreementView;
use ya_negotiators::{AgreementAction, ProposalAction};

use ya_client_model::market::{Agreement, NewProposal, Proposal, Reason};
use ya_client_model::NodeId;

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NegotiationStage {
    Proposal { last_response: ProposalAction },
    Agreement { last_response: AgreementAction },
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

#[derive(Hash, Clone, Debug, Serialize, Deserialize)]
pub struct NodePair(NodeId, NodeId);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NegotiationRecord {
    pub results: HashMap<NodePair, NegotiationResult>,
    pub proposals: HashMap<String, Proposal>,
    pub agreements: HashMap<String, Agreement>,

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
        let negotiation = record
            .results
            .entry(NodePair(counter_proposal.issuer_id, with_node))
            .or_insert(NegotiationResult::new());

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::AcceptProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
            },
        });

        negotiation.proposals.push(counter_proposal.clone());
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

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::CounterProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
                proposal: NewProposal {
                    properties: counter_proposal.properties.clone(),
                    constraints: counter_proposal.constraints.clone(),
                },
            },
        });

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

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::RejectProposal {
                id: rejected_proposal.prev_proposal_id.unwrap(),
                reason,
            },
        });
    }

    pub fn approve(&self, agreement: Agreement) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .entry(NodePair(
                agreement.requestor_id().clone(),
                agreement.provider_id().clone(),
            ))
            .or_insert(NegotiationResult::new());

        negotiation.stage.push(NegotiationStage::Agreement {
            last_response: AgreementAction::ApproveAgreement {
                id: agreement.agreement_id.clone(),
            },
        });

        negotiation.agreement = Some(AgreementView::try_from(&agreement).unwrap());
        record
            .agreements
            .insert(agreement.agreement_id.clone(), agreement);
    }

    pub fn reject_agreement(&self, agreement: Agreement, reason: Option<Reason>) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .entry(NodePair(
                agreement.requestor_id().clone(),
                agreement.provider_id().clone(),
            ))
            .or_insert(NegotiationResult::new());

        negotiation.stage.push(NegotiationStage::Agreement {
            last_response: AgreementAction::RejectAgreement {
                id: agreement.agreement_id.clone(),
                reason,
            },
        });
    }

    pub fn propose_agreement(&self, agreement: Agreement) {
        let mut record = self.0.lock().unwrap();
        record
            .agreements
            .insert(agreement.agreement_id.clone(), agreement);
    }

    pub fn get_proposal(&self, id: &String) -> Option<Proposal> {
        self.0.lock().unwrap().proposals.get(id).cloned()
    }

    pub fn get_agreement(&self, id: &String) -> Option<Agreement> {
        self.0.lock().unwrap().agreements.get(id).cloned()
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

impl NegotiationResult {
    pub fn is_finished(&self) -> bool {
        if self.agreement.is_some() {
            return true;
        }

        match self.stage.last() {
            Some(stage) => match stage {
                NegotiationStage::Proposal {
                    last_response: ProposalAction::RejectProposal { .. },
                } => true,
                NegotiationStage::Agreement {
                    last_response: AgreementAction::RejectAgreement { .. },
                } => true,
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
