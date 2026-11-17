use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;
use calternal_types::ErrorCode;

/// JSON object with deterministically ordered top-level keys; nested values remain arbitrary JSON.
pub type JsonObject = BTreeMap<String, Value>;
/// Operation and worker transport version accepted by this preview.
pub const PROTOCOL_VERSION: u32 = 1;
/// Portable declarative view version accepted by this preview.
pub const GENUI_VERSION: u32 = 1;
/// Maximum Markdown document size in UTF-8 bytes (1 MiB).
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
/// Maximum serialized operation body size in bytes (2 MiB).
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
/// Default loopback development HTTP port; not an external deployment endpoint.
pub const DEFAULT_PORT: u16 = 3210;

pub(crate) fn optional_present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
pub(crate) fn required_nullable_string<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
pub(crate) fn nullable_string_schema(g: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    g.subschema_for::<Option<String>>()
}

/// Envelope for every `POST /api/v1/operations` call. Retain request identity and input unchanged after uncertain mutation outcomes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    /// Caller correlation/retry identity, 1..128 Unicode scalars; preserve it with identical input on retry.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    /// Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.
    pub operation: String,
    /// Operation-specific JSON object; must pass the selected operation schema before dispatch.
    pub input: JsonObject,
}

/// Safe error payload for all operations; messages must not contain credentials or private diagnostics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    /// Stable machine-readable failure category; callers should branch on this rather than message text.
    pub code: ErrorCode,
    /// Human-readable safe diagnostic; never include credentials or raw private exception details.
    pub message: String,
    /// Optional structured diagnostic object; omission differs from explicit JSON null, which is rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "JsonObject")]
    pub details: Option<JsonObject>,
}

