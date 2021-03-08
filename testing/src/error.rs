use ya_client_model::NodeId;

/// Note: trace can't be of type Backtrace, because thiserror
/// treats it as fields with special meaning, at it doesn't compile.
#[derive(thiserror::Error, Debug)]
pub enum NegotiatorError {
    #[error("Requestor {node_id} not found.")]
    RequestorNotFound { node_id: NodeId, trace: String },
    #[error("Provider {node_id} not found.")]
    ProviderNotFound { node_id: NodeId, trace: String },
    #[error("Proposal {id} not found. {trace}")]
    ProposalNotFound { id: String, trace: String },
    #[error("Agreement {id} not found.")]
    AgreementNotFound { id: String, trace: String },
    #[error("Proposal {id} has no previous Proposal.")]
    NoPrevProposal { id: String, trace: String },
}
