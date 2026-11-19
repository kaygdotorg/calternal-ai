//! Resource ownership gives no access to a space or machine.

use crate::{AccountId, GroupId};
use serde::{Deserialize, Serialize};

/// Protected owner of accounted resources for a personal or shared space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum ResourceOwner {
    /// Resources attributed to one account.
    Account(AccountId),
    /// Resources attributed to a shared space's owner group.
    Group(GroupId),
}
