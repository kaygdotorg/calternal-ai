use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use ts_rs::TS;

pub type JsonObject = BTreeMap<String, Value>;
pub const PROTOCOL_VERSION: u32 = 1;
pub const GENUI_VERSION: u32 = 1;
pub const MAX_DOCUMENT_BYTES: usize = 1024 * 1024;
pub const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_WORKER_OUTPUT_BYTES: usize = 256 * 1024;
pub const DEFAULT_PORT: u16 = 3210;

fn optional_present<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(d).map(Some)
}
fn required_nullable_string<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
fn nullable_string_schema(g: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    g.subschema_for::<Option<String>>()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationRequest {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub operation: String,
    pub input: JsonObject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    NotFound,
    Conflict,
    InvalidInput,
    TooLarge,
    Unavailable,
    Internal,
}
impl ErrorCode {
    pub fn http_status(self) -> u16 {
        match self {
            Self::Unauthorized => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::Conflict => 409,
            Self::InvalidInput => 400,
            Self::TooLarge => 413,
            Self::Unavailable => 503,
            Self::Internal => 500,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Success<T> {
    #[schemars(length(min = 1, max = 128))]
    pub request_id: String,
    pub ok: True,
    pub data: T,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    #[serde(deserialize_with = "required_nullable_string")]
    #[schemars(schema_with = "nullable_string_schema")]
    pub request_id: Option<String>,
    pub ok: False,
    pub error: ApiError,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum OperationResponse<T> {
    Success(Success<T>),
    Failure(Failure),
}
impl<T> OperationResponse<T> {
    pub fn success(request_id: impl Into<String>, data: T) -> Self {
        Self::Success(Success {
            request_id: request_id.into(),
            ok: True,
            data,
        })
    }
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteSummary {
    pub id: String,
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    pub revision: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub id: String,
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    pub markdown: String,
    pub revision: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteVersion {
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    pub markdown: String,
    pub revision: String,
    pub updated_at: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesGetInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    pub id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesCreateInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    pub markdown: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesUpdateInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    pub id: String,
    #[schemars(length(min = 1, max = 256))]
    pub base_revision: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    pub markdown: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    pub id: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "u32")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    #[schemars(length(min = 1, max = 256))]
    pub cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListResult {
    pub notes: Vec<NoteSummary>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteResult {
    pub note: Note,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryResult {
    pub versions: Vec<NoteVersion>,
    #[serde(deserialize_with = "required_nullable_string")]
    #[schemars(schema_with = "nullable_string_schema")]
    pub next_cursor: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsListInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginInput {
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    pub plugin_id: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginDependency {
    pub id: String,
    pub version: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginOperation {
    pub id: String,
    pub description: String,
    pub mutates: False,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum ExtensionPoint {
    #[serde(rename = "notes.detail")]
    NotesDetail,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginView {
    pub id: String,
    pub extension_point: ExtensionPoint,
    pub view: GenUiView,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub api_version: u32,
    pub name: String,
    pub description: String,
    pub dependencies: Vec<PluginDependency>,
    pub capabilities: Vec<String>,
    pub operations: Vec<PluginOperation>,
    pub views: Vec<PluginView>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginState {
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub generation: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsListResult {
    pub plugins: Vec<PluginState>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginResult {
    pub plugin: PluginState,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginPreviewResult {
    pub view: GenUiView,
    pub preview: True,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    pub words: u32,
    pub characters: u32,
    pub lines: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SummarizeResult {
    pub stats: Stats,
    pub view: GenUiView,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenUiAction {
    pub operation: String,
    pub input: JsonObject,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub enum GenUiNode {
    Stack {
        id: String,
        children: Vec<GenUiNode>,
    },
    Text {
        id: String,
        text: String,
    },
    Button {
        id: String,
        label: String,
        action: GenUiAction,
    },
    Stat {
        id: String,
        label: String,
        value: String,
    },
    Resource {
        id: String,
        resource_id: String,
        space_id: String,
        label: String,
    },
    DocumentEditor {
        id: String,
        resource_id: String,
        space_id: String,
    },
}
impl GenUiNode {
    pub fn id(&self) -> &str {
        match self {
            Self::Stack { id, .. }
            | Self::Text { id, .. }
            | Self::Button { id, .. }
            | Self::Stat { id, .. }
            | Self::Resource { id, .. }
            | Self::DocumentEditor { id, .. } => id,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenUiView {
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub genui_version: u32,
    pub root: GenUiNode,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum SpaceRole {
    Reader,
    Editor,
    Manager,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceGrant {
    pub id: String,
    pub role: SpaceRole,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Principal {
    pub id: String,
    pub spaces: Vec<SpaceGrant>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OperationDescription {
    pub id: String,
    pub description: String,
    pub mutates: bool,
    pub input_schema: JsonObject,
    pub output_schema: JsonObject,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DescribeResult {
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub genui_version: u32,
    pub principal: Principal,
    pub operations: Vec<OperationDescription>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerResource {
    pub id: String,
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    pub revision: String,
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    pub markdown: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum WorkerOperation {
    #[serde(rename = "notes-insights.summarize")]
    Summarize,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequest {
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    pub invocation_id: String,
    pub operation: WorkerOperation,
    pub resource: WorkerResource,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkerErrorCode {
    InvalidInput,
    Internal,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerError {
    pub code: WorkerErrorCode,
    pub message: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerSuccess {
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    pub invocation_id: String,
    pub ok: True,
    pub data: SummarizeResult,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerFailure {
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    pub invocation_id: String,
    pub ok: False,
    pub error: WorkerError,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum WorkerResponse {
    Success(WorkerSuccess),
    Failure(WorkerFailure),
}

// Schema export root groups every cross-lane type; not an HTTP request body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub request: OperationRequest,
    pub response: OperationResponse<Value>,
    pub describe: DescribeResult,
    pub note: Note,
    pub notes_list_input: NotesListInput,
    pub notes_get_input: NotesGetInput,
    pub notes_create_input: NotesCreateInput,
    pub notes_update_input: NotesUpdateInput,
    pub notes_history_input: NotesHistoryInput,
    pub notes_list_result: NotesListResult,
    pub note_result: NoteResult,
    pub notes_history_result: NotesHistoryResult,
    pub plugins_list_input: PluginsListInput,
    pub plugin_input: PluginInput,
    pub plugins_list_result: PluginsListResult,
    pub plugin_result: PluginResult,
    pub plugin_preview_result: PluginPreviewResult,
    pub summarize_result: SummarizeResult,
    pub worker_request: WorkerRequest,
    pub worker_response: WorkerResponse,
}
