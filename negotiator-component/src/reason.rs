use derive_more::Display;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use ya_client_model::market::Reason;

/// Helper structure providing functionalities to build `Reason`
/// in case of rejecting Agreement/Proposal.  
#[derive(Clone, Display, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[display(fmt = "'{}'", message)]
pub struct RejectReason {
    pub message: String,
    #[serde(flatten)]
    pub extra: serde_json::Value,
}

impl RejectReason {
    pub fn new(message: impl ToString) -> RejectReason {
        RejectReason {
            message: message.to_string(),
            extra: serde_json::json!({}),
        }
    }

    pub fn entry<T: Into<serde_json::Value>>(
        mut self,
        key: impl ToString,
        value: T,
    ) -> RejectReason {
        self.extra
            .as_object_mut()
            .unwrap()
            .insert(key.to_string(), value.into());
        self
    }

    pub fn final_flag(self, flag: bool) -> Self {
        self.entry("golem.proposal.rejection.is-final".to_string(), flag)
    }
}

impl Into<Reason> for RejectReason {
    fn into(self) -> Reason {
        Reason {
            message: self.message,
            extra: self.extra,
        }
    }
}

impl Into<Option<Reason>> for RejectReason {
    fn into(self) -> Option<Reason> {
        Some(self.into())
    }
}
