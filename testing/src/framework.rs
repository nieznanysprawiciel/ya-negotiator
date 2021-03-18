use ya_agreement_utils::{AgreementView, OfferTemplate};
use ya_negotiators::factory::*;
use ya_negotiators::AgreementResult;

use ya_client_model::market::Proposal;
use ya_client_model::NodeId;

use crate::negotiation_record::{NegotiationRecord, NegotiationRecordSync};
use crate::node::{Node, NodeType};
use crate::provider::{provider_agreements_processor, provider_proposals_processor};
use crate::requestor::{requestor_agreements_processor, requestor_proposals_processor};

use crate::prepare_test_dir;
use anyhow::anyhow;
use futures::future::select_all;
use futures::{Future, FutureExt};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::timeout;

#[derive(thiserror::Error)]
#[error("{error}\nNegotiation traceback:\n\n{negotiation_traceback}")]
pub struct FrameworkError {
    error: anyhow::Error,
    negotiation_traceback: NegotiationRecord,
}

/// Emulates running negotiations between Requestor and Provider.
/// TODO: Support for multiple Provider/Requestor Negotiators at the same time.
pub struct Framework {
    pub requestors: HashMap<NodeId, Arc<Node>>,
    pub providers: HashMap<NodeId, Arc<Node>>,

    pub test_dir: PathBuf,
}

impl Framework {
    pub fn new_empty(test_name: &str) -> anyhow::Result<Framework> {
        Ok(Framework {
            requestors: HashMap::new(),
            providers: HashMap::new(),
            test_dir: prepare_test_dir(test_name)?,
        })
    }

    pub fn new(
        test_name: &str,
        prov_config: NegotiatorsConfig,
        req_config: NegotiatorsConfig,
    ) -> anyhow::Result<Framework> {
        let framework = Self::new_empty(test_name)?
            .add_provider(prov_config)?
            .add_requestor(req_config)?;

        Ok(framework)
    }

    pub fn add_provider(mut self, config: NegotiatorsConfig) -> anyhow::Result<Self> {
        let node = Node::new(config, NodeType::Provider, None, self.test_dir.clone())?;
        self.providers.insert(node.node_id, node);
        Ok(self)
    }

    pub fn add_requestor(mut self, config: NegotiatorsConfig) -> anyhow::Result<Self> {
        let node = Node::new(config, NodeType::Requestor, None, self.test_dir.clone())?;
        self.requestors.insert(node.node_id, node);
        Ok(self)
    }

    pub fn add_named_provider(
        mut self,
        config: NegotiatorsConfig,
        name: &str,
    ) -> anyhow::Result<Self> {
        let node = Node::new(
            config,
            NodeType::Provider,
            Some(name.to_string()),
            self.test_dir.clone(),
        )?;
        self.providers.insert(node.node_id, node);
        Ok(self)
    }

    pub fn add_named_requestor(
        mut self,
        config: NegotiatorsConfig,
        name: &str,
    ) -> anyhow::Result<Self> {
        let node = Node::new(
            config,
            NodeType::Requestor,
            Some(name.to_string()),
            self.test_dir.clone(),
        )?;
        self.requestors.insert(node.node_id, node);
        Ok(self)
    }

    pub async fn run_for_templates(
        &self,
        demand: OfferTemplate,
        offer: OfferTemplate,
    ) -> Result<NegotiationRecord, FrameworkError> {
        let record = NegotiationRecordSync::new(30);

        let mut offers = vec![];
        for (_, provider) in &self.providers {
            offers.push(
                provider
                    .create_offer(&offer)
                    .await
                    .map_err(|e| FrameworkError::from(e, &record))?,
            )
        }

        let mut demands = vec![];
        for (_, requestor) in &self.requestors {
            demands.push(
                requestor
                    .create_offer(&demand)
                    .await
                    .map_err(|e| FrameworkError::from(e, &record))?,
            )
        }

        let processors_handle = self.spawn_processors(record.clone(), Duration::from_secs(10));
        self.init_for(offers, demands, record.clone()).await;

        processors_handle
            .await
            .map_err(|e| FrameworkError::from(e, &record))?;

        let record = record.0.lock().unwrap();
        Ok(record.clone())
    }

    // Will start negotiations for all pairs of Offer/Demand.
    pub async fn init_for(
        &self,
        offers: Vec<Proposal>,
        demands: Vec<Proposal>,
        record: NegotiationRecordSync,
    ) {
        for demand in demands {
            // Each Offer Proposal generated for Requestor will have this single
            // Proposal set as `prev_proposal_id`
            record.add_proposal(demand.clone());

            for offer in &offers {
                //TODO: We should do Offer/Demand matching here.
                let requestor = self.requestors.get(&demand.issuer_id).unwrap();
                let mut p_proposal = offer.clone();
                p_proposal.prev_proposal_id = Some(demand.proposal_id.clone());

                record.add_proposal(p_proposal.clone());

                if let Err(e) = requestor.react_to_proposal(&p_proposal, &demand).await {
                    record.error(requestor.node_id, offer.issuer_id, e.into());
                }
            }
        }
    }

    fn spawn_processors(&self, record: NegotiationRecordSync, run_for: Duration) -> JoinHandle<()> {
        tokio::spawn(
            select_all(vec![
                timeout(
                    run_for,
                    provider_proposals_processor(
                        self.providers.clone(),
                        self.requestors.clone(),
                        record.clone(),
                    ),
                )
                .boxed(),
                timeout(
                    run_for,
                    provider_agreements_processor(
                        self.providers.clone(),
                        self.requestors.clone(),
                        record.clone(),
                    ),
                )
                .boxed(),
                timeout(
                    run_for,
                    requestor_proposals_processor(
                        self.providers.clone(),
                        self.requestors.clone(),
                        record.clone(),
                    ),
                )
                .boxed(),
                timeout(
                    run_for,
                    requestor_agreements_processor(
                        self.providers.clone(),
                        self.requestors.clone(),
                        record.clone(),
                    ),
                )
                .boxed(),
            ])
            .map(|_| ()),
        )
    }

    pub async fn run_finalize_agreements(
        &self,
        to_finalize: Vec<(&AgreementView, AgreementResult)>,
    ) -> Vec<anyhow::Result<()>> {
        let mut results = vec![];
        for agreement in to_finalize {
            results.push(self.finalize_agreement(agreement.0, agreement.1).await);
        }
        results
    }

    pub async fn finalize_agreement(
        &self,
        agreement: &AgreementView,
        result: AgreementResult,
    ) -> anyhow::Result<()> {
        let requestor = self
            .requestors
            .get(&agreement.requestor_id()?)
            .ok_or(anyhow!("No Requestor"))?;
        let provider = self
            .providers
            .get(&agreement.provider_id()?)
            .ok_or(anyhow!("No Provider"))?;

        // First call both functions and resolve errors later. We don't want
        // to omit any of these calls.
        let prov_result = requestor
            .agreement_finalized(&agreement.id, result.clone())
            .await;
        let req_result = provider.agreement_finalized(&agreement.id, result).await;

        prov_result?;
        req_result?;
        Ok(())
    }
}

trait NegotiationResponseProcessor: Future<Output = ()> + Sized + 'static {}

impl FrameworkError {
    pub fn from(error: impl Into<anyhow::Error>, result: &NegotiationRecordSync) -> FrameworkError {
        FrameworkError {
            error: error.into(),
            negotiation_traceback: result.0.lock().unwrap().clone(),
        }
    }
}

impl fmt::Debug for FrameworkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self)
    }
}
