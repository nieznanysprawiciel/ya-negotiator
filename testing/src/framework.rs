use ya_agreement_utils::OfferTemplate;
use ya_negotiators::factory::*;
use ya_negotiators::{AgreementResponse, ProposalResponse};

use ya_client_model::market::Proposal;

use crate::node::{Node, NodeType};

pub enum NegotiationStage {
    Initial,
    Proposal { last_response: ProposalResponse },
    Agreement { last_response: AgreementResponse },
}

pub struct NegotiationResult {
    stage: NegotiationStage,
    proposals: Vec<Proposal>,
}

/// Emulates running negotiations between Requestor and Provider.
/// TODO: Support for multiple Provider/Requestor Negotiators at the same time.
pub struct Framework {
    pub requestor: Node,
    pub provider: Node,
}

impl Framework {
    pub fn new(
        prov_config: NegotiatorsConfig,
        req_config: NegotiatorsConfig,
    ) -> anyhow::Result<Framework> {
        Ok(Framework {
            requestor: Node::new(req_config, NodeType::Requestor)?,
            provider: Node::new(prov_config, NodeType::Provider)?,
        })
    }

    pub async fn run_for_templates(
        &self,
        demand: OfferTemplate,
        offer: OfferTemplate,
    ) -> anyhow::Result<NegotiationResult> {
        let offer = self.provider.create_offer(&offer).await?;
        let demand = self.requestor.create_offer(&demand).await?;

        self.run_for_offers(demand, offer).await
    }

    /// Negotiators should have offers already created. This functions emulates
    /// negotiations, when negotiators continue negotiations with new Nodes, without
    /// resubscribing Offers/Demands.
    pub async fn run_for_offers(
        &self,
        demand: Proposal,
        offer: Proposal,
    ) -> anyhow::Result<NegotiationResult> {
        let mut result = NegotiationResult {
            proposals: vec![offer.clone(), demand.clone()],
            stage: NegotiationStage::Initial,
        };

        // loop {
        //     //self.requestor.react_to_proposal()
        // }
        Ok(result)
    }
}
