//! Plugin listing/enablement results, the v0 wire manifest projection and the v0
//! portable view tree (GenUI v0) with its statistics result.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
