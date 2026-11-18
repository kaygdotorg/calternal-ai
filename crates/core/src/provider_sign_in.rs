//! Account-only provider sign-in commands and their private outcome.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Select the sandbox and official provider for sign-in.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderSignInStart {
    /// Space UUID in which the caller may use this provider.
    pub space_id: String,
    /// Optional saved thread UUID for the private prompt and final notice.
    #[serde(default)]
    pub thread_id: Option<String>,
    /// Installed provider plugin ID.
    pub provider_id: String,
    /// Explicitly selected personal machine UUID.
    #[serde(default)]
    pub machine_id: Option<String>,
    /// Use an entitled anonymous pool machine. Mutually exclusive with machine_id.
    #[serde(default)]
    pub pool: bool,
}

/// Supply a browser authorization code to the original official process.
#[derive(Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderSignInAnswer {
    /// Account-private request UUID.
    pub id: String,
    /// The short-lived code. Never saved in a request receipt.
    pub code: String,
}

/// Look up one account-private, volatile sign-in request.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderSignInStatus {
    /// Account-private request UUID.
    pub id: String,
}

/// Report that the person began composing in a saved thread.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderSignInTyping {
    /// Authorized space UUID.
    pub space_id: String,
    /// Saved thread UUID.
    pub thread_id: String,
}

/// A sign-in result never includes the challenge or the browser's answer.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
pub struct ProviderSignInResult {
    /// Account-private request UUID.
    pub id: String,
    /// Pending, done or failed.
    pub status: String,
}
