//! Closed space capability vocabulary and data-only built-in role definitions.
//!
//! Following ADR 0058, checks consume capability sets rather than role names.
//! Dotted names retain the reference app's naming style: `members` scopes its
//! server-wide `people` concept to a space, and `space.settings` replaces
//! `server.settings` at this boundary. Plugin/content names describe new space
//! operations. Read-only plugin operations require `space.read`; `plugins.use`
//! permits the broader use granted to Editors, without changing Reader behavior.
//!
//! Stored directories resolve the role-capability rows on every request.
//! Target and ownership constraints are checked again in the write transaction.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

/// One host-defined space permission; unknown names cannot confer authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Capability {
    /// Read space content and invoke read-only plugin operations.
    SpaceRead = 1 << 0,
    /// Create and update content within the space.
    ContentWrite = 1 << 1,
    /// Use plugins beyond the read-only operations covered by `space.read`.
    PluginsUse = 1 << 2,
    /// Enable, disable and maintain shared space plugins.
    PluginsManage = 1 << 3,
    /// Invite members into the space.
    MembersInvite = 1 << 4,
    /// Manage ordinary members; does not authorize managing Managers or Owner.
    MembersManage = 1 << 5,
    /// Manage Managers, subject to protected ownership invariants.
    ManagersManage = 1 << 6,
    /// Change shared space settings.
    SpaceSettings = 1 << 7,
    /// Transfer ownership subject to the protected transfer protocol.
    SpaceTransfer = 1 << 8,
    /// Delete the space through its protected deletion protocol.
    SpaceDelete = 1 << 9,
    /// List memberships of the authenticated account, without space authority.
    SpacesList = 1 << 10,
    /// Create a space as the authenticated account, without existing membership.
    SpacesCreate = 1 << 11,
}

impl Capability {
    /// Every defined capability, used for coverage and closed-set decoding.
    pub const ALL: [Self; 12] = [
        Self::SpaceRead,
        Self::ContentWrite,
        Self::PluginsUse,
        Self::PluginsManage,
        Self::MembersInvite,
        Self::MembersManage,
        Self::ManagersManage,
        Self::SpaceSettings,
        Self::SpaceTransfer,
        Self::SpaceDelete,
        Self::SpacesList,
        Self::SpacesCreate,
    ];

    /// Stable dotted name suitable for future auth.db role-capability rows.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SpaceRead => "space.read",
            Self::ContentWrite => "content.write",
            Self::PluginsUse => "plugins.use",
            Self::PluginsManage => "plugins.manage",
            Self::MembersInvite => "members.invite",
            Self::MembersManage => "members.manage",
            Self::ManagersManage => "managers.manage",
            Self::SpaceSettings => "space.settings",
            Self::SpaceTransfer => "space.transfer",
            Self::SpaceDelete => "space.delete",
            Self::SpacesList => "spaces.list",
            Self::SpacesCreate => "spaces.create",
        }
    }

    /// Resolve a known persisted name; unknown capabilities fail closed.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.name() == name)
    }
}

/// Allocation-free set of known capabilities, independent of any role name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CapabilitySet(u16);

impl CapabilitySet {
    /// Construct an empty set, which grants no operation authority.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Construct a set from known permissions, including persisted role rows.
    pub const fn new(capabilities: &[Capability]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < capabilities.len() {
            bits |= capabilities[index] as u16;
            index += 1;
        }
        Self(bits)
    }

    /// Test one permission with one bitwise AND and no allocation.
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & capability as u16 != 0
    }

    /// Return a set with this permission removed, preserving all other grants.
    pub const fn without(self, capability: Capability) -> Self {
        Self(self.0 & !(capability as u16))
    }
}

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
