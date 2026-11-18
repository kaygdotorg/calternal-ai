//! Plugin listing/enablement results (with the v1 manifest) and generated-UI views.
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
/// Space-scoped activation snapshot returned by `plugins.list`, `plugins.enable` and `plugins.disable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginState {
    /// The plugin's full v1 manifest as registered; declarations grant no authority.
    pub manifest: ManifestV1,
    /// Whether this plugin is currently active in the selected space.
    pub enabled: bool,
    /// Activation generation used to reject results from superseded plugin state.
    pub generation: u32,
}
/// An available contribution resolved for the caller and selected space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolvedPluginSlot {
    /// Owning package identifier; pair with the local contribution ID.
    pub plugin_id: String,
    /// Space activation generation to invalidate stale mounts.
    pub generation: u32,
    /// Validated declaration; it conveys no authority to dispatch or mount code.
    pub contribution: PluginSlotContribution,
}
/// An owned mount available in the caller's selected space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolvedOwnedPluginSlot {
    /// Owner package. Pair it with the local ID from the definition.
    pub plugin_id: String,
    /// Current owner activation generation, used to invalidate a mounted slot.
    pub generation: u32,
    /// The validated declaration, including renderer and capacity constraints.
    pub definition: OwnedPluginSlot,
}
/// Result for `plugins.list`, including registered manifests and activation state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginsListResult {
    /// Registered plugins and their space-scoped activation state.
    pub plugins: Vec<PluginState>,
    /// Available contributions in slot, package ID and local ID order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slots: Option<Vec<ResolvedPluginSlot>>,
    /// Available owned mounts, ordered by owner package and local slot ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub owned_slots: Option<Vec<ResolvedOwnedPluginSlot>>,
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
    /// Generated-UI view, compiled by the host before delivery.
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
    /// Generated-UI view, compiled by the host before delivery.
    pub view: GenUiView,
}
/// A generated-UI view: a program in the GenUI language (`crates/genui`,
/// `packages/genui`) that each client compiles and evaluates with its own native
/// components. The host compiles it before delivery and refuses a view with any
/// diagnostic; actions in it describe operations, which dispatch checks again.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GenUiView {
    /// Generated-UI language version; exactly 2 (version 1 was the retired JSON tree).
    #[schemars(range(min = 2, max = 2))]
    #[ts(type = "2")]
    pub genui_version: u32,
    /// Program source, at most 64 KiB of UTF-8.
    #[schemars(length(max = 65536))]
    pub source: String,
}