// Singleton booleans keep deserialization, JSON Schema and TS discriminants exact.
macro_rules! literal_bool {
    ($name:ident, $value:expr) => {
        /// Exact boolean wire discriminant. Deserialization rejects the opposite value.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_bool($value)
            }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                if bool::deserialize(d)? == $value {
                    Ok(Self)
                } else {
                    Err(serde::de::Error::custom("wrong boolean discriminant"))
                }
            }
        }
        impl JsonSchema for $name {
            fn schema_name() -> String {
                stringify!($name).into()
            }
            fn json_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
                let mut object = schemars::schema::SchemaObject::default();
                object.instance_type = Some(schemars::schema::InstanceType::Boolean.into());
                object.const_value = Some(Value::Bool($value));
                object.into()
            }
        }
        impl TS for $name {
            type WithoutGenerics = Self;
            fn name() -> String {
                stringify!($value).into()
            }
            fn inline() -> String {
                Self::name()
            }
            fn inline_flattened() -> String {
                Self::name()
            }
            fn decl() -> String {
                String::new()
            }
            fn decl_concrete() -> String {
                String::new()
            }
        }
    };
}
literal_bool!(True, true);
literal_bool!(False, false);
/// Successful response envelope for any operation, carrying its matching request identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Success<T> {
    /// Caller correlation/retry identity, 1..128 Unicode scalars; preserve it with identical input on retry.
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    /// Exact boolean discriminant required by this envelope; the opposite value cannot deserialize.
    pub ok: True,
    /// Operation-specific successful result payload.
    pub data: T,
}
/// Failed response envelope for any operation; a malformed request may have no recoverable identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Recovered caller correlation identity; null only when the request ID could not be recovered.
    #[serde(deserialize_with = "required_nullable_string")]
    #[schemars(schema_with = "nullable_string_schema")]
    pub request_id: Option<String>,
    /// Exact boolean discriminant required by this envelope; the opposite value cannot deserialize.
    pub ok: False,
    /// Safe structured failure returned instead of successful data.
    pub error: ApiError,
}
/// Discriminated success or failure envelope used by all operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum OperationResponse<T> {
    /// A correlated successful result with its exact true discriminant.
    Success(Success<T>),
    /// A correlated failure with its exact false discriminant.
    Failure(Failure),
}
impl<T> OperationResponse<T> {
    /// Wrap an operation result with its correlation identity; callers provide a valid request ID.
    pub fn success(request_id: impl Into<String>, data: T) -> Self {
        Self::Success(Success {
            request_id: request_id.into(),
            ok: True,
            data,
        })
    }
    /// Wrap a safe failure; use no request ID only when the request identity could not be recovered.
    pub fn failure(
        request_id: Option<String>,
        code: ErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self::Failure(Failure {
            request_id,
            ok: False,
            error: ApiError {
                code,
                message: message.into(),
                details: None,
            },
        })
    }
}
/// Preview role exposed by `GET /api/v1/describe`; membership is enforced by the host, never by this enum alone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpaceRole {
    /// Read authorized space content.
    Reader,
    /// Read and edit authorized space content.
    Editor,
    /// Read, edit and manage space plugins.
    Manager,
    /// Sole owner; includes Manager grants plus management of Managers, transfer and deletion.
    Owner,
}
/// Space membership exposed by `GET /api/v1/describe` for the authenticated principal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceGrant {
    /// Space UUID; the only way a space is named on the wire.
    pub id: String,
    /// Current preview membership role for this space.
    pub role: SpaceRole,
}
/// Authenticated preview identity returned by `GET /api/v1/describe` with its current space memberships.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    /// Authenticated account identity; never substitute it for a bearer credential.
    pub id: String,
    /// Space grants visible for this authenticated identity.
    pub spaces: Vec<SpaceGrant>,
}
/// Discovery entry for a versioned operation with generated input and output schemas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationDescription {
    /// Versioned operation identifier used for dispatch; never a URL or executable path.
    pub id: String,
    /// Human-readable description for discovery; carries no execution authority.
    pub description: String,
    /// Whether this operation changes durable state; extension declarations are constrained to false.
    pub mutates: bool,
    /// Generated JSON Schema object for operation-specific input validation.
    pub input_schema: JsonObject,
    /// Generated JSON Schema object for the successful operation result.
    pub output_schema: JsonObject,
}
/// Result of `GET /api/v1/describe`; versions, caller identity and operation discovery.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DescribeResult {
    /// Transport contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    /// Portable view contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub genui_version: u32,
    /// Authenticated identity and memberships; no bearer credential is returned.
    pub principal: Principal,
    /// Declared/discovered operation contracts; dispatch still checks current permissions.
    pub operations: Vec<OperationDescription>,
}
// Schema export root groups every cross-lane type; not an HTTP request body.
/// Schema export root grouping all operation types; never an HTTP request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    /// Common address resolution input.
    pub resolve_input: ResolveInput,
    /// Current visible address resolution.
    pub resolve_result: ResolveResult,
    /// Schema representative for the stored space directory spaces list input.
    pub spaces_list_input: crate::SpacesListInput,
    /// Schema representative for the stored space directory spaces create input.
    pub spaces_create_input: crate::SpacesCreateInput,
    /// Schema representative for the stored space directory space input.
    pub space_input: crate::SpaceInput,
    /// Schema representative for the stored space directory space member input.
    pub space_member_input: crate::SpaceMemberInput,
    /// Schema representative for the stored space directory space set role input.
    pub space_set_role_input: crate::SpaceSetRoleInput,
    /// Schema representative for the stored space directory spaces list result.
    pub spaces_list_result: crate::SpacesListResult,
    /// Schema representative for the stored space directory space result.
    pub space_result: crate::SpaceResult,
    /// Schema representative for the stored space directory space members result.
    pub space_members_result: crate::SpaceMembersResult,
    /// Schema representative for the stored space directory space changed result.
    pub space_changed_result: crate::SpaceChangedResult,
    /// Schema representative for the secrets request input.
    pub secrets_request_input: crate::SecretsRequestInput,
    /// Schema representative for the secrets request result.
    pub secrets_request_result: crate::SecretsRequestResult,
    /// Schema representative for the secrets list input.
    pub secrets_list_input: crate::SecretsListInput,
    /// Schema representative for the secrets list result.
    pub secrets_list_result: crate::SecretsListResult,
    /// Schema representative for the secrets revoke input.
    pub secrets_revoke_input: crate::SecretsRevokeInput,
    /// Schema representative for the secrets revoke result.
    pub secrets_revoke_result: crate::SecretsRevokeResult,
    /// Schema representative for the trusted provide endpoint's body.
    pub secrets_provide_input: crate::SecretsProvideInput,
    /// Schema representative for the trusted provide endpoint's result.
    pub secrets_provide_result: crate::SecretsProvideResult,
    /// Schema export representative for `request`; not a field accepted by operation dispatch.
    pub request: OperationRequest,
    /// Schema export representative for `response`; not a field accepted by operation dispatch.
    pub response: OperationResponse<Value>,
    /// Schema export representative for `describe`; not a field accepted by operation dispatch.
    pub describe: DescribeResult,
    /// Schema export representative for `note`; not a field accepted by operation dispatch.
    pub note: Note,
    /// Schema export representative for `notes_list_input`; not a field accepted by operation dispatch.
    pub notes_list_input: NotesListInput,
    /// Schema export representative for `notes_get_input`; not a field accepted by operation dispatch.
    pub notes_get_input: NotesGetInput,
    /// Schema export representative for `notes_create_input`; not a field accepted by operation dispatch.
    pub notes_create_input: NotesCreateInput,
    /// Schema export representative for `notes_update_input`; not a field accepted by operation dispatch.
    pub notes_update_input: NotesUpdateInput,
    /// Schema export representative for `notes_history_input`; not a field accepted by operation dispatch.
    pub notes_history_input: NotesHistoryInput,
    /// Schema export representative for `notes_list_result`; not a field accepted by operation dispatch.
    pub notes_list_result: NotesListResult,
    /// Schema export representative for `note_result`; not a field accepted by operation dispatch.
    pub note_result: NoteResult,
    /// Schema export representative for `notes_history_result`; not a field accepted by operation dispatch.
    pub notes_history_result: NotesHistoryResult,
    /// Schema export representative for `plugins_list_input`; not a field accepted by operation dispatch.
    pub plugins_list_input: PluginsListInput,
    /// Schema export representative for `plugin_input`; not a field accepted by operation dispatch.
    pub plugin_input: PluginInput,
    /// Schema export representative for `plugins_list_result`; not a field accepted by operation dispatch.
    pub plugins_list_result: PluginsListResult,
    /// Schema export representative for `plugin_result`; not a field accepted by operation dispatch.
    pub plugin_result: PluginResult,
    /// Schema export representative for `plugin_preview_result`; not a field accepted by operation dispatch.
    pub plugin_preview_result: PluginPreviewResult,
    /// Schema export representative for `summarize_result`; not a field accepted by operation dispatch.
    pub summarize_result: SummarizeResult,
}
