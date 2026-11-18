//! Wire contracts and policy metadata for people, group and invitation operations.
use crate::{
    JsonObject, OperationDescription, OperationRisk, SpaceChangedResult, SpaceInput, SpaceRole,
};
use calternal_types::{Capability, Error};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A direct space grant identifies exactly one person or group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceGrantInput {
    /// Space UUID.
    pub space_id: String,
    /// Account UUID when the grant targets a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Group UUID when the grant targets a group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
}

/// A role assignment to exactly one person or group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceGrantRoleInput {
    /// Space UUID.
    pub space_id: String,
    /// Account UUID when granting access directly to a person.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    /// Group UUID when granting access through current group membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// Requested role. Ownership uses explicit transfer.
    pub role: SpaceRole,
}

/// A group grant visible to current members of the space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct SpaceGroupMember {
    /// The group's stable UUID.
    pub group_id: String,
    /// Current group display name.
    pub name: String,
    /// The role assigned to the group.
    pub role: SpaceRole,
    /// Number of active people in the group.
    #[ts(type = "number")]
    pub member_count: u64,
}

/// An operation scoped to the authenticated account or server with no input fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DirectoryInput {}

/// Maximum rows in a directory page, including an individual group's members.
pub const DIRECTORY_PAGE_SIZE: usize = 50;

/// A continuation for a server directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DirectoryPageInput {
    /// The previous page's cursor. Omit it to start at the first row.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::optional_present"
    )]
    #[ts(optional)]
    #[schemars(with = "String", length(min = 1, max = 524288))]
    pub cursor: Option<String>,
}

/// A continuation scoped to one space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceDirectoryPageInput {
    /// Space UUID.
    pub space_id: String,
    /// The previous page's cursor for this space and operation.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::optional_present"
    )]
    #[ts(optional)]
    #[schemars(with = "String", length(min = 1, max = 524288))]
    pub cursor: Option<String>,
}

/// List groups or continue an individual group's member page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupDirectoryPageInput {
    /// Group UUID when reading one group's members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    /// Use the group list cursor or, with `group_id`, its member cursor.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::optional_present"
    )]
    #[ts(optional)]
    #[schemars(with = "String", length(min = 1, max = 524288))]
    pub cursor: Option<String>,
}

/// A space role and its currently declared capabilities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RoleDefinition {
    /// The stored role name.
    pub name: String,
    /// Stable dotted capability names from the authority catalog.
    pub capabilities: Vec<String>,
}

/// Space role catalog result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct RolesResult {
    /// Role definitions, ordered by name.
    pub roles: Vec<RoleDefinition>,
}

/// The current account's new display name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProfileInput {
    /// Name shown to other people.
    pub name: String,
}

/// The updated profile's public display fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct ProfileResult {
    /// The unchanged account UUID.
    pub account_id: String,
    /// The normalized name.
    pub name: String,
}

/// Target of a server people operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PersonInput {
    /// Account UUID.
    pub account_id: String,
}

/// A person in the authorized server directory.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DirectoryPerson {
    /// The account's stable UUID.
    pub account_id: String,
    /// Current display name.
    pub name: String,
    /// Server role, independent of space access.
    pub role: String,
    /// Whether the account may sign in and exercise its grants.
    pub disabled: bool,
}

/// Server people visible to the operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PeopleResult {
    /// Current accounts, including disabled records.
    pub people: Vec<DirectoryPerson>,
    /// Continuation after the last row, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A completed account disable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DisabledPerson {
    /// Disabled account UUID.
    pub account_id: String,
    /// Always true on a successful disable.
    pub disabled: bool,
}

/// A group's display name on creation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupCreateInput {
    /// Printable name shown to authorized people.
    pub name: String,
}

/// Target group UUID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupInput {
    /// The group's stable UUID.
    pub group_id: String,
}

/// Rename a local group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupRenameInput {
    /// The group's stable UUID.
    pub group_id: String,
    /// Its new name.
    pub name: String,
}

/// One membership in a local group.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct GroupPersonInput {
    /// The group's stable UUID.
    pub group_id: String,
    /// The person's account UUID.
    pub account_id: String,
}

/// A server group and its active members.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DirectoryGroup {
    /// The group's stable UUID.
    pub group_id: String,
    /// Current display name.
    pub name: String,
    /// Local or identity-provider managed.
    pub source: String,
    /// Account UUIDs for each active member.
    pub members: Vec<String>,
    /// Continue these members with this group's UUID, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub members_next_cursor: Option<String>,
}

