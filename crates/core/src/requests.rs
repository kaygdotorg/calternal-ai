//! Account request and standing instruction contracts.
use crate::{GenUiView, JsonObject, OperationDescription, OperationRisk};
use calternal_types::{Capability, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A request is presented only to the authenticated recipient.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct PersonRequest {
    /// Stable request UUID.
    pub id: String,
    /// Addressed space UUID, absent for personal secret requests.
    pub space_id: Option<String>,
    /// Thread UUID when the request was raised during a turn.
    pub thread_id: Option<String>,
    /// Job UUID for a plugin wait addressed to this person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub job_id: Option<String>,
    /// Question, approval or trusted secret prompt.
    pub detail: RequestDetail,
}

/// The information needed to render each pending request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RequestDetail {
    /// One generated form, which may have multiple fields.
    Question {
        /// Label shown in inbox summaries and non-interactive clients.
        label: String,
        /// Form drawn by the application.
        form: GenUiView,
    },
    /// Trusted, fixed form describing the proposed authority.
    Approval {
        /// App operation prefix.
        app: String,
        /// Whether the proposed grant includes writes.
        write: bool,
        /// Choices enforced by the server for this action, in display order.
        lifetimes: Vec<ApprovalLifetime>,
        /// Exact operation details when raised by an automatic approval check.
        #[ts(type = "JsonObject")]
        action: JsonObject,
    },
    /// An expiring challenge from an official machine-side login program.
    ProviderSignIn {
        /// Installed provider plugin ID.
        provider_id: String,
        /// Provider prompt shown privately to the signed-in person.
        label: String,
        /// Provider-owned HTTPS authorization page.
        url: Option<String>,
        /// Code to enter on the provider page for a device flow.
        display_code: Option<String>,
        /// The official command expects an authorization code from the user.
        needs_code: bool,
        /// Unix milliseconds when the pending challenge expires.
        #[ts(type = "number")]
        expires_at_ms: i64,
    },
    /// Trusted, fixed form describing the existing secret request.
    Secret {
        /// Secret name, never the value.
        name: String,
        /// Short human-readable name.
        label: String,
        /// Why the caller requested the credential.
        reason: String,
        /// Unix milliseconds when the original fifteen-minute request expires.
        #[ts(type = "number")]
        expires_at_ms: i64,
    },
}

/// How long approved authority remains usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalLifetime {
    /// The first matching action may use the grant.
    Once,
    /// The grant survives turns until the thread is archived or deleted.
    ThisThread,
}

/// Account-private pending request snapshot.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct RequestsListResult {
    /// Currently authorized requests, ordered by ID.
    pub requests: Vec<PersonRequest>,
}

/// Resolve a pending question with the submitted form values.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestAnswerInput {
    /// Question UUID.
    pub id: String,
    /// Submitted form values.
    #[ts(type = "JsonObject")]
    pub answer: JsonObject,
}

/// Resolve a pending approval with the selected lifetime.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestApproveInput {
    /// Approval UUID.
    pub id: String,
    /// Maximum duration of the grant.
    pub lifetime: ApprovalLifetime,
}

/// Decline one pending approval.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestDenyInput {
    /// Approval UUID.
    pub id: String,
}

/// Result of resolving a private request.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct RequestResolution {
    /// Resolved request UUID.
    pub id: String,
    /// Answered, approved or denied.
    pub status: String,
}

/// Raise a question from the caller's current thread.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct RequestAskInput {
    /// Form capable of asking several questions at once.
    pub form: GenUiView,
}

/// Standing instructions for one space. Omitting Markdown reads the file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceInstructionsInput {
    /// Space UUID.
    pub space_id: String,
    /// Replacement Markdown, limited to 16 KiB.
    pub markdown: Option<String>,
}

/// The bounded standing instructions document.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct SpaceInstructionsResult {
    /// Markdown text. An absent file is represented by empty text.
    pub markdown: String,
}

type Schema = fn() -> JsonObject;

/// Target and authorization of the built-in request and instruction operations.
#[derive(Clone, Copy)]
pub enum RequestTarget {
    /// Signed-in account's private requests, with a membership check for each space.
    Account,
    /// Issuing agent's current turn. Space and job come from its token.
    Turn,
    /// Explicit space UUID and current space settings capability.
    Space,
}

