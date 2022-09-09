use anyhow::anyhow;
use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};

use crate::component::{
    AgreementEvent, AgreementResult, NegotiationResult, NegotiatorComponent, Score,
};

/// Processes multiple negotiators.
#[derive(Clone)]
pub struct NegotiatorsChain {
    inner: Arc<RwLock<NegotiatorsChainImpl>>,
}

#[derive(Default)]
struct NegotiatorsChainImpl {
    /// Ordered components. Negotiation calls execution order matters.
    components: Vec<(String, Arc<Box<dyn NegotiatorComponent>>)>,
    /// Named lookup.
    names: HashMap<String, Arc<Box<dyn NegotiatorComponent>>>,
}

impl NegotiatorsChainImpl {
    pub fn add_component(&mut self, mut name: String, component: Box<dyn NegotiatorComponent>) {
        // Unwrap should be caught by tests. This way we avoid returning result and complicating code.
        let re = Regex::new(r"#(?P<idx>[0-9]+)\z").unwrap();

        while let Some(_) = self.names.get(&name) {
            if let Some(idx) = re
                .captures(&name)
                .and_then(|caps| caps.name("idx"))
                .and_then(|capture| capture.as_str().parse::<u32>().map(|idx| idx + 1).ok())
            {
                name = re.replace(&name, format!("#{idx}")).to_string()
            } else {
                name = format!("{name}#1");
            }
        }

        let component = Arc::new(component);

        self.components.push((name.clone(), component.clone()));
        self.names.insert(name, component);
    }

    pub fn list(&self) -> Vec<String> {
        self.components
            .iter()
            .map(|(name, _)| name)
            .cloned()
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<Arc<Box<dyn NegotiatorComponent>>> {
        self.names.get(name).cloned()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, Arc<Box<dyn NegotiatorComponent>>)> {
        self.components
            .iter()
            .map(|(name, component)| (name.as_str(), component.clone()))
    }
}

impl NegotiatorsChain {
    pub fn new() -> NegotiatorsChain {
        NegotiatorsChain {
            inner: Arc::new(RwLock::new(NegotiatorsChainImpl::default())),
        }
    }