/// Server group directory result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct GroupsResult {
    /// Groups ordered by UUID.
    pub groups: Vec<DirectoryGroup>,
    /// Continuation after the last row, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Create a single-use invitation to a space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct InviteCreateInput {
    /// Space UUID.
    pub space_id: String,
    /// Role granted on redemption. Owner is never assignable by invitation.
    pub role: SpaceRole,
    /// Absolute Unix milliseconds; omission defaults to seven days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(type = "number")]
    pub expires_at: Option<i64>,
}

/// Identify an invitation within its owning space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct InviteInput {
    /// Space UUID.
    pub space_id: String,
    /// Invitation record UUID.
    pub invite_id: String,
}

/// A created invitation. Use the authorized list to copy the link.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InviteCreated {
    /// Invitation record UUID.
    pub invite_id: String,
    /// Absolute expiry in Unix milliseconds.
    #[ts(type = "number")]
    pub expires_at: i64,
}

/// An invitation visible to an authorized space manager or owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct DirectoryInvite {
    /// Invitation record UUID.
    pub invite_id: String,
    /// Role granted upon successful redemption.
    pub role: SpaceRole,
    /// Creation time in Unix milliseconds.
    #[ts(type = "number")]
    pub created_at: i64,
    /// Expiration time in Unix milliseconds.
    #[ts(type = "number")]
    pub expires_at: i64,
    /// Redemption time in Unix milliseconds, or null before redemption.
    #[ts(type = "number | null")]
    pub used_at: Option<i64>,
    /// Revocation time in Unix milliseconds, or null before revocation.
    #[ts(type = "number | null")]
    pub revoked_at: Option<i64>,
    /// Bearer link, present only for a usable invitation.
    pub link: Option<String>,
}

/// Current invitation directory, including unexpired used and revoked records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InvitesResult {
    /// Authorized invitation records.
    pub invites: Vec<DirectoryInvite>,
    /// Continuation after the last row, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A revoked invitation's public identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct InviteRevoked {
    /// Invitation record UUID.
    pub invite_id: String,
}

/// Redeem a link as the signed-in account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct InviteRedeemInput {
    /// The secret from the invite URL, passed in the HTTP body.
    pub secret: String,
}

macro_rules! operations {
    ($($variant:ident: $id:literal, $description:literal, $cap:ident, $risk:ident, $target:ident, $input:ty => $out:ty;)*) => {
        /// A host people operation with one description, capability, scope and risk.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PeopleOperation {
            $(#[doc = $description] $variant,)*
        }
        impl PeopleOperation {
            /// All people operations in declaration order.
            pub const ALL: &[Self] = &[$(Self::$variant,)*];
            /// Look up metadata by stable wire identifier. Cost: bounded match.
            pub fn from_id(id: &str) -> Option<Self> {
                match id { $($id => Some(Self::$variant),)* _ => None }
            }
            /// Stable operation identifier.
            pub const fn id(self) -> &'static str {
                match self { $(Self::$variant => $id,)* }
            }
            /// Current access required before execution or receipt replay.
            pub const fn capability(self) -> Capability {
                match self { $(Self::$variant => Capability::$cap,)* }
            }
            /// Operation risk used by host discovery and review.
            pub const fn risk(self) -> OperationRisk {
                match self { $(Self::$variant => OperationRisk::$risk,)* }
            }
            /// Scope from which authority is obtained.
            pub const fn target(self) -> PeopleTarget {
                match self { $(Self::$variant => PeopleTarget::$target,)* }
            }
            /// Generate discovery schemas. The host caches the entire catalog.
            pub(crate) fn describe(self) -> OperationDescription {
                match self {
                    $(Self::$variant => OperationDescription {
                        id: $id.into(), description: $description.into(),
                        risk: self.risk(), mutates: self.risk() != OperationRisk::Read,
                        required_capability: Some(self.capability().name().into()),
                        target: Some(self.target()),
                        input_schema: serde_json::from_value(serde_json::to_value(schemars::schema_for!($input)).unwrap()).unwrap(),
                        output_schema: serde_json::from_value(serde_json::to_value(schemars::schema_for!($out)).unwrap()).unwrap(),
                    },)*
                }
            }
            pub(crate) fn validate(self, input: &JsonObject) -> Result<(), Error> {
                match self { $(Self::$variant => { let _: $input = crate::validation::decode(input)?; },)* }
                if matches!(self, Self::AddSpaceMember | Self::SetSpaceRole | Self::RemoveSpaceMember) {
                    SpaceGrantInput::validate_selection(
                        input.get("account_id").is_some_and(|v| v.is_string()),
                        input.get("group_id").is_some_and(|v| v.is_string()),
                    )?;
                }
                validate_target(input)
            }
        }
    };
}

