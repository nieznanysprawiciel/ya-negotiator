use ya_client_model::NodeId;

#[derive(thiserror::Error, Debug)]
pub enum NegotiatorError {
    #[error("Requestor {0} not found")]
    RequestorNotFound(NodeId),
    #[error("Provider {0} not found")]
    ProviderNotFound(NodeId),
    #[error("Proposal {0} not found")]
    ProposalNotFound(String),
    #[error("Agreement {0} not found")]
    AgreementNotFound(String),
    #[error("Proposal {0} has no previous Proposal")]
    NoPrevProposal(String),
}