    pub fn with(components: Vec<(String, Box<dyn NegotiatorComponent>)>) -> NegotiatorsChain {
        let mut inner = NegotiatorsChainImpl::default();
        for (name, component) in components {
            inner.add_component(name, component)
        }

        NegotiatorsChain {
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// Function will rename component, if the name was already used.
    /// Function adds subsequent numbers to string for example:
    /// from `NegotiatorsChain` it will make `NegotiatorsChain#1` and than `NegotiatorsChain#2`.
    pub async fn add_component(
        self,
        name: &str,
        component: Box<dyn NegotiatorComponent>,
    ) -> NegotiatorsChain {
        self.inner
            .write()
            .await
            .add_component(name.to_string(), component);
        self
    }

    pub async fn list_components(&self) -> Vec<String> {
        self.inner.read().await.list()
    }
}

#[async_trait(?Send)]
impl NegotiatorComponent for NegotiatorsChain {
    async fn negotiate_step(
        &self,
        incoming_proposal: &ProposalView,
        mut template: ProposalView,
        mut score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let mut all_ready = true;
        for (name, component) in self.inner.read().await.iter() {
            let result = component
                .negotiate_step(incoming_proposal, template, score)
                .await?;
            match result {
                NegotiationResult::Ready {
                    proposal: offer,
                    score: new_score,
                } => {
                    template = offer;
                    score = new_score;
                }
                NegotiationResult::Negotiating {
                    proposal: offer,
                    score: new_score,
                } => {
                    log::info!(
                        "Negotiator component '{}' is still negotiating Proposal [{}].",
                        name,
                        incoming_proposal.id
                    );

                    all_ready = false;
                    template = offer;
                    score = new_score;
                }
                NegotiationResult::Reject { reason, is_final } => {
                    return Ok(NegotiationResult::Reject { reason, is_final })
                }
            }
        }

        // Full negotiations is ready only, if all `NegotiatorComponent` returned
        // ready state. Otherwise we must still continue negotiations.
        Ok(match all_ready {
            true => NegotiationResult::Ready {
                proposal: template,
                score,
            },
            false => NegotiationResult::Negotiating {
                proposal: template,
                score,
            },
        })
    }

    async fn fill_template(
        &self,
        mut offer_template: OfferTemplate,
    ) -> anyhow::Result<OfferTemplate> {
        for (name, component) in self.inner.read().await.iter() {
            offer_template = component.fill_template(offer_template).await.map_err(|e| {
                anyhow!("Negotiator component '{name}' failed filling Offer template. {e}")
            })?;
        }
        Ok(offer_template)
    }

    async fn on_agreement_terminated(
        &self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        for (name, component) in self.inner.read().await.iter() {
            component
                .on_agreement_terminated(agreement_id, result).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Agreement [{agreement_id}] termination. {e}"
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_agreement_approved(&self, agreement: &AgreementView) -> anyhow::Result<()> {
        for (name, component) in self.inner.read().await.iter() {
            component
                .on_agreement_approved(agreement).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Agreement [{}] approval. {e}",
                        agreement.id,
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_proposal_rejected(&self, proposal_id: &str) -> anyhow::Result<()> {
        for (name, component) in self.inner.read().await.iter() {
            component
                .on_proposal_rejected(proposal_id).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling Proposal [{proposal_id}] rejection. {e}",
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn on_agreement_event(
        &self,
        agreement_id: &str,
        event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        for (name, component) in self.inner.read().await.iter() {
            component
                .on_agreement_event(agreement_id, event).await
                .map_err(|e| {
                    log::warn!(
                        "Negotiator component '{name}' failed handling post Terminate event [{agreement_id}]. {e}",
                    )
                })
                .ok();
        }
        Ok(())
    }

    async fn control_event(
        &self,
        component: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        match self.inner.read().await.get(component) {
            None => Ok(serde_json::Value::Null),
            Some(negotiator) => negotiator.control_event(component, params).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use test_case::test_case;

    use super::*;
    use crate::static_lib::factory;
    use crate::{NegotiatorAsync, NegotiatorFactoryDefault};

    #[derive(Default)]
    pub struct ExampleNegotiator {}

    impl NegotiatorComponent for ExampleNegotiator {}

    impl NegotiatorFactoryDefault<ExampleNegotiator> for ExampleNegotiator {
        type Type = NegotiatorAsync;
    }

    fn create_test_negotiator() -> Box<dyn NegotiatorComponent> {
        factory::<ExampleNegotiator>()("ExampleNegotiator", serde_yaml::Value::Null, PathBuf::new())
            .unwrap()
    }

    #[test_case(
        &["ExampleNegotiator"],
        &["ExampleNegotiator"];
        "First element's name shouldn't change"
    )]
    #[test_case(
        &["ExampleNegotiator", "ExampleNegotiator"],
        &["ExampleNegotiator", "ExampleNegotiator#1"];
        "Second element should get #1 postfix"
    )]
    #[test_case(
        &["ExampleNegotiator", "ExampleNegotiator", "ExampleNegotiator"],
        &["ExampleNegotiator", "ExampleNegotiator#1", "ExampleNegotiator#2"];
        "Third element should get #2 postfix"
    )]
    #[test_case(
        &["ExampleNegotiator", "ExampleNegotiator", "ExampleNegotiator", "ExampleNegotiator", "ExampleNegotiator"],
        &["ExampleNegotiator", "ExampleNegotiator#1", "ExampleNegotiator#2", "ExampleNegotiator#3", "ExampleNegotiator#4"];
        "Check postfix for 5 elements to be sure"
    )]
    #[test_case(
        &["ExampleNegotiator#1", "ExampleNegotiator"],
        &["ExampleNegotiator#1", "ExampleNegotiator"];
        "First element already with postfix"
    )]
    #[test_case(
        &["ExampleNegotiator#2", "ExampleNegotiator", "ExampleNegotiator"],
        &["ExampleNegotiator#2", "ExampleNegotiator", "ExampleNegotiator#1"];
        "Postfix #2 on first position"
    )]
    #[test_case(
        &["ExampleNegotiator#2", "ExampleNegotiator#1", "ExampleNegotiator#3"],
        &["ExampleNegotiator#2", "ExampleNegotiator#1", "ExampleNegotiator#3"];
        "Keep postfixes in order if they exist"
    )]
    #[test_case(
        &["ExampleNegotiator#", "ExampleNegotiator"],
        &["ExampleNegotiator#", "ExampleNegotiator"];
        "Tricky name postfix"
    )]
    #[tokio::test]
    async fn test_negotiators_chain_add_elements(names: &[&str], assert_names: &[&str]) {
        let negotiators = (0..names.len())
            .into_iter()
            .map(|i| (names[i].to_string(), create_test_negotiator()))
            .collect();

        let chain = NegotiatorsChain::with(negotiators);
        let components = chain.list_components().await;

        for i in 0..names.len() {
            assert_eq!(components[i], assert_names[i]);
        }
    }
}
