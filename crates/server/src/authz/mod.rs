//! Host-owned operation policy, applied before dispatch and retry replay.
//!
//! A bounded table scan selects the policy; the directory then reads current
//! capability rows. The stored implementation runs on the auth worker. Denial messages allocate
//! only when constructing the existing wire error. Resource IDs are always bound
//! to the authorized space by the storage APIs; possession of an ID grants nothing.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::borrow::Cow;

use calternal_core::JsonObject;

pub mod capabilities;

pub use capabilities::{BUILTIN_ROLES, Capability, CapabilitySet, RoleSet};

use crate::auth::{Directory, Identity};
use crate::error::HostError;

/// Input scope whose space must be authorized before any resource lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Account-scoped operation; the stored handler verifies account existence.
    Account,
    /// The input's `space_id` is the complete access boundary.
    Space,
    /// The global item resolver derives the owning space before policy admission.
    /// Missing membership and cross-space IDs return the same not-found error.
    Note,
}

/// Additional state-dependent constraints enforced inside an authorized handler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// No extra authority-bearing state beyond space membership and resource scope.
    None,
    /// Manifest-derived plugin operation: invocation requires an enabled plugin, every
    /// brokered host call rechecks it, and delivery requires unchanged
    /// enablement/generation plus a fresh directory capability check.
    PluginInvocation,
}

/// Immutable policy associated with one operation binding: a host row from
/// [`POLICIES`] or a row derived from a registered plugin manifest.
/// Cloning a host row copies a borrowed name; a plugin row clones its owned name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Policy {
    /// Exact operation identifier, matching discovery and the handler binding.
    pub operation: Cow<'static, str>,
    /// Single capability required from the current directory, including on retries.
    pub required: Capability,
    /// Input scope and the required storage resource binding.
    pub scope: Scope,
    /// Extra input-dependent checks retained by the handler.
    pub constraint: Constraint,
}

/// Exhaustive host binding policies; new bindings must add a policy and matrix row.
/// Plugin operations are never listed here: their rows are derived from registered
/// manifests by [`crate::plugins::Registry`], and an operation with neither row fails
/// closed instead of receiving inferred authority.
pub const POLICIES: &[Policy] = &[
    // The resolver derives and checks space.read itself; there is no supplied space.
    Policy {
        operation: Cow::Borrowed("resolve"),
        required: Capability::SpaceRead,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.list"),
        required: Capability::SpacesList,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.create"),
        required: Capability::SpacesCreate,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.members"),
        required: Capability::SpaceRead,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.set_role"),
        required: Capability::MembersManage,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.remove_member"),
        required: Capability::MembersManage,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.leave"),
        required: Capability::SpaceRead,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.transfer"),
        required: Capability::SpaceTransfer,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("spaces.delete"),
        required: Capability::SpaceDelete,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.list"),
        required: Capability::SpaceRead,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.get"),
        required: Capability::SpaceRead,
        scope: Scope::Note,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.history"),
        required: Capability::SpaceRead,
        scope: Scope::Note,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.create"),
        required: Capability::ContentWrite,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.update"),
        required: Capability::ContentWrite,
        scope: Scope::Note,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("plugins.list"),
        required: Capability::SpaceRead,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("plugins.enable"),
        required: Capability::PluginsManage,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("plugins.disable"),
        required: Capability::PluginsManage,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("plugins.preview"),
        required: Capability::SpaceRead,
        scope: Scope::Space,
        constraint: Constraint::None,
    },
];

/// Find an explicit host policy using a bounded, allocation-free scan.
/// Plugin rows are looked up through [`crate::plugins::Registry::policy`].
pub fn policy(operation: &str) -> Option<&'static Policy> {
    POLICIES.iter().find(|p| p.operation == operation)
}

impl Policy {
    /// Check current directory membership before resource access or receipt replay.
    /// Both an unknown space and a missing grant return the same denial. This does
    /// not validate the full wire schema (the transport does that independently).
    /// Successful checks borrow `space_id`; the directory owns lookup costs.
    pub fn authorize(
        &self,
        directory: &impl Directory,
        principal: &Identity,
        input: &JsonObject,
    ) -> Result<(), HostError> {
        if self.scope == Scope::Account {
            return Ok(());
        }
        let space = input
            .get("space_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| HostError::invalid_input("input does not match operation schema"))?;
        self.authorize_space(directory, principal, space)
    }

    /// Recheck the already extracted scope, including before asynchronous delivery.
    /// Borrowing the scope adds no allocation; all authority still comes from a
    /// current directory read rather than the request's authenticated identity.
    /// Missing membership hides both existing and absent spaces with not-found;
    /// an existing member missing the required capability receives forbidden.
    pub fn authorize_space(
        &self,
        directory: &impl Directory,
        principal: &Identity,
        space: &str,
    ) -> Result<(), HostError> {
        match directory.capabilities(&principal.id, space) {
            None if self.scope == Scope::Note => Err(HostError::not_found("item not found")),
            None => Err(HostError::not_found("space not found")),
            Some(set)
                if self.scope == Scope::Note
                    && !set.contains(Capability::SpaceRead)
                    && !set.contains(self.required) =>
            {
                Err(HostError::not_found("item not found"))
            }
            Some(set) if !set.contains(self.required) => {
                // Preserve the development wire error text for existing clients.
                Err(HostError::forbidden("insufficient role for this space"))
            }
            Some(_) => Ok(()),
        }
    }
}
