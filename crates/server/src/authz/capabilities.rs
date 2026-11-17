//! Data-only built-in role definitions over the shared capability vocabulary
//! ([`calternal_types::Capability`]); checks consume capability sets, never role
//! names (ADR 0058).
//!
//! Stored directories resolve the role-capability rows on every request.
//! Target and ownership constraints are checked again in the write transaction.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use calternal_types::{Capability, CapabilitySet};

/// Named role definition; space dispatch consumes only its resolved capability set.
pub struct RoleSet {
    /// Stable built-in role name; Owner is derived from the stored ownership relation.
    pub name: &'static str,
    /// Complete grants, with no rank or implicit role inheritance at lookup time.
    pub capabilities: CapabilitySet,
}

use Capability::*;

/// Built-in role sets in Reader, Editor, Manager, Owner order (not authority ranks).
/// Manager membership administration excludes Managers and Owner; future member
/// handlers must additionally check the target, rather than infer it from this set.
/// Owner includes every Manager grant plus the three owner-only permissions.
pub const BUILTIN_ROLES: [RoleSet; 4] = [
    RoleSet {
        name: "reader",
        capabilities: CapabilitySet::new(&[SpaceRead]),
    },
    RoleSet {
        name: "editor",
        capabilities: CapabilitySet::new(&[SpaceRead, ContentWrite, PluginsUse]),
    },
    RoleSet {
        name: "manager",
        capabilities: CapabilitySet::new(&[
            SpaceRead,
            ContentWrite,
            PluginsUse,
            PluginsManage,
            MembersInvite,
            MembersManage,
            SpaceSettings,
        ]),
    },
    RoleSet {
        name: "owner",
        capabilities: CapabilitySet::new(&[
            SpaceRead,
            ContentWrite,
            PluginsUse,
            PluginsManage,
            MembersInvite,
            MembersManage,
            SpaceSettings,
            ManagersManage,
            SpaceTransfer,
            SpaceDelete,
        ]),
    },
];

/// Adapt legacy wire memberships to built-in data; includes the stored Owner role.
/// This adapter is solely for development fixtures, never an operation check.
pub fn development_role(role: &calternal_core::SpaceRole) -> CapabilitySet {
    use calternal_core::SpaceRole;
    match role {
        SpaceRole::Reader => BUILTIN_ROLES[0].capabilities,
        SpaceRole::Editor => BUILTIN_ROLES[1].capabilities,
        SpaceRole::Manager => BUILTIN_ROLES[2].capabilities,
        SpaceRole::Owner => BUILTIN_ROLES[3].capabilities,
    }
}
