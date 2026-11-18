//! Sharing operation inputs and discovery metadata.
use crate::{JsonObject, OperationDescription, OperationRisk, validation::decode};
use calternal_types::{AccountId, Error, SpaceId};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

/// A recognized kind and stable item UUID.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SharedItemRef {
    /// The item kind, either notes or collections in this slice.
    pub kind: String,
    /// The stable UUID of the item.
    #[schemars(with = "String")]
    pub id: Uuid,
}

/// Exactly one identified account or server group.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ShareAudience {
    /// A named account.
    Account(ShareAccountAudience),
    /// A group with current membership.
    Group(ShareGroupAudience),
}

/// A named account audience.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareAccountAudience {
    /// The active recipient account UUID.
    #[schemars(with = "String")]
    pub account_id: AccountId,
}

/// A live group audience.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareGroupAudience {
    /// The recipient group UUID.
    #[schemars(with = "String")]
    pub group_id: calternal_types::GroupId,
}

/// Current operations allowed on the shared item.
#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SharePermission {
    /// Read the live item.
    View,
    /// Read and edit the live item.
    Edit,
}

/// Create or replace a named grant.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareGrantInput {
    /// The target item reference.
    pub item: SharedItemRef,
    /// The named person or group receiving access.
    pub audience: ShareAudience,
    /// The permission to grant.
    pub permission: SharePermission,
}

/// Bind a grant UUID to its item.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareGrantTarget {
    /// The target item reference.
    pub item: SharedItemRef,
    /// The independently manageable grant UUID.
    #[schemars(with = "String")]
    pub grant_id: Uuid,
}

/// Read one page of sharing metadata.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareListInput {
    /// The target item reference.
    pub item: SharedItemRef,
    /// An optional continuation issued for this operation and scope.
    pub cursor: Option<String>,
}

/// Search the currently visible directory for audience choices.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareAudiencesInput {
    /// The target item reference.
    pub item: SharedItemRef,
    /// A display-name search prefix or substring, or empty.
    pub query: String,
    /// An optional continuation issued for this operation and scope.
    pub cursor: Option<String>,
}

/// List current received shares for an app or collection kind.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SharesReceivedInput {
    /// The item kind, either notes or collections in this slice.
    pub kind: String,
    /// An optional continuation issued for this operation and scope.
    pub cursor: Option<String>,
}

/// Create a labeled public view link.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareLinkCreateInput {
    /// The target item reference.
    pub item: SharedItemRef,
    /// The display name in the Share sheet.
    pub label: String,
    /// Unix milliseconds. A null value means Never.
    pub expires_at: Option<i64>,
}

/// Update metadata on an existing public link.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareLinkUpdateInput {
    /// The target item reference.
    pub item: SharedItemRef,
    /// The independently manageable grant UUID.
    #[schemars(with = "String")]
    pub grant_id: Uuid,
    /// The display name in the Share sheet.
    pub label: Option<String>,
    /// Unix milliseconds. A null value means Never.
    #[serde(default, deserialize_with = "optional_expiry")]
    pub expires_at: Option<Option<i64>>,
}

fn optional_expiry<'de, D: serde::Deserializer<'de>>(
    value: D,
) -> Result<Option<Option<i64>>, D::Error> {
    Option::deserialize(value).map(Some)
}

/// Set who can manage item links in a space.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareLinkPolicyInput {
    /// The space whose link policy changes.
    #[schemars(with = "String")]
    pub space_id: SpaceId,
    /// Whether the space Editor capability can manage links.
    pub editors_can_manage_links: bool,
}

/// Read one saved Notes collection.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionGetInput {
    /// The saved collection UUID.
    #[schemars(with = "String")]
    pub id: Uuid,
}

/// Create a saved set of notes in one space.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionCreateInput {
    /// Owning space UUID.
    #[schemars(with = "String")]
    pub space_id: SpaceId,
    /// Display title.
    pub title: String,
    /// Explicit same-space note UUIDs.
    #[schemars(with = "Vec<String>")]
    pub members: Vec<Uuid>,
}

/// Replace a saved collection's explicit note list.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionMembersInput {
    /// Target collection UUID.
    #[schemars(with = "String")]
    pub id: Uuid,
    /// Expected current collection revision.
    pub base_revision: String,
    /// Explicit same-space note UUIDs.
    #[schemars(with = "Vec<String>")]
    pub members: Vec<Uuid>,
}

/// An explicit acknowledgement that the recipient has seen a share.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareSeenInput {
    /// The received item reference.
    pub item: SharedItemRef,
}

