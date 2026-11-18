//! Native provider session ownership for saved conversations.
use calternal_types::AccountId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// One bounded session record kept in the thread envelope and snapshotted at admission.
pub(crate) const KEY: &str = "provider_session";

/// Provider-local session reference. A thread-scoped binding checks the
/// machine and account without storing a machine ID in shared Markdown.
/// The session ID identifies stored context and conveys no access.
#[derive(Serialize, Deserialize)]
pub(crate) struct ProviderSession {
    account: AccountId,
    provider: String,
    machine_binding: String,
    session_id: String,
}

impl ProviderSession {
    /// Bind an already validated provider event to the authenticated turn's placement.
    pub(crate) fn new(
        account: AccountId,
        provider: &str,
        thread: Uuid,
        machine: Uuid,
        session_id: &str,
    ) -> Self {
        Self {
            account,
            provider: provider.into(),
            machine_binding: binding(account, thread, machine),
            session_id: session_id.into(),
        }
    }

    /// Decode an existing record only for its owner. No I/O.
    pub(crate) fn owned(metadata: &Value, account: AccountId) -> Option<Self> {
        let record: Self = serde_json::from_value(metadata.get(KEY)?.clone()).ok()?;
        (record.account == account).then_some(record)
    }

    /// Return the native session for the same provider and machine, or the notice
    /// for the current account's fresh turn. No work is transferred.
    pub(crate) fn resume(
        self,
        provider: &str,
        thread: Uuid,
        machine: Uuid,
    ) -> Result<String, &'static str> {
        if self.provider != provider {
            return Err("Starting a new provider session because the provider changed.");
        }
        if self.machine_binding != binding(self.account, thread, machine) {
            return Err("Starting a new provider session because the machine changed.");
        }
        Ok(self.session_id)
    }
}

/// Derive a thread- and account-bound machine reference without publishing the
/// real machine ID. One hash of fixed-size identifiers.
pub(crate) fn binding(account: AccountId, thread: Uuid, machine: Uuid) -> String {
    crate::store::sha256_hex(format!("provider-session/{account}/{thread}/{machine}").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn provider_session_is_owned_and_local_to_its_binding() {
        let owner = AccountId::new();
        let machine = Uuid::new_v4();
        let thread = Uuid::new_v4();
        let metadata =
            json!({(KEY): ProviderSession::new(owner, "claude", thread, machine, "one")});
        assert!(!metadata.to_string().contains(&machine.to_string()));
        let owned = || ProviderSession::owned(&metadata, owner).unwrap();

        assert!(ProviderSession::owned(&metadata, AccountId::new()).is_none());
        assert_eq!(owned().resume("claude", thread, machine), Ok("one".into()));
        assert!(
            owned()
                .resume("codex", thread, machine)
                .unwrap_err()
                .contains("provider changed")
        );
        assert!(
            owned()
                .resume("claude", thread, Uuid::new_v4())
                .unwrap_err()
                .contains("machine changed")
        );
    }
}
