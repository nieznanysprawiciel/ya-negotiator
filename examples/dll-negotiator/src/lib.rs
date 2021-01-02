use ya_negotiator_shared_lib_interface::plugin::{
    AgreementResult, NegotiationResult, NegotiatorComponent, OfferTemplate, ProposalView, Reason,
};

pub struct FilterNodes {
    names: Vec<String>,
}

impl NegotiatorComponent for FilterNodes {
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
    ) -> anyhow::Result<NegotiationResult> {
        Ok(match demand.pointer_typed("/golem/node/id/name") {
            Ok(node_name) => {
                if self.names.contains(&node_name) {
                    NegotiationResult::Reject {
                        reason: Some(Reason::new("Node on rejection list.")),
                    }
                } else {
                    NegotiationResult::Ready { offer }
                }
            }
            Err(_) => NegotiationResult::Reject {
                reason: Some(Reason::new("Unnamed Node")),
            },
        })
    }

    fn fill_template(&mut self, offer_template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(offer_template)
    }

    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_agreement_approved(&mut self, _agreement_id: &str) -> anyhow::Result<()> {
        Ok(())
    }
}
