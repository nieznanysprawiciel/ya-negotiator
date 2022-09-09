use anyhow::{anyhow, bail};
use serde_yaml;
use std::path::PathBuf;
use std::time::Duration;
use tonic::Code;

use crate::grpc::{CallNegotiatorRequest, CreateNegotiatorRequest, ShutdownRequest};

use ya_agreement_utils::{AgreementView, OfferTemplate, ProposalView};
use ya_negotiator_component::component::{NegotiationResult, NegotiatorComponent, Score};
use ya_negotiator_component::{AgreementEvent, AgreementResult, RejectReason};

use crate::factory::{NegotiatorClient, RemoteServiceHandle};
use crate::message::{NegotiationMessage, NegotiationResponse};

/// Component forwarding calls to external binary using gRPC protocol.
#[allow(dead_code)]
pub struct GRPCComponent {
    service: RemoteServiceHandle,
    client: NegotiatorClient,
    id: String,
}

impl GRPCComponent {
    pub(crate) async fn new(
        path: PathBuf,
        name: &str,
        config: serde_yaml::Value,
        working_dir: PathBuf,
    ) -> anyhow::Result<GRPCComponent> {
        let service = RemoteServiceHandle::create_service(path.clone())
            .await
            .map_err(|e| anyhow!("Can't create service: {}. {e}", path.display()))?;
        let mut client = service.client().await;

        let request = tonic::Request::new(CreateNegotiatorRequest {
            name: name.to_string(),
            params: serde_yaml::to_string(&config)?,
            workdir: working_dir
                .to_str()
                .map(|path| path.to_string())
                .ok_or(anyhow!(
                    "Failed converting path: {} to string!",
                    working_dir.display()
                ))?,
        });

        let id = client
            .create_negotiator(request)
            .await
            .map_err(|e| anyhow!("GRPC: Failed to create negotiator: {name}. {e}"))?
            .into_inner()
            .id;

        Ok(GRPCComponent {
            service,
            client,
            id,
        })
    }

    async fn forward_rpc(&self, params: NegotiationMessage) -> anyhow::Result<NegotiationResponse> {
        let mut client = self.client.clone();
        let request = tonic::Request::new(CallNegotiatorRequest {
            id: self.id.clone(),
            message: serde_json::to_string(&params)
                .map_err(|e| anyhow!("Failed to serialize params: {e}"))?,
        });

        let response = client
            .call_negotiator(request)
            .await
            .map_err(|e| match e.code() {
                Code::Ok => anyhow!("{}", e.message()),
                _ => anyhow!("RPC call failed: {e}"),
            })?
            .into_inner();

        let result: NegotiationResponse = serde_json::from_str(&response.response)
            .map_err(|e| anyhow!("Failed to deserialize response: {e}"))?;
        Ok(result)
    }
}

#[async_trait::async_trait(?Send)]
impl NegotiatorComponent for GRPCComponent {
    async fn negotiate_step(
        &self,
        their: &ProposalView,
        template: ProposalView,
        score: Score,
    ) -> anyhow::Result<NegotiationResult> {
        let params = NegotiationMessage::NegotiateStep {
            their: their.clone(),
            template,
            score,
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::NegotiationResult(result) => Ok(result),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn fill_template(&self, template: OfferTemplate) -> anyhow::Result<OfferTemplate> {
        let params = NegotiationMessage::FillTemplate { template };

        match self.forward_rpc(params).await? {
            NegotiationResponse::OfferTemplate(template) => Ok(template),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn on_agreement_terminated(
        &self,
        agreement_id: &str,
        result: &AgreementResult,
    ) -> anyhow::Result<()> {
        let params = NegotiationMessage::AgreementTerminated {
            agreement_id: agreement_id.to_string(),
            result: result.clone(),
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::Empty => Ok(()),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn on_agreement_approved(&self, agreement: &AgreementView) -> anyhow::Result<()> {
        let params = NegotiationMessage::AgreementSigned {
            agreement: agreement.clone(),
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::Empty => Ok(()),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn on_proposal_rejected(&self, proposal_id: &str) -> anyhow::Result<()> {
        let params = NegotiationMessage::ProposalRejected {
            proposal_id: proposal_id.to_string(),
            reason: RejectReason::new("Not implemented"),
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::Empty => Ok(()),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn on_agreement_event(
        &self,
        agreement_id: &str,
        event: &AgreementEvent,
    ) -> anyhow::Result<()> {
        let params = NegotiationMessage::AgreementEvent {
            agreement_id: agreement_id.to_string(),
            event: event.clone(),
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::Empty => Ok(()),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn control_event(
        &self,
        component: &str,
        params: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let params = NegotiationMessage::ControlEvent {
            component: component.to_string(),
            params,
        };

        match self.forward_rpc(params).await? {
            NegotiationResponse::Generic(value) => Ok(value),
            _ => bail!("Unexpected `NegotiationResponse` type."),
        }
    }

    async fn shutdown(&self, timeout: Duration) -> anyhow::Result<()> {
        let mut client = self.client.clone();
        let request = tonic::Request::new(ShutdownRequest {
            id: self.id.clone(),
            timeout: timeout.as_secs_f32(),
        });

        client
            .shutdown_negotiator(request)
            .await
            .map_err(|e| anyhow!("GRPC: Failed to shutdown negotiator: {e}"))?;
        Ok(())
    }
}
