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
/// Maximum worker response size in bytes (256 KiB).
pub const MAX_WORKER_OUTPUT_BYTES: usize = 256 * 1024;
/// Default loopback development HTTP port; not an external deployment endpoint.
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
/// Metadata returned by `notes.list`; excludes Markdown to bound listing payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteSummary {
    /// Stable UUID note identity; access is checked against the accompanying space.
    pub id: String,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Current snapshot returned by `notes.get`, `notes.create` and `notes.update`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Note {
    /// Stable UUID note identity; access is checked against the accompanying space.
    pub id: String,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Immutable historical snapshot in a `notes.history` page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteVersion {
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Input for `notes.list`; the server independently checks access to the selected space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListInput {
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
}
/// Input for the common `/<kind>/<id>` address resolver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolveInput {
    /// Open item-kind identifier, independent of its rendering plugin; future kinds need no enum change.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub kind: String,
    /// Globally unique item UUID; possession grants no authority.
    pub id: String,
}
/// Current owning space and effective capabilities for a visible address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolveResult {
    /// Resolved item kind.
    pub kind: String,
    /// Stable item UUID.
    pub id: String,
    /// Owning space UUID, independent of account identity.
    pub space_id: String,
    /// Current effective capability names; informational, rechecked on use.
    pub access: Vec<String>,
}
/// Input for `notes.get` and `notes-insights.summarize`; a resource ID alone grants no access.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesGetInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
}
/// Input for `notes.create`; creates a new resource in an authorized space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesCreateInput {
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
}
/// Input for `notes.update`; the base revision must match before the write can commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesUpdateInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
    /// Expected opaque current revision, 1..256 UTF-8 bytes; mismatch produces a conflict.
    #[schemars(length(min = 1, max = 256))]
    pub base_revision: String,
    /// Optional replacement title: nonblank, at most 256 Unicode scalars, no controls; omission preserves the title and null is rejected.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
}
/// Input for `notes.history`; bounded pagination uses opaque server-issued cursors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
    /// Optional page size, 1..100 snapshots; omission selects the server default, null is rejected.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "u32")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    /// Optional opaque pagination token, 1..256 UTF-8 bytes; omit for the first page, never send null.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    #[schemars(length(min = 1, max = 256))]
    pub cursor: Option<String>,
}
/// Result for `notes.list`, containing at most 100 current note summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListResult {
    /// At most 100 current summaries, ordered by descending update timestamp then resource ID.
    pub notes: Vec<NoteSummary>,
}
/// Result for `notes.get`, `notes.create` and `notes.update`, containing one current snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteResult {
    /// Current note snapshot visible to the authenticated caller.
    pub note: Note,
}
/// Result for `notes.history`; snapshots are paginated without exposing storage paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryResult {
    /// Historical snapshots in descending server sequence order, bounded by the requested page size.
    pub versions: Vec<NoteVersion>,
    /// Opaque token for the next history page; null marks the end.
    #[serde(deserialize_with = "required_nullable_string")]
    #[schemars(schema_with = "nullable_string_schema")]
    pub next_cursor: Option<String>,
}
/// Input for `plugins.list`; state is resolved within the authorized space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsListInput {
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
}
/// Input for `plugins.enable`, `plugins.disable` and `plugins.preview`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginInput {
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Registered trusted-plugin identifier, not a package path or source of authority.
    pub plugin_id: String,
}
/// Exact plugin dependency declaration in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginDependency {
    /// Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.
    pub id: String,
    /// Nonempty plugin version string; dependencies declare the exact version they require.
    pub version: String,
}
/// Read-only extension operation declaration in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginOperation {
    /// Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.
    pub id: String,
    /// Human-readable description for discovery; carries no execution authority.
    pub description: String,
    /// Whether this operation changes durable state; extension declarations are constrained to false.
    pub mutates: False,
}
/// Versioned host contribution slots in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum ExtensionPoint {
    /// Contribute to the detail view of an authorized note.
    #[serde(rename = "notes.detail")]
    NotesDetail,
}
/// Portable view contribution in manifests returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginView {
    /// Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.
    pub id: String,
    /// Host slot at which this contribution may be mounted.
    pub extension_point: ExtensionPoint,
    /// Validated portable presentation with bounded node, depth, text and serialized sizes.
    pub view: GenUiView,
}
/// Versioned trusted-plugin metadata exposed by `plugins.list`, `plugins.enable` and `plugins.disable`. Declarations do not themselves grant authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    /// Stable plugin or contribution identifier, 1..80 ASCII letters, digits or `_.:-`.
    pub id: String,
    /// Nonempty plugin version string; dependencies declare the exact version they require.
    pub version: String,
    /// Supported plugin contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub api_version: u32,
    /// Nonempty plugin display name for discovery.
    pub name: String,
    /// Human-readable description for discovery; carries no execution authority.
    pub description: String,
    /// Required plugin IDs and exact versions; no ambient access is inherited from a dependency.
    pub dependencies: Vec<PluginDependency>,
    /// Declared capability names; actual authority remains host-controlled.
    pub capabilities: Vec<String>,
    /// Declared/discovered operation contracts; dispatch still checks current permissions.
    pub operations: Vec<PluginOperation>,
    /// Portable contributions registered for supported host extension points.
    pub views: Vec<PluginView>,
}
/// Space-scoped activation snapshot returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginState {
    /// Registered trusted-plugin metadata; enabling it does not replace host enforcement.
    pub manifest: PluginManifest,
    /// Whether this plugin is currently active in the selected space.
    pub enabled: bool,
    /// Activation generation used to reject results from superseded plugin state.
    pub generation: u32,
}
/// Result for `plugins.list`, including registered manifests and activation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsListResult {
    /// Registered plugins and their space-scoped activation state.
    pub plugins: Vec<PluginState>,
}
/// Result for `plugins.enable` and `plugins.disable` after the state transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginResult {
    /// Plugin state after the requested activation transition.
    pub plugin: PluginState,
}
/// Result for `plugins.preview`, computed from fixture data rather than private note content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginPreviewResult {
    /// Validated portable presentation with bounded node, depth, text and serialized sizes.
    pub view: GenUiView,
    /// Always true: this view was prepared with disposable fixture data.
    pub preview: True,
}
/// Read-only Markdown statistics returned by `notes-insights.summarize`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Stats {
    /// Count of Unicode letter/number word runs, allowing internal straight or curly apostrophes.
    pub words: u32,
    /// Count of Unicode scalar values, not UTF-8 bytes or grapheme clusters.
    pub characters: u32,
    /// Logical line count; CRLF counts as one break, a trailing break adds no empty line, and empty text has zero lines.
    pub lines: u32,
}
/// Result for `notes-insights.summarize`, combining statistics with a portable presentation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SummarizeResult {
    /// Counts computed from the supplied Markdown snapshot without modifying it.
    pub stats: Stats,
    /// Validated portable presentation with bounded node, depth, text and serialized sizes.
    pub view: GenUiView,
}
/// Button operation call in `plugins.preview` and `notes-insights.summarize` views or plugin manifests; dispatch still checks current authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenUiAction {
    /// Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.
    pub operation: String,
    /// Operation-specific JSON object; must pass the selected operation schema before dispatch.
    pub input: JsonObject,
}
/// Portable declarative nodes in plugin previews and `notes-insights.summarize` results. Validation bounds depth to 8, nodes to 128 and enforces unique IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[serde(deny_unknown_fields)]
pub enum GenUiNode {
    /// Ordered container of child nodes.
    Stack {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Ordered child nodes; the complete tree must remain within depth 8 and 128 nodes.
        children: Vec<GenUiNode>,
    },
    /// Literal non-interactive text.
    Text {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Literal display text, limited to 8 KiB of UTF-8.
        text: String,
    },
    /// Labeled action dispatched through the operation boundary.
    Button {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Accessible display label, 1..256 Unicode scalars.
        label: String,
        /// Declared operation to dispatch on activation; no authority is embedded in the action.
        action: GenUiAction,
    },
    /// Labeled statistic rendered as text.
    Stat {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Accessible display label, 1..256 Unicode scalars.
        label: String,
        /// Display-formatted statistic value; rendering must treat it as text.
        value: String,
    },
    /// Link to a stable resource resolved through current access checks.
    Resource {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Stable UUID resource target; resolving it requires access to its space.
        resource_id: String,
        /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
        space_id: String,
        /// Accessible display label, 1..256 Unicode scalars.
        label: String,
    },
    /// Host-owned document editor bound to an authorized resource.
    DocumentEditor {
        /// Node identifier, 1..80 ASCII letters, digits or `_.:-`; unique across the complete view.
        id: String,
        /// Stable UUID resource target; resolving it requires access to its space.
        resource_id: String,
        /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
        space_id: String,
    },
}
impl GenUiNode {
    /// Borrow the node identity without allocation; tree validation enforces its format and uniqueness.
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
/// Versioned portable view in plugin manifests, previews and `notes-insights.summarize`. Validation limits JSON to 64 KiB and string values to 32 KiB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenUiView {
    /// Portable view contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub genui_version: u32,
    /// Root node of the bounded portable tree; IDs are unique across the tree.
    pub root: GenUiNode,
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
    /// Stable space UUID in stored directories; fixture directories may expose aliases.
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
/// Authorized note snapshot supplied to the `notes-insights.summarize` worker; contains no ambient storage access.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerResource {
    /// Stable UUID note identity; access is checked against the accompanying space.
    pub id: String,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
}
/// Worker protocol operations supported by the trusted `notes-insights.summarize` process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub enum WorkerOperation {
    /// Compute read-only Markdown statistics for an authorized snapshot.
    #[serde(rename = "notes-insights.summarize")]
    Summarize,
}
/// Single invocation sent to the `notes-insights.summarize` worker over standard input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerRequest {
    /// Transport contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    /// Host-issued worker correlation identifier; must match the expected invocation.
    pub invocation_id: String,
    /// Versioned operation identifier; operation envelopes and actions require 1..128 UTF-8 bytes.
    pub operation: WorkerOperation,
    /// Authorized immutable note snapshot delivered to this worker invocation.
    pub resource: WorkerResource,
}
/// Safe worker failure categories for `notes-insights.summarize`; transport failures are handled by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum WorkerErrorCode {
    /// Input violates the contract (HTTP 400 in operation errors).
    InvalidInput,
    /// An internal failure occurred; expose only safe diagnostics (HTTP 500 in operation errors).
    Internal,
}
/// Safe failure payload from the `notes-insights.summarize` worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerError {
    /// Stable machine-readable failure category; callers should branch on this rather than message text.
    pub code: WorkerErrorCode,
    /// Human-readable safe diagnostic; never include credentials or raw private exception details.
    pub message: String,
}
/// Successful `notes-insights.summarize` worker envelope correlated to one invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerSuccess {
    /// Transport contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    /// Host-issued worker correlation identifier; must match the expected invocation.
    pub invocation_id: String,
    /// Exact boolean discriminant required by this envelope; the opposite value cannot deserialize.
    pub ok: True,
    /// Operation-specific successful result payload.
    pub data: SummarizeResult,
}
/// Failed `notes-insights.summarize` worker envelope correlated to one invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct WorkerFailure {
    /// Transport contract version; exactly 1 in this preview.
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub protocol_version: u32,
    /// Host-issued worker correlation identifier; must match the expected invocation.
    pub invocation_id: String,
    /// Exact boolean discriminant required by this envelope; the opposite value cannot deserialize.
    pub ok: False,
    /// Safe structured failure returned instead of successful data.
    pub error: WorkerError,
}
/// Discriminated response from the `notes-insights.summarize` worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum WorkerResponse {
    /// A correlated successful result with its exact true discriminant.
    Success(WorkerSuccess),
    /// A correlated failure with its exact false discriminant.
    Failure(WorkerFailure),
}

// Schema export root groups every cross-lane type; not an HTTP request body.
/// Schema export root grouping all operation and worker types; never an HTTP request body.
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
    /// Schema export representative for `worker_request`; not a field accepted by operation dispatch.
    pub worker_request: WorkerRequest,
    /// Schema export representative for `worker_response`; not a field accepted by operation dispatch.
    pub worker_response: WorkerResponse,
}