/// Locally authored edits the server could no longer save to a shared note.
#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShareRescueInput {
    /// The former note title, from the caller's local copy.
    pub title: String,
    /// Only the caller's locally changed Markdown blocks, in document order.
    pub changed_blocks: Vec<String>,
}

/// A host-owned sharing operation declaration. The host owns its resource and
/// policy checks because they vary between people, links and account lists.
pub struct SharingOperation {
    /// Stable wire operation identifier.
    pub id: &'static str,
    /// Whether the operation persists a change.
    pub mutates: bool,
    /// Risk used by the protocol catalog.
    pub risk: OperationRisk,
    description: &'static str,
    schema: fn() -> JsonObject,
    validate_input: fn(&JsonObject) -> Result<(), Error>,
}

fn schema<T: JsonSchema>() -> JsonObject {
    serde_json::from_value(serde_json::to_value(schemars::schema_for!(T)).expect("schema"))
        .expect("schema object")
}

fn validate_input<T: serde::de::DeserializeOwned>(input: &JsonObject) -> Result<(), Error> {
    decode::<T>(input).map(|_| ())
}

macro_rules! operation {
    ($name:literal, $doc:literal, $input:ty, $mutates:literal, $risk:ident) => {
        SharingOperation {
            id: $name,
            mutates: $mutates,
            risk: OperationRisk::$risk,
            description: $doc,
            schema: schema::<$input>,
            validate_input: validate_input::<$input>,
        }
    };
}

/// The bounded operation catalogue for item sharing.
pub const SHARING_OPERATIONS: &[SharingOperation] = &[
    operation!(
        "collections.get",
        "Read a saved notes collection and its current revision",
        CollectionGetInput,
        false,
        Read
    ),
    operation!(
        "collections.create",
        "Save an explicit set of notes in one space",
        CollectionCreateInput,
        true,
        Reversible
    ),
    operation!(
        "collections.set_members",
        "Replace a saved collection's explicit notes",
        CollectionMembersInput,
        true,
        WidensAccess
    ),
    operation!(
        "sharing.mark_seen",
        "Mark a received item share as seen",
        ShareSeenInput,
        true,
        Reversible
    ),
    operation!(
        "sharing.rescue",
        "Save the caller's blocked offline edits as a private note",
        ShareRescueInput,
        true,
        Reversible
    ),
    operation!(
        "sharing.list",
        "List sharing controls and grants for an item",
        ShareListInput,
        false,
        Read
    ),
    operation!(
        "sharing.audiences",
        "Find visible people or groups to share an item with",
        ShareAudiencesInput,
        false,
        Read
    ),
    operation!(
        "sharing.shared_with_me",
        "List items shared with the signed-in account",
        SharesReceivedInput,
        false,
        Read
    ),
    operation!(
        "sharing.grant",
        "Set view or edit access for one named audience",
        ShareGrantInput,
        true,
        WidensAccess
    ),
    operation!(
        "sharing.revoke",
        "Revoke one item grant, including while the item is in Trash",
        ShareGrantTarget,
        true,
        Reversible
    ),
    operation!(
        "sharing.link_create",
        "Issue a read-only public link",
        ShareLinkCreateInput,
        true,
        WidensAccess
    ),
    operation!(
        "sharing.link_update",
        "Change a public link's label or expiry",
        ShareLinkUpdateInput,
        true,
        WidensAccess
    ),
    operation!(
        "sharing.link_copy",
        "Retrieve an authorized public link URL",
        ShareGrantTarget,
        false,
        Read
    ),
    operation!(
        "sharing.set_link_policy",
        "Set the authority needed to manage public links",
        ShareLinkPolicyInput,
        true,
        WidensAccess
    ),
];

/// Find one authenticated sharing operation by its wire name.
pub fn sharing_operation(id: &str) -> Option<&'static SharingOperation> {
    SHARING_OPERATIONS.iter().find(|op| op.id == id)
}

impl SharingOperation {
    /// Decode the typed operation schema before authorization. No storage reads.
    pub fn validate(&self, input: &JsonObject) -> Result<(), Error> {
        (self.validate_input)(input)
    }

    /// Build the wire descriptor from the same declaration consumed by dispatch.
    pub fn describe(&self) -> OperationDescription {
        OperationDescription {
            id: self.id.into(),
            description: self.description.into(),
            mutates: self.mutates,
            risk: self.risk,
            required_capability: None,
            target: None,
            input_schema: (self.schema)(),
            output_schema: schema::<JsonObject>(),
        }
    }
}
