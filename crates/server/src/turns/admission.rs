//! Temporary manual-turn port. Thread storage will implement this admission seam
//! when threads.send owns the message and job transaction.
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::Value;
use std::sync::Arc;

/// Authorized request identity shared with every other write in this space.
pub(super) struct ManualAdmission {
    content: Arc<calternal_content::Content>,
    account: AccountId,
    space: SpaceId,
    request: String,
    operation: String,
    fingerprint: String,
}
impl ManualAdmission {
    pub(super) fn new(
        content: Arc<calternal_content::Content>,
        account: AccountId,
        space: SpaceId,
        request: &str,
        operation: &str,
        input: &calternal_core::JsonObject,
    ) -> Self {
        Self {
            content,
            account,
            space,
            request: request.into(),
            operation: operation.into(),
            fingerprint: crate::api::fingerprint(operation, input),
        }
    }
    /// One shared receipt read and FULL publication under this space's lock.
    /// The callback must not re-enter content; jobs and authority use other stores.
    pub(super) fn apply(
        &self,
        change: impl FnOnce() -> Result<Value, Error>,
    ) -> Result<Value, Error> {
        self.content.admit(
            self.space,
            calternal_content::Request {
                account: self.account,
                id: &self.request,
                operation: &self.operation,
                fingerprint: &self.fingerprint,
            },
            change,
        )
    }
}
