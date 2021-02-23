use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementAction, AgreementResult, ProposalAction};

use ya_client_model::market::proposal::State;
use ya_client_model::market::{Agreement, DemandOfferBase, NewProposal, Proposal, Reason};
use ya_client_model::NodeId;

use crate::node::{Node, NodeType};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NegotiationStage {
    Initial,
    Proposal { last_response: ProposalAction },
    Agreement { last_response: AgreementAction },
    Error(String),
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
pub struct NegotiationRecordImpl {
    pub results: HashMap<NodePair, NegotiationResult>,
    pub proposals: HashMap<String, Proposal>,
    pub agreements: HashMap<String, Agreement>,
}

#[derive(Clone, Debug)]
pub struct NegotiationRecord(Arc<Mutex<NegotiationRecordImpl>>);

impl NegotiationRecord {
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

    pub fn accept(&self, counter_proposal: Proposal, with_node: NodeId) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .get_mut(&NodePair(counter_proposal.issuer_id, with_node))
            .unwrap();

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::AcceptProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
            },
        });

        record
            .proposals
            .insert(counter_proposal.proposal_id.clone(), counter_proposal);
    }

    pub fn counter(&self, counter_proposal: Proposal, with_node: NodeId) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .get_mut(&NodePair(counter_proposal.issuer_id, with_node))
            .unwrap();

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::CounterProposal {
                id: counter_proposal.clone().prev_proposal_id.unwrap(),
                proposal: NewProposal {
                    properties: counter_proposal.properties.clone(),
                    constraints: counter_proposal.constraints.clone(),
                },
            },
        });

        record
            .proposals
            .insert(counter_proposal.proposal_id.clone(), counter_proposal);
    }

    pub fn reject(&self, owner_node: NodeId, rejected_proposal: Proposal, reason: Option<Reason>) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .get_mut(&NodePair(owner_node, rejected_proposal.issuer_id))
            .unwrap();

        negotiation.stage.push(NegotiationStage::Proposal {
            last_response: ProposalAction::RejectProposal {
                id: rejected_proposal.prev_proposal_id.unwrap(),
                reason,
            },
        });
    }

    pub fn approve(&self, owner_node: NodeId, with_node: NodeId, agreement: Agreement) {
        let mut record = self.0.lock().unwrap();
        let negotiation = record
            .results
            .get_mut(&NodePair(owner_node, with_node))
            .unwrap();

        negotiation.stage.push(NegotiationStage::Agreement {
            last_response: AgreementAction::ApproveAgreement {
                id: agreement.agreement_id.clone(),
            },
        });

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
        write!(
            f,
            "{}",
            serde_json::to_string_pretty(&(*self.0.lock().unwrap())).unwrap()
        )
    }
}