/// One catalog entry used by discovery and server dispatch.
pub struct RequestOperation {
    /// Wire operation name.
    pub id: &'static str,
    /// Required capability for the operation's space targets.
    pub required: Capability,
    /// How the host derives the target.
    pub target: RequestTarget,
    risk: OperationRisk,
    description: &'static str,
    input: Schema,
    output: Schema,
    validate: fn(&JsonObject) -> Result<(), Error>,
}

fn schema<T: JsonSchema>() -> JsonObject {
    serde_json::from_value(serde_json::to_value(schemars::schema_for!(T)).expect("schema"))
        .expect("object schema")
}
fn validate<T: serde::de::DeserializeOwned>(input: &JsonObject) -> Result<(), Error> {
    serde_json::from_value::<T>(serde_json::json!(input))
        .map(drop)
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}
macro_rules! operation {
    ($id:literal, $description:literal, $target:ident, $capability:ident, $risk:ident, $input:ty, $output:ty) => {
        RequestOperation {
            id: $id,
            description: $description,
            required: Capability::$capability,
            target: RequestTarget::$target,
            risk: OperationRisk::$risk,
            input: schema::<$input>,
            output: schema::<$output>,
            validate: validate::<$input>,
        }
    };
}
/// Static policy rows, ordered by the public operation catalog.
pub const REQUEST_OPERATIONS: &[RequestOperation] = &[
    operation!(
        "plugins.settings.get",
        "Read a declared space plugin setting",
        Space,
        SpaceSettings,
        Read,
        crate::PluginSettingAddress,
        crate::PluginSettingValue
    ),
    operation!(
        "plugins.settings.set",
        "Edit a declared space plugin setting",
        Space,
        SpaceSettings,
        Reversible,
        crate::PluginSettingEdit,
        crate::PluginSettingChange
    ),
    operation!(
        "provider.sign_in.start",
        "Start a private provider sign-in",
        Account,
        PluginsUse,
        Reversible,
        crate::ProviderSignInStart,
        crate::ProviderSignInResult
    ),
    operation!(
        "provider.sign_in.answer",
        "Supply a code to the official provider login",
        Account,
        PluginsUse,
        Reversible,
        crate::ProviderSignInAnswer,
        crate::ProviderSignInResult
    ),
    operation!(
        "provider.sign_in.status",
        "Read a private provider sign-in result",
        Account,
        PluginsUse,
        Read,
        crate::ProviderSignInStatus,
        crate::ProviderSignInResult
    ),
    operation!(
        "provider.sign_in.typing",
        "Offer provider re-authentication while composing",
        Account,
        PluginsUse,
        Reversible,
        crate::ProviderSignInTyping,
        Option<crate::ProviderSignInResult>
    ),
    operation!(
        "requests.list",
        "List requests waiting on the signed-in account",
        Account,
        SpaceRead,
        Read,
        crate::AccountInputList,
        RequestsListResult
    ),
    operation!(
        "requests.ask",
        "Ask the person a question from the current turn",
        Turn,
        PluginsUse,
        Reversible,
        RequestAskInput,
        RequestResolution
    ),
    operation!(
        "requests.answer",
        "Answer a pending question",
        Account,
        SpaceRead,
        Reversible,
        RequestAnswerInput,
        RequestResolution
    ),
    operation!(
        "requests.approve",
        "Approve with a fresh verified passkey",
        Account,
        PluginsUse,
        WidensAccess,
        RequestApproveInput,
        RequestResolution
    ),
    operation!(
        "requests.deny",
        "Decline a pending approval",
        Account,
        PluginsUse,
        Reversible,
        RequestDenyInput,
        RequestResolution
    ),
    operation!(
        "space.instructions",
        "Read or replace standing space instructions",
        Space,
        SpaceSettings,
        Irreversible,
        SpaceInstructionsInput,
        SpaceInstructionsResult
    ),
];

/// Find a request operation without allocating.
pub fn request_operation(id: &str) -> Option<&'static RequestOperation> {
    REQUEST_OPERATIONS.iter().find(|row| row.id == id)
}

impl RequestOperation {
    /// Generate the transport schemas for the cached discovery catalog.
    pub(crate) fn description(&self) -> OperationDescription {
        OperationDescription {
            required_capability: None,
            target: None,
            id: self.id.into(),
            description: self.description.into(),
            mutates: self.risk != OperationRisk::Read,
            risk: self.risk,
            input_schema: (self.input)(),
            output_schema: (self.output)(),
        }
    }

    /// Check the declared input shape without granting any authority.
    pub(crate) fn validate(&self, input: &JsonObject) -> Result<(), Error> {
        (self.validate)(input)
    }
}
