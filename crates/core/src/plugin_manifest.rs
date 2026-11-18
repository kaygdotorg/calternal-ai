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
    /// List a bounded page of live items from the calling plugin's own kind.
    #[serde(rename = "list_items_v1")]
    ListItemsV1,
    /// Read the effective value of a setting declared by the calling package.
    #[serde(rename = "read_setting_v1")]
    ReadSettingV1,
    /// Prepare remote asset references from a declared, stored content revision.
    #[serde(rename = "content_prepare_v1")]
    ContentPrepareV1,
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
    /// Durably record a running job's checkpoint, from its run invocation.
    #[serde(rename = "checkpoint_job")]
    CheckpointJob,
    /// Request a scoped opaque handle, never secret bytes.
    #[serde(rename = "request_secret_handle")]
    RequestSecretHandle,
}

/// Version-one worker request for an indexed page of its own kind.
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginItemPageInput {
    /// Kind declared by the calling plugin. The invocation supplies the space.
    pub kind: String,
    /// Exact label filter, using the indexed label key.
    #[ts(optional)]
    pub label: Option<String>,
    /// Opaque continuation returned by this call. Omit it for the first page.
    #[ts(optional)]
    pub cursor: Option<String>,
    /// Number of items, from 1 to 100. Defaults to 100.
    #[ts(optional)]
    pub limit: Option<u16>,
}

/// Indexed fields available without loading the item's body or copying metadata.
#[derive(Debug, Serialize, JsonSchema, TS)]
pub struct PluginItemSummary {
    /// Item UUID in the selected kind and space.
    pub id: String,
    /// Current opaque revision.
    pub revision: String,
    /// UTC commit timestamp.
    pub updated_at: String,
    /// Optional title, limited to 256 Unicode scalars.
    pub title: Option<String>,
}

/// Page of live items in descending update order, then stable item ID order.
#[derive(Debug, Serialize, JsonSchema, TS)]
pub struct PluginItemPageResult {
    /// Bounded summaries in index order.
    pub items: Vec<PluginItemSummary>,
    /// Continuation for the same account, space, package, kind and filter.
    pub next_cursor: Option<String>,
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
#[ts(untagged)]
pub enum ManifestPlacement {
    /// Visible subtree claimed by one plugin, with at most eight safe components.
    VisibleTree {
        /// One safe visible top-level folder.
        folder: String,
        /// Safe directory components below the folder; never a filesystem path.
        subdirectories: Vec<String>,
    },
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
    /// Opt in to indexing the title and Markdown body for space search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub searchable: Option<bool>,
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
    /// Required risk declaration; capability checks still apply in every mode.
    pub risk: crate::OperationRisk,
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
/// Owner of an override for a declared plugin setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ManifestSettingScope {
    /// Each person in the space can have their own value.
    Personal,
    /// Space managers can change the shared value.
    Space,
}

/// Typed plugin setting; secrets use separate handles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestSetting {
    /// Override owner. Omission selects the shared space setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scope: Option<ManifestSettingScope>,
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
/// Host-defined mount point for an activated plugin contribution.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum PluginSlot {
    /// A section in the single navigation sidebar.
    SidebarSection,
    /// Content within a host-managed card or window.
    CardWindow,
    /// Accessory row beside the host-owned composer.
    ComposerRow,
    /// Generated decoration within a message row.
    MessageDecoration,
    /// Bounded panel along a thread edge.
    ThreadEdge,
    /// Section outside protected account controls.
    SettingsSection,
    /// Command discoverable in the host action palette.
    PaletteCommands,
    /// Host-timed notification content.
    Toasts,
}

/// A package-qualified slot. Its namespace cannot address a host surface.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OwnedPluginSlotId {
    /// Registered package that declares the slot.
    pub plugin_id: String,
    /// Local slot ID in that package.
    pub id: String,
}

/// Mount addressed either by the fixed host vocabulary or a package-qualified name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
#[serde(untagged)]
pub enum PluginSlotTarget {
    /// One extensible host mount point.
    Host(PluginSlot),
    /// A slot defined by an installed package.
    Owned(OwnedPluginSlotId),
}

/// Operation-backed contribution types available to a package-owned mount.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum OwnedPluginSlotKind {
    /// A read-only operation yielding a portable generated view.
    Generated,
    /// An operation invoked through the host's normal command and approval path.
    Command,
}

/// One package-owned slot, with explicit visibility, accepted renderers and capacity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OwnedPluginSlot {
    /// Stable local name; the host obtains the owner ID from the enclosing manifest.
    pub id: String,
    /// Space capability a caller must hold to resolve this slot.
    pub required_capability: ManifestCapability,
    /// Nonempty set of operation-backed renderer types. Duplicates are invalid.
    #[schemars(length(min = 1, max = 2))]
    pub accepts: Vec<OwnedPluginSlotKind>,
    /// Maximum visible contributions per listing, between 1 and 32.
    #[schemars(range(min = 1, max = 32))]
    pub max_contributions: u8,
}

/// A portable operation-backed contribution or a reference to a sealed web bundle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub enum PluginSlotRenderer {
    /// Resolve through a declared read-only operation returning a generated view.
    Generated {
        /// Plugin-local operation declaration to dispatch with current authority.
        operation: String,
    },
    /// Advertise a packaged entry point for a sealed web renderer.
    Sealed {
        /// Relative HTML asset path. The prototype does not serve or mount this asset.
        asset: String,
    },
    /// Register a named palette action targeting a declared operation.
    Command {
        /// Operation to dispatch with current authority and normal approval checks.
        operation: String,
    },
}

