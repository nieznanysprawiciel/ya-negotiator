use ya_client_model::market::Proposal;
use ya_client_model::NodeId;

use crate::agreement::{expand, try_from_path, TypedPointer};
use crate::{Error, OfferTemplate};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProposalView {
    pub content: OfferTemplate,
    pub id: String,
    pub issuer: NodeId,
}

impl ProposalView {
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        self.content.pointer(pointer)
    }

    pub fn pointer_typed<'a, T: Deserialize<'a>>(&self, pointer: &str) -> Result<T, Error> {
        self.content.pointer_typed(pointer)
    }

    pub fn properties<'a, T: Deserialize<'a>>(
        &self,
        pointer: &str,
    ) -> Result<HashMap<String, T>, Error> {
        self.content.properties_at(pointer)
    }
}

impl TryFrom<Value> for ProposalView {
    type Error = Error;

    fn try_from(mut value: Value) -> Result<Self, Self::Error> {
        let offer = OfferTemplate {
            properties: value
                .pointer_mut("/properties")
                .map(Value::take)
                .unwrap_or(Value::Null),
            constraints: value
                .pointer("/constraints")
                .as_typed(Value::as_str)?
                .to_owned(),
        };
        Ok(ProposalView {
            content: offer,
            id: value
                .pointer("/proposalId")
                .as_typed(Value::as_str)?
                .to_owned(),
            issuer: value
                .pointer("/issuerId")
                .as_typed(Value::as_str)?
                .parse()
                .map_err(|e| Error::InvalidValue(format!("Can't parse NodeId. {}", e)))?,
        })
    }
}

impl TryFrom<&PathBuf> for ProposalView {
    type Error = Error;

    fn try_from(path: &PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(try_from_path(path)?)
    }
}

impl TryFrom<&Proposal> for ProposalView {
    type Error = Error;

    fn try_from(proposal: &Proposal) -> Result<Self, Self::Error> {
        Self::try_from(expand(serde_json::to_value(proposal)?))
    }
}
