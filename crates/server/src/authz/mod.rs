//! Host-owned operation policy, applied before dispatch and retry replay.
//!
//! A bounded table scan selects the policy; the directory then reads current
//! capability rows. The stored implementation runs on the auth worker. Denial messages allocate
//! only when constructing the existing wire error. Resource IDs are always bound
//! to the authorized space by the storage APIs; possession of an ID grants nothing.

use std::borrow::Cow;

use calternal_core::JsonObject;

use crate::auth::{Directory, Identity};
use calternal_types::{Capability, CapabilitySet, Error, SpaceId};

/// Input scope whose space must be authorized before any resource lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Account-scoped operation; the stored handler verifies account existence.
    Account,
    /// The input's `space_id` is the complete access boundary.
    Space,
    /// The global item resolver derives the owning space before policy admission.
    /// Missing membership and cross-space IDs return the same not-found error.
    Item,
    /// The job store derives the job's owning space before policy admission; a job
    /// the caller cannot see answers exactly like a missing one.
    Job,
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
    /// The caller must be the job's starting account or hold `plugins.manage`.
    JobOwner,
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

/// Space and secrets host binding policies; account-input policies are derived
/// from core by [`policy`]. new bindings must add a policy and matrix row.
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
        scope: Scope::Item,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("notes.history"),
        required: Capability::SpaceRead,
        scope: Scope::Item,
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
        scope: Scope::Item,
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
    Policy {
        operation: Cow::Borrowed("jobs.get"),
        required: Capability::SpaceRead,
        scope: Scope::Job,
        constraint: Constraint::None,
    },
    // Cancelling also requires being the job's starting account or plugins.manage.
    Policy {
        operation: Cow::Borrowed("jobs.cancel"),
        required: Capability::PluginsUse,
        scope: Scope::Job,
        constraint: Constraint::JobOwner,
    },
    // Secrets are owned by the caller's account or, with `space_id`, by a space.
    // `required` applies only to the space case; `crate::secrets` checks it.
    Policy {
        operation: Cow::Borrowed("secrets.request"),
        required: Capability::PluginsUse,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("secrets.list"),
        required: Capability::SpaceRead,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
    Policy {
        operation: Cow::Borrowed("secrets.revoke"),
        required: Capability::SpaceSettings,
        scope: Scope::Account,
        constraint: Constraint::None,
    },
];

/// Find an explicit host policy using a bounded, allocation-free scan.
/// Plugin rows are looked up through [`crate::plugins::Registry::policy`].
pub fn policy(operation: &str) -> Option<&'static Policy> {
    static ACCOUNT: std::sync::OnceLock<Vec<Policy>> = std::sync::OnceLock::new();
    let account = ACCOUNT.get_or_init(|| {
        calternal_core::account_input_operations()
            .map(|id| Policy {
                operation: Cow::Borrowed(id),
                required: Capability::SpaceRead,
                scope: Scope::Account,
                constraint: Constraint::None,
            })
            .collect()
    });
    POLICIES
        .iter()
        .chain(account)
        .chain(agent_policies())
        .find(|p| p.operation == operation)
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
    ) -> Result<(), Error> {
        if self.scope == Scope::Account {
            return Ok(());
        }
        let space = input
            .get("space_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::invalid_input("input does not match operation schema"))?;
        // A malformed ID names no space: the same answer as one the caller cannot see.
        let space = SpaceId::parse(space).map_err(|_| self.hidden())?;
        self.authorize_space(directory, principal, space)
    }

    /// The not-found answer for this scope, identical for absent and inaccessible targets.
    fn hidden(&self) -> Error {
        match self.scope {
            Scope::Item => Error::not_found("item not found"),
            Scope::Job => Error::not_found("job not found"),
            _ => Error::not_found("space not found"),
        }
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
        space: SpaceId,
    ) -> Result<(), Error> {
        self.check(directory.capabilities(principal.id, space))
    }

    /// Applies this policy to an already looked-up capability set (`None`: no
    /// membership), so one lookup can serve admission. Cost: a few bit tests.
    pub fn check(&self, granted: Option<CapabilitySet>) -> Result<(), Error> {
        match granted {
            None => Err(self.hidden()),
            Some(set)
                if self.scope == Scope::Item
                    && !set.contains(Capability::SpaceRead)
                    && !set.contains(self.required) =>
            {
                Err(Error::not_found("item not found"))
            }
            Some(set) if !set.contains(self.required) => {
                // Preserve the development wire error text for existing clients.
                Err(Error::forbidden("insufficient role for this space"))
            }
            Some(_) => Ok(()),
        }
    }
}

/// Policies for the manual-provider seam, derived from its operation declarations.
/// The turn dispatcher additionally checks the actual job owner, current space
/// and fresh-person proof. Built once, then borrowed without allocation.
pub fn agent_policies() -> &'static [Policy] {
    static ROWS: std::sync::OnceLock<Vec<Policy>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        calternal_core::AGENT_OPERATIONS
            .iter()
            .map(|(id, _, _)| Policy {
                operation: Cow::Borrowed(*id),
                required: if *id == "agent.events" {
                    Capability::SpaceRead
                } else {
                    Capability::PluginsUse
                },
                scope: if matches!(*id, "agent.run" | "agent.check") {
                    Scope::Space
                } else {
                    Scope::Account
                },
                constraint: Constraint::None,
            })
            .collect()
    })
}