/// One plugin-owned contribution that the host can resolve without running code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PluginSlotContribution {
    /// Stable identifier unique within this manifest.
    pub id: String,
    /// Accessible name and visible label, bounded to 128 UTF-8 bytes.
    pub label: String,
    /// Extensible host mount or package-owned target, independent of the renderer.
    pub slot: PluginSlotTarget,
    /// Renderer or command to resolve.
    pub renderer: PluginSlotRenderer,
}

/// Network authority requested by a package. Each declaration has its own
/// approval meaning and is subject to the live space activation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestNetwork {
    /// Anonymous remote assets from stored content. Omit for no proxy access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub content_fetch: Option<ManifestContentFetch>,
}

/// Anonymous content proxy scope shown during trusted installation review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestContentFetch {
    /// Host-enforced media and transport policy.
    pub profile: ContentFetchProfile,
    /// Up to 32 distinct text sources owned by the declaring package.
    #[schemars(length(min = 1, max = 32))]
    pub sources: Vec<ContentFetchSource>,
}

/// Host-owned remote media policy. Each additional profile requires review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContentFetchProfile {
    /// Anonymous HTTPS GET for allowed image and font formats, excluding SVG.
    ImagesFonts,
}

/// A stored field the broker may read to derive proxy references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentFetchSource {
    /// The declaring package's own text kind.
    pub kind: String,
    /// The source field interpreted as HTML.
    pub field: ContentFetchField,
}

/// Worker request to sanitize an exact stored content revision. Authority
/// comes from invocation admission and the plugin's content declaration.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentPrepareInput {
    /// Plugin-owned kind named in its proxy profile.
    pub kind: String,
    /// Item UUID.
    pub item_id: String,
    /// Current opaque revision received from the item store.
    pub revision: String,
    /// Field containing the HTML source.
    pub field: ContentFetchField,
}

/// Prepared content reference for the acting person, valid briefly.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentPrepareResult {
    /// Opaque frame ID. Frame delivery checks the current viewer and source.
    pub frame_id: String,
}

/// Read a prepared sealed frame as the person it was created for.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentFrameAddress {
    /// Space UUID.
    pub space_id: String,
    /// Short-lived frame ID from content preparation.
    pub frame_id: String,
}

/// Trusted client data for mounting a sealed frame and for external navigation.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentFrameView {
    /// Relative trusted frame URL. Mount with an empty iframe sandbox.
    pub frame_url: String,
    /// External targets in the order of the frame's navigation references.
    /// Open only through the host's explicit navigation action.
    pub links: Vec<String>,
    /// Current remote-content preference for the viewer.
    pub preference: ContentFetchPreference,
}

/// One person's policy for remote images and fonts in a space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContentFetchPreference {
    /// Request an explicit decision before downloading remote assets.
    Ask,
    /// Fetch eligible assets at ingest, with the older-content fallback.
    #[default]
    Always,
    /// Show placeholders for remote assets.
    Never,
}

/// Read the viewer's remote content policy in one space.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentFetchPreferenceAddress {
    /// Space UUID. The authenticated caller supplies the account identity.
    pub space_id: String,
}

/// Set the caller's own remote content policy in one space.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ContentFetchPreferenceEdit {
    /// Space UUID. The authenticated caller supplies the account identity.
    pub space_id: String,
    /// New choice applied at the next fetch and asset delivery.
    pub preference: ContentFetchPreference,
}

/// Stored source fields supported by the content proxy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ContentFetchField {
    /// The item's UTF-8 text body.
    Text,
}

/// Plugin menu read without running plugin code. Only documented optional
/// fields may be omitted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestV1 {
    /// Optional outbound network declarations. Omit for no network access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub network: Option<ManifestNetwork>,
    /// Optional commands, extra phrases and Settings sections for the palette.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub palette: Option<crate::PaletteDeclaration>,
    /// Machine-side provider adapter; omitted by plugins that offer no provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub provider: Option<ManifestProvider>,
    /// Optional contributions to host slots, validated without running code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub slots: Option<Vec<PluginSlotContribution>>,
    /// Up to 32 mounts owned by this package, resolved only while enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 32))]
    #[ts(optional)]
    pub owned_slots: Option<Vec<OwnedPluginSlot>>,
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
    /// Kinds offered by this plugin. An omitted list declares no kinds.
    #[serde(default)]
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

/// Machine-side provider declaration; executable is installed in the sandbox
/// image or account home, never downloaded or run by the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ManifestProvider {
    /// Version of the bounded stdio contract, currently one.
    pub protocol_version: u32,
    /// Basename resolved inside the sandbox's PATH; no shell, path or arguments.
    pub executable: String,
    /// Whether the executable implements the private `--sign-in` mode.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sign_in: bool,
    /// Whether the adapter implements the private, stateless `--title` command.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub title_generation: bool,
    /// Default installed-CLI model identifier, at most 128 UTF-8 bytes.
    pub default_model: String,
    /// Models offered by this adapter, validated and cached with the manifest.
    pub models: Vec<ProviderModel>,
}

/// One selectable provider model and the explicit reasoning levels it accepts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderModel {
    /// Stable model identifier passed to the adapter.
    pub id: String,
    /// Short display name for the model picker.
    pub name: String,
    /// Ordered reasoning levels. Empty means the model accepts no explicit level.
    pub reasoning_levels: Vec<String>,
}
