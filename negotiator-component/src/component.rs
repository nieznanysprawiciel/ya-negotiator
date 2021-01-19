use serde::{Deserialize, Serialize};

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_client_model::market::Reason;

/// Result returned by `NegotiatorComponent` during Proposals evaluation.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum NegotiationResult {
    /// `NegotiatorComponent` fully negotiated his part of Proposal, and it can be turned into
    /// valid Agreement without changes. Provider will send counter Proposal.
    /// `NegotiatorComponent` shouldn't return this value, if he changed anything in his
    /// part of Proposal.
    /// On Requestor side returning this type means, that Proposal will be proposed
    /// to Provider. On Provider side it doesn't have any consequences, since Provider
    /// doesn't have initiative to propose Agreements.
    Ready { proposal: ProposalView },
    /// Proposal is not ready to become Agreement, but negotiations
    /// are in progress.
    Negotiating { proposal: ProposalView },
    /// Proposal is not acceptable and should be rejected.
    /// Negotiations can't be continued.
    Reject { reason: Option<Reason> },
}

/// Result of agreement execution.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AgreementResult {
    /// Failed to approve agreement. (Agreement even wasn't created).
    /// It can happen for Provider in case call to `approve_agreement` will fail.
    /// For Requestor it happens, when Agreement gets rejected or it's creation/sending fails.
    /// TODO: Maybe we should distinguish these cases with enum??
    ApprovalFailed,
    /// Agreement was finished with success after first Activity.
    ClosedByProvider,
    /// Agreement was finished with success by Requestor.
    ClosedByRequestor,
    /// Agreement was broken by one party. It indicates non successful end of Agreement.
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
    ///
    /// Parameters:
    /// incoming_proposal - Proposal that we got from other party.
    /// template - First component gets Proposal from previous negotiations. All subsequent
    ///            components change this Proposal and than it is passed to next component
    ///            in modified shape.
    fn negotiate_step(
        &mut self,
        incoming_proposal: &ProposalView,
        template: ProposalView,
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
