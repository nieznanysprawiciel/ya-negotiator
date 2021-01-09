use serde::{Deserialize, Serialize};

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_client_model::market::Reason;

/// Result returned by `NegotiatorComponent` during Proposals evaluation.
#[derive(Serialize, Deserialize)]
pub enum NegotiationResult {
    /// `NegotiatorComponent` fully negotiated his part of Proposal,
    /// and it can be turned into valid Agreement. Provider will send
    /// counter Proposal.
    Ready { offer: ProposalView },
    /// Proposal is not ready to become Agreement, but negotiations
    /// are in progress.
    Negotiating { offer: ProposalView },
    /// Proposal is not acceptable and should be rejected.
    /// Negotiations can't be continued.
    Reject { reason: Option<Reason> },
}

/// Result of agreement execution.
#[derive(Clone, Serialize, Deserialize)]
pub enum AgreementResult {
    /// Failed to approve agreement. (Agreement even wasn't created)
    ApprovalFailed,
    /// Agreement was finished with success after first Activity.
    ClosedByProvider,
    /// Agreement was finished with success by Requestor.
    ClosedByRequestor,
    /// Agreement was broken by us.
    Broken { reason: Option<Reason> },
}

/// `NegotiatorComponent` implements negotiation logic for part of Agreement
/// specification. Components should be as granular as possible to allow composition
/// with other Components.
///
/// Future goal is to allow developers to create their own specifications and implement
/// components, that are able to negotiate this specification.
/// It would be useful to have `NegotiatorComponents`, that can be loaded from shared library
/// or can communicate with negotiation logic in external process (maybe RPC or TCP??).
pub trait NegotiatorComponent {
    /// Push forward negotiations as far as you can.
    /// `NegotiatorComponent` should modify only properties in his responsibility
    /// and return remaining part of Proposal unchanged.
    fn negotiate_step(
        &mut self,
        demand: &ProposalView,
        offer: ProposalView,
    ) -> anyhow::Result<NegotiationResult>;

    /// Called during Offer/Demand creation. `NegotiatorComponent` should add properties
    /// and constraints for which it is responsible during future negotiations.
    fn fill_template(&mut self, template: OfferTemplate) -> anyhow::Result<OfferTemplate>;

    /// Called when Agreement was finished. `NegotiatorComponent` can use termination
    /// result to adjust his future negotiation strategy.
    fn on_agreement_terminated(
        &mut self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()>;

    /// Called when Negotiator decided to approve/propose Agreement. It's only notification,
    /// `NegotiatorComponent` can't reject Agreement anymore.
    /// TODO: Can negotiator find out from which Proposals is this Agreement created??
    fn on_agreement_approved(&mut self, agreement: &AgreementView) -> anyhow::Result<()>;
}
