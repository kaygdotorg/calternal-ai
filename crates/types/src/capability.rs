//! The closed space capability vocabulary (ADR 0058: checks consume capability
//! sets, never role names).
//!
//! Dotted names follow the existing app's style: `members` scopes its
//! server-wide `people` concept to a space, and `space.settings` replaces
//! `server.settings`. Read-only plugin operations need `space.read`;
//! `plugins.use` is the broader use granted to Editors.

/// One host-defined space permission; an unknown stored name confers nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Every defined capability, for coverage tests and closed-set decoding.
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

    /// The stored and wire name, as written in `auth.db` role rows and manifests.
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

    /// Resolves a stored name; an unknown name is `None` and grants nothing.
    /// Cost: linear scan of 12 static names, no allocation.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// Allocation-free set of capabilities, independent of any role name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct CapabilitySet(u16);

impl CapabilitySet {
    /// The empty set, which grants no operation.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// A set holding exactly `capabilities`.
    pub const fn new(capabilities: &[Capability]) -> Self {
        let mut bits = 0;
        let mut index = 0;
        while index < capabilities.len() {
            bits |= capabilities[index] as u16;
            index += 1;
        }
        Self(bits)
    }

    /// Whether the set grants `capability`. Cost: one bitwise AND.
    pub const fn contains(self, capability: Capability) -> bool {
        self.0 & capability as u16 != 0
    }

    /// This set plus `capability`.
    pub const fn with(self, capability: Capability) -> Self {
        Self(self.0 | capability as u16)
    }

    /// This set without `capability`, keeping every other grant.
    pub const fn without(self, capability: Capability) -> Self {
        Self(self.0 & !(capability as u16))
    }

    /// The granted capabilities in [`Capability::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL.into_iter().filter(move |c| self.contains(*c))
    }
}

impl FromIterator<Capability> for CapabilitySet {
    fn from_iter<I: IntoIterator<Item = Capability>>(iter: I) -> Self {
        iter.into_iter().fold(Self::empty(), Self::with)
    }
}
