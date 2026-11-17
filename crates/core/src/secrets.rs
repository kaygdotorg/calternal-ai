//! Wire contracts for secret operations. No type here carries a secret value
//! except [`SecretsProvideInput`], which only the trusted provide endpoint accepts.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Ask for a secret; the value is supplied later through the trusted surface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsRequestInput {
    /// Owning space UUID; omit for a personal (account-owned) secret.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub space_id: String,
    /// Secret name: 1–96 bytes of `a-z 0-9 . _ - :`, starting with a letter or digit.
    pub name: String,
    /// Short label shown in the prompt, at most 120 characters.
    pub label: String,
    /// Why it is needed, shown in the prompt, at most 500 characters.
    pub reason: String,
}
/// Pending request identifier returned by `secrets.request`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsRequestResult {
    /// UUID of the pending request, answered by the provide endpoint.
    pub request_id: String,
}
/// List secret metadata and pending requests of one owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsListInput {
    /// Owning space UUID; omit for the caller's personal secrets.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub space_id: String,
}
/// Everything about a stored secret except its value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretMetadata {
    /// Secret name.
    pub name: String,
    /// Human label.
    pub label: String,
    /// Current version, increased by each rotation.
    pub version: u64,
    /// Creation time in Unix milliseconds.
    pub created_at: i64,
    /// Last change in Unix milliseconds.
    pub updated_at: i64,
}
/// A request waiting for a value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretPendingRequest {
    /// Request UUID.
    pub id: String,
    /// Name the value will be stored under.
    pub name: String,
    /// Label shown in the prompt.
    pub label: String,
    /// Reason shown in the prompt.
    pub reason: String,
    /// Who asked: account and route (session, plugin or agent thread).
    pub requested_by: String,
    /// Request time in Unix milliseconds.
    pub created_at: i64,
}
/// Metadata and pending requests of one owner; never values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsListResult {
    /// Stored secrets ordered by name.
    pub secrets: Vec<SecretMetadata>,
    /// Pending requests, oldest first.
    pub requests: Vec<SecretPendingRequest>,
}
/// Destroy a secret (`name`) or withdraw a pending request (`request_id`); exactly one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsRevokeInput {
    /// Owning space UUID; omit for a personal secret.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub space_id: String,
    /// Secret to destroy, with every version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub name: String,
    /// Pending request to withdraw.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub request_id: String,
}
/// Empty success for `secrets.revoke`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsRevokeResult {}
/// Body of `POST /api/v1/auth/secrets/provide`. Not an operation: only a signed-in
/// browser or app session with a recent passkey assertion may send it.
#[derive(Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsProvideInput {
    /// Owning space UUID; omit for a personal secret.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    pub space_id: String,
    /// Pending request being answered.
    pub request_id: String,
    /// The secret value, 1 byte to 64 KiB of UTF-8.
    pub value: String,
}
impl std::fmt::Debug for SecretsProvideInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretsProvideInput")
            .field("space_id", &self.space_id)
            .field("request_id", &self.request_id)
            .finish_non_exhaustive()
    }
}
/// Which secret the provide endpoint stored; never the value or a handle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SecretsProvideResult {
    /// Secret name.
    pub name: String,
    /// Version now stored.
    pub version: u64,
}
