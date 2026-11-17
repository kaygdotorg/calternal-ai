//! Wire contracts for stored space directory operations.
use crate::SpaceRole;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Empty input for account-scoped space listing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpacesListInput {}
/// Input for creation; the authenticated account becomes owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpacesCreateInput {
    /// Display name, with the same length and control constraints as a title.
    pub name: String,
}
/// Input for members, leave and directory-record deletion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceInput {
    /// Space UUID; names are never accepted in its place.
    pub space_id: String,
}
/// Target account for removal or explicit ownership transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceMemberInput {
    /// Space UUID; names are never accepted in its place.
    pub space_id: String,
    /// Stable target account UUID, independently allocated from the space.
    pub account_id: String,
}
/// Space metadata and the caller's current effective role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct StoredSpace {
    /// Independent stable UUIDv7 space identity.
    pub id: String,
    /// Display name, never an authority key.
    pub name: String,
    /// Account UUID of the sole owner.
    pub owner: String,
    /// Creation time in Unix milliseconds.
    pub created: i64,
    /// Caller role, including the effective Owner role.
    pub role: SpaceRole,
    /// Whether this is the personal space provisioned for the caller.
    pub personal: bool,
}
/// Account's visible spaces; server administration does not add memberships.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpacesListResult {
    /// Current memberships only.
    pub spaces: Vec<StoredSpace>,
}
/// Newly created space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceResult {
    /// Metadata for the new owned space.
    pub space: StoredSpace,
}
/// Account membership in a space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceMember {
    /// Member's stable account UUID.
    pub account_id: String,
    /// Current account display name.
    pub name: String,
    /// Current directly granted role.
    pub role: SpaceRole,
}
/// Member directory visible to a current space reader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceMembersResult {
    /// Direct account grants on this page. Exactly one Owner has a direct grant.
    pub members: Vec<SpaceMember>,
    /// Direct group grants, including names and active member counts.
    pub groups: Vec<crate::SpaceGroupMember>,
    /// Continue after the last grant, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}
/// A completed membership mutation or directory-record deletion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct SpaceChangedResult {}