/// Admission boundary for the people operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PeopleTarget {
    /// The server capability directory, independent of space access.
    Server,
    /// The current signed-in account.
    Account,
    /// The space named by the input.
    Space,
    /// The live invitation named by a bearer secret.
    Invitation,
}

operations! {
    ListSpaceMembers: "spaces.members", "List direct account and group grants.", SpaceRead, Read, Space, SpaceDirectoryPageInput => crate::SpaceMembersResult;
    TransferOwnership: "spaces.transfer", "Transfer ownership to a current account in the space.", SpaceTransfer, WidensAccess, Space, crate::SpaceMemberInput => SpaceChangedResult;
    AddSpaceMember: "spaces.add_member", "Grant a person or group access to a space.", MembersInvite, WidensAccess, Space, SpaceGrantRoleInput => SpaceChangedResult;
    SetSpaceRole: "spaces.set_role", "Change a direct account or group grant.", MembersManage, WidensAccess, Space, SpaceGrantRoleInput => SpaceChangedResult;
    RemoveSpaceMember: "spaces.remove_member", "Remove a direct account or group grant.", MembersManage, Irreversible, Space, SpaceGrantInput => SpaceChangedResult;
    ListRoles: "roles.list", "List the space roles and their capabilities.", RolesList, Read, Account, DirectoryInput => RolesResult;
    SetProfile: "account.set_profile", "Change your own display name.", AccountProfile, Reversible, Account, ProfileInput => ProfileResult;
    ListPeople: "people.list", "List server accounts.", PeopleManage, Read, Server, DirectoryPageInput => PeopleResult;
    DisablePerson: "people.disable", "Disable an account and revoke its sessions.", PeopleManage, Irreversible, Server, PersonInput => DisabledPerson;
    ListGroups: "groups.list", "List server groups.", GroupsManage, Read, Server, GroupDirectoryPageInput => GroupsResult;
    CreateGroup: "groups.create", "Create a local group.", GroupsManage, WidensAccess, Server, GroupCreateInput => GroupInput;
    RenameGroup: "groups.rename", "Rename a local group.", GroupsManage, Reversible, Server, GroupRenameInput => GroupInput;
    DeleteGroup: "groups.delete", "Delete a local group and remove its grants.", GroupsManage, Irreversible, Server, GroupInput => GroupInput;
    AddGroupMember: "groups.add_member", "Add a person to a local group.", GroupsManage, WidensAccess, Server, GroupPersonInput => GroupInput;
    RemoveGroupMember: "groups.remove_member", "Remove a person from a local group.", GroupsManage, Irreversible, Server, GroupPersonInput => GroupInput;
    CreateInvite: "invites.create", "Create a single-use space invitation.", MembersInvite, WidensAccess, Space, InviteCreateInput => InviteCreated;
    ListInvites: "invites.list", "List and copy current space invitations.", MembersInvite, Read, Space, SpaceDirectoryPageInput => InvitesResult;
    RevokeInvite: "invites.revoke", "Revoke an unused space invitation.", MembersInvite, Irreversible, Space, InviteInput => InviteRevoked;
    RedeemInvite: "invites.redeem", "Redeem a space invitation as the signed-in account.", InviteRedeem, WidensAccess, Invitation, InviteRedeemInput => SpaceInput;
}

fn validate_target(input: &JsonObject) -> Result<(), Error> {
    // Scope admission hides missing and inaccessible spaces, including names
    // presented as IDs. Leave UUID scope parsing to that boundary.
    if let Some(space) = input.get("space_id").and_then(|v| v.as_str()) {
        crate::validate_id(space)?;
    }
    if let Some(cursor) = input.get("cursor").and_then(|v| v.as_str()) {
        crate::validate_page_cursor(cursor)?;
    }
    for field in ["account_id", "group_id", "invite_id"] {
        if let Some(value) = input.get(field).and_then(|v| v.as_str()) {
            crate::validate_resource_id(value)?;
        }
    }
    if input.get("role").is_some_and(|r| r == "owner") {
        return Err(Error::invalid_input("use explicit ownership transfer"));
    }
    Ok(())
}

impl SpaceGrantInput {
    /// Validate that a change selects just one account or group.
    pub fn validate_selection(account: bool, group: bool) -> Result<(), Error> {
        if account == group {
            return Err(Error::invalid_input(
                "provide exactly one account_id or group_id",
            ));
        }
        Ok(())
    }
}
