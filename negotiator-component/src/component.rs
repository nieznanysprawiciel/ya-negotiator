use serde::{Deserialize, Serialize};

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_client_model::market::Reason;

/// Structure for exchanging Proposal evaluation score.
/// Each `NegotiatorComponent` can add it's own score value the same way,
/// as it adds properties to Proposal. `NegotiatorComponents` can read scores returned
/// by previous components and base it's results on it (Note that component can never
/// be sure, that any score will be returned by previous components).
///
/// `NegotiatorComponent` is allowed to add as many score values (or other additional information)
/// as he wants. Each component defines himself under what names these scores will be placed in
/// `Score` structure (so namespacing is encouraged).
/// Property `final-score` has special meaning and will be used by `CompositeNegotiator` to
/// choose between Proposals and Agreements.
///
/// We use the same structure for scoring Proposals as for negotiating
/// them. We don't need constraints part here, but this structure has many utils
/// useful for properties manipulation, that I don't want to duplicate its functionality.
pub type Score = OfferTemplate;

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
    Ready {
        proposal: ProposalView,
        score: Score,
    },
    /// Proposal is not ready to become Agreement, but negotiations
    /// are in progress.
    Negotiating {
        proposal: ProposalView,
        score: Score,
    },
    /// Proposal is not acceptable and should be rejected.
    /// Negotiations can't be continued.
    Reject { reason: Option<Reason> },
}

/// Result of agreement execution.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AgreementResult {
    /// Failed to approve agreement. (Agreement even wasn't created).
    /// It can happen for Provider in case call to `approve_agreement` will fail.
    /// For Requestor it happens, when Agreement gets rejected or it's creation/sending fails.
    /// TODO: Maybe we should distinguish these cases with enum??
    /// TODO: We should pass rejection Reason.
    ApprovalFailed,
    /// Agreement was finished with success after first Activity.
    ClosedByProvider,
    /// Agreement was finished with success by Requestor.
    ClosedByRequestor,
    /// Agreement was broken by one party. It indicates non successful end of Agreement.
    Broken { reason: Option<Reason> },
}

/// Notification about things happening with Agreement after it's termination.
#[non_exhaustive]
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum AgreementEvent {
    InvoiceAccepted,
    /// Could be partially paid??
    InvoicePaid,
    InvoiceRejected,
    /// Provider/Requestor is unreachable, so we can't send terminate Agreement.
    UnableToTerminate,
    ComputationFailure(serde_json::Value),
    Custom(serde_json::Value),
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
    /// Push forward negotiations as far as you can. Evaluate Proposal.
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
        _their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        Ok(NegotiationResult::Ready {
            proposal: template,
            score,
        })
    }

    /// Called during Offer/Demand creation. `NegotiatorComponent` should add properties
    /// and constraints for which it is responsible during future negotiations.
    fn fill_template(&mut self, template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        Ok(template)
    }

    /// Called when Agreement was finished. `NegotiatorComponent` can use termination
    /// result to adjust his future negotiation strategy.
    fn on_agreement_terminated(
        &mut self,
        _agreement_id: &str,
        _result: &AgreementResult,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called when Negotiator decided to approve/propose Agreement. It's only notification,
    /// `NegotiatorComponent` can't reject Agreement anymore.
    /// TODO: Can negotiator find out from which Proposals is this Agreement created??
    fn on_agreement_approved(&mut self, _agreement: &AgreementView) -> anyhow::Result<()> {
        Ok(())
    }

    /// Called when other party rejects our Proposal.
    /// TODO: We should call this, if any of our components rejected Proposal either.
    ///       Add flag that will indicate who rejected.
    /// TODO: Add Reason parameter.
    fn on_proposal_rejected(&mut self, _proposal_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Notifies `NegotiatorComponent`, about events related to Agreement appearing after
    /// it's termination.
    fn on_agreement_event(
        &mut self,
        _agreement_id: &str,
        _event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Allows to control `NegotiatorComponent's` behavior or query any information
    /// from it. Thanks to this event Requestor/Provider implementation can interact with
    /// `NegotiatorComponents`.
    fn control_event(
        &mut self,
        _component: &str,
        _params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::Value::Null)
    }
}
