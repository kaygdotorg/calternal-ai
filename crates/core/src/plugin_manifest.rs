//! Data-only manifest v1. Declarations request authority; they never issue grants.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
/// Closed host vocabulary, matching protected host capability names.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum ManifestCapability {
    /// Read authorized space content.
    #[serde(rename = "space.read")]
    SpaceRead,
    /// Write authorized space content.
    #[serde(rename = "content.write")]
    ContentWrite,
    /// Use installed plugins.
    #[serde(rename = "plugins.use")]
    PluginsUse,
    /// Manage shared plugin installations.
    #[serde(rename = "plugins.manage")]
    PluginsManage,
    /// Invite space members.
    #[serde(rename = "members.invite")]
    MembersInvite,
    /// Manage ordinary space members.
    #[serde(rename = "members.manage")]
    MembersManage,
    /// Manage Managers; owner constraints still apply.
    #[serde(rename = "managers.manage")]
    ManagersManage,
    /// Change shared settings.
    #[serde(rename = "space.settings")]
    SpaceSettings,
    /// Explicitly transfer ownership.
    #[serde(rename = "space.transfer")]
    SpaceTransfer,
    /// Delete a space.
    #[serde(rename = "space.delete")]
    SpaceDelete,
    /// List the acting account memberships.
    #[serde(rename = "spaces.list")]
    SpacesList,
    /// Create a space for the acting account.
    #[serde(rename = "spaces.create")]
    SpacesCreate,
}
/// The only broker methods a worker may request.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum HostCallKind {
    /// Read an authorized item.
    #[serde(rename = "read_item")]
    ReadItem,
    /// Write an authorized item.
    #[serde(rename = "write_item")]
    WriteItem,
    /// List an authorized collection.
    #[serde(rename = "list_collection")]
    ListCollection,
    /// Emit an authorized event.
    #[serde(rename = "emit_event")]
    EmitEvent,
    /// Request durable job admission.
    #[serde(rename = "start_job")]
    StartJob,
    /// Request a scoped opaque handle, never secret bytes.
    #[serde(rename = "request_secret_handle")]
    RequestSecretHandle,
}
/// Retry behavior declared by an operation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum ManifestIdempotency {
    /// Read with no durable effect.
    #[serde(rename = "read_only")]
    ReadOnly,
    /// Host receipt required before durable effects.
    #[serde(rename = "request_id")]
    RequestId,
    /// Automatic retry prohibited; reconcile uncertain outcomes.
    #[serde(rename = "never")]
    Never,
}
/// Shell region populated by a component reference.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum ManifestRegion {
    /// App sidebar body.
    #[serde(rename = "sidebar")]
    Sidebar,
    /// Main card.
    #[serde(rename = "main")]
    Main,
    /// Panel in the right card.
    #[serde(rename = "right_card")]
    RightCard,
}
/// Hooks observe or block; rewriting is unavailable.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum ManifestHookMode {
    /// Observe permitted matching operations.
    #[serde(rename = "watch")]
    Watch,
    /// Return proceed, wait or reject under a host deadline.
    #[serde(rename = "block")]
    Block,
}
/// Primitive setting value type.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
pub enum ManifestSettingType {
    /// UTF-8 text.
    #[serde(rename = "string")]
    String,
    /// Boolean value.
    #[serde(rename = "boolean")]
    Boolean,
    /// Finite JSON number.
    #[serde(rename = "number")]
    Number,
}
/// Placement inside a space userdata tree; hidden files belong to the declaring plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged, deny_unknown_fields)]
pub enum ManifestPlacement {
    /// User-visible top-level folder; the host rejects cross-plugin folder claims.
    Visible {
        /// One NFC folder component, with no leading dot or reserved name.
        folder: String,
    },
    /// Plugin-owned files ordinary people need not see.
    Hidden {
        /// Must be true; placement is `.calternal/<plugin-id>/<kind>/` within userdata.
        hidden: bool,
    },
}
/// Kind identity remains independent of plugin identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestKind {
    /// Placement within the selected per-space userdata tree; never an appdata database path.
    pub storage: ManifestPlacement,
    /// True selects userdata-ssd for claimed text; false selects userdata-hdd for other files.
    pub claimed_text: bool,
    /// Lowercase ASCII file extension without the dot (1 to 16 letters or digits).
    /// Claimed-text kinds are Markdown: absent or `md`. Other kinds must declare one;
    /// the host stores their bytes unparsed under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub extension: Option<String>,
    /// Stable kind name used in item addresses.
    pub id: String,
    /// JSON Schema for the kind content; no executable code.
    pub schema: serde_json::Value,
}
/// The single item an operation acts on. The host reads the item ID from the named
/// input field, resolves its owning space with the existence-hiding resolver and
/// authorizes the caller in that space before any plugin code runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestTarget {
    /// Kind of the target item, such as `notes`; it may belong to another plugin.
    pub kind: String,
    /// Top-level input property holding the item ID; the input schema must require it
    /// as a string.
    pub field: String,
}
/// Public operation declaration consumed by host policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestOperation {
    /// Globally unique operation identifier.
    pub id: String,
    /// JSON Schema of operation input.
    pub input_schema: serde_json::Value,
    /// JSON Schema of successful output.
    pub output_schema: serde_json::Value,
    /// Required current host capability.
    pub required_capability: ManifestCapability,
    /// Whether this operation can produce durable effects.
    pub mutates: bool,
    /// Retry and host receipt contract.
    pub idempotency: ManifestIdempotency,
    /// Item the operation targets; absent (or null) for a space-scoped operation,
    /// whose wire input names its space in `space_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target: Option<ManifestTarget>,
}
/// Provisioning template; the host allocates a fresh Collection UUID per enable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestCollection {
    /// Globally unique template identifier, never a Collection address.
    pub id: String,
    /// Kind of items in this collection.
    pub kind: String,
    /// Display name for provisioned collections.
    pub name: String,
}
/// Component reference only; component format is separately versioned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestView {
    /// Stable contribution identifier.
    pub id: String,
    /// Shell region.
    pub region: ManifestRegion,
    /// Registered component identifier; no inline code.
    pub component: String,
}
/// Authorized attention candidates; the shell selects at most three.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestAttention {
    /// Stable attention source identifier.
    pub id: String,
    /// Declared operation providing candidates.
    pub operation: String,
}
/// Typed space setting; secrets use separate handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestSetting {
    /// Stable setting identifier.
    pub id: String,
    /// Primitive type of the default and stored values.
    pub value_type: ManifestSettingType,
    /// Default setting value.
    pub default: serde_json::Value,
}
/// Schedule declaration; durable execution belongs to the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestSchedule {
    /// Stable schedule identifier.
    pub id: String,
    /// Declared operation to invoke.
    pub operation: String,
    /// Schedule expression; activation awaits host scheduler validation.
    pub expression: String,
}
/// Webhook declaration; this does not mount an HTTP route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestWebhook {
    /// Stable webhook identifier.
    pub id: String,
    /// Declared operation receiving authorized webhook input.
    pub operation: String,
}
/// Space-scoped hook enabled by a Manager; never a global interceptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestHook {
    /// Stable hook identifier.
    pub id: String,
    /// Watch or block only.
    pub mode: ManifestHookMode,
    /// Exact operation IDs or namespace patterns ending in .* .
    pub patterns: Vec<String>,
    /// Declared operation implementing the hook.
    pub operation: String,
}
/// Network-free theme asset; protected surfaces cannot be themed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestTheme {
    /// Stable theme identifier.
    pub id: String,
    /// Relative packaged CSS asset; activation requires sealed theme validation.
    pub stylesheet: String,
}
/// Plugin menu read without running plugin code. Every top-level field is required;
/// only the documented optional fields inside declarations may be omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    /// Stable plugin identifier.
    pub id: String,
    /// Human-readable plugin name.
    pub name: String,
    /// Registered icon identifier.
    pub icon: String,
    /// Exact package version.
    pub version: String,
    /// Manifest API version, currently 1.
    pub api_version: u32,
    /// Kinds offered by this plugin.
    pub kinds: Vec<ManifestKind>,
    /// Public operation schemas and policy.
    pub operations: Vec<ManifestOperation>,
    /// Collection provisioning templates.
    pub collections: Vec<ManifestCollection>,
    /// Shell region component references.
    pub views: Vec<ManifestView>,
    /// Attention sources.
    pub attention: Vec<ManifestAttention>,
    /// Typed space settings.
    pub settings: Vec<ManifestSetting>,
    /// Schedule declarations.
    pub schedules: Vec<ManifestSchedule>,
    /// Webhook declarations.
    pub webhooks: Vec<ManifestWebhook>,
    /// Requested secret names only; no values.
    pub secrets: Vec<String>,
    /// Space-scoped watch or block hooks.
    pub hooks: Vec<ManifestHook>,
    /// Packaged theme declarations.
    pub themes: Vec<ManifestTheme>,
    /// Broker calls the worker may request; each still needs live authorization.
    pub host_calls: Vec<HostCallKind>,
}
