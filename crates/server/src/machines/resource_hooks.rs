//! Operator grants and short-lived decisions for hosted placement.

use calternal_types::{AccountId, Error, SpaceId};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::{Lease, LeaseState, MachineService, State, compute, db, runtime, sql};

pub(super) const MIGRATION: &str = include_str!("resource_hooks.sql");

/// Exact installed worker identity granted access to one space's admission seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceHookBinding {
    /// Ordinary installed package identifier.
    pub package: String,
    /// Approved package review SHA-256, lowercase hexadecimal.
    pub checksum: String,
    /// Optional dedicated space where this package keeps its own resource state.
    pub state_space: Option<SpaceId>,
}

/// Resource admission outcome returned by a declared block hook.
#[derive(Debug, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Decision {
    Allow { memory_mib: Option<u32> },
    Queue { reason: String, retry_after_ms: u32 },
    Refuse { reason: String },
}

impl Decision {
    pub(crate) fn valid(&self) -> bool {
        match self {
            Self::Allow { memory_mib } => memory_mib
                .is_none_or(|mib| calternal_machine::protocol::GuestSize::from_mib(mib).is_some()),
            Self::Queue { reason, .. } | Self::Refuse { reason } => reason_is_safe(reason),
        }
    }
}

pub(super) struct Permit {
    binding: ResourceHookBinding,
    renew_after: Instant,
    pub(super) reason: Option<String>,
    memory_mib: Option<u32>,
}

pub(crate) struct Invocation {
    pub(crate) binding: ResourceHookBinding,
    pub(crate) space: SpaceId,
    pub(crate) input: Value,
    pub(crate) owner: Option<calternal_types::ResourceOwner>,
}

impl MachineService {
    /// Bind or clear an operator-approved admission hook for a space.
    /// Cost: one protected scope lookup and one configuration commit. The
    /// operator must verify the complete package review before binding its checksum.
    pub fn set_resource_hook(
        &self,
        space: SpaceId,
        binding: Option<ResourceHookBinding>,
    ) -> Result<(), Error> {
        if let Some(binding) = &binding {
            if !(1..=128).contains(&binding.package.len())
                || !binding.package.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.' | b'_')
                })
                || binding.checksum.len() != 64
                || !binding
                    .checksum
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
            {
                return Err(Error::invalid_input(
                    "invalid resource hook package or digest",
                ));
            }
            let authority = self.lock()?.authority.clone();
            authority
                .require_resource_space(space)
                .map_err(compute::authority_error)?;
            if let Some(state_space) = binding.state_space {
                authority
                    .require_resource_space(state_space)
                    .map_err(compute::authority_error)?;
            }
        }
        let mut state = self.lock()?;
        if let Some(binding) = binding {
            state.db.execute(
                "INSERT INTO resource_hooks VALUES(?1,?2,?3,?4) ON CONFLICT(space)
                 DO UPDATE SET package=excluded.package,checksum=excluded.checksum,state_space=excluded.state_space",
                params![space.to_string(),binding.package,binding.checksum,binding.state_space.map(|space| space.to_string())],
            ).map_err(sql)?;
            state.resource_hooks.insert(space, binding);
        } else {
            state
                .db
                .execute(
                    "DELETE FROM resource_hooks WHERE space=?1",
                    [space.to_string()],
                )
                .map_err(sql)?;
            state.resource_hooks.remove(&space);
        }
        state.place_waiting(&self.settings);
        Ok(())
    }

    /// Snapshot only the resources implicated by this waiting lease. One
    /// bounded confirmed-runtime read when a scope is assigned.
    pub(crate) fn resource_hook_invocation(
        &self,
        account: AccountId,
        lease: Uuid,
    ) -> Result<Option<Invocation>, Error> {
        let state = self.lock()?;
        let lease = state
            .leases
            .get(&lease)
            .filter(|l| l.row.account == account && !l.row.worker && !l.row.background_title);
        let Some(lease) = lease.filter(|l| l.state == LeaseState::Waiting) else {
            return Ok(None);
        };
        let Some(binding) = state.resource_hooks.get(&lease.row.space) else {
            return Ok(None);
        };
        if state
            .resource_permits
            .get(&lease.row.id)
            .is_some_and(|permit| permit.binding == *binding && permit.renew_after > Instant::now())
        {
            return Ok(None);
        }
        let scope = lease.row.resource_owner;
        let (ns, unresolved) = if let Some(scope) = scope {
            let now = db::now_ms();
            runtime::usage(&state.db, scope, now.saturating_sub(35 * 86_400_000), now)?
        } else {
            (0, 0)
        };
        let guests: Vec<_> = state
            .machines
            .values()
            .filter_map(|machine| {
                let live = machine.live.as_ref()?;
                let report = live.guest_resources.get(&account)?;
                (report.resource_owner == scope).then(|| {
                    json!({
                        "generation": report.generation,
                        "memory_available_bytes": report.memory_available_bytes.to_string(),
                        "active_threads": report.active_threads,
                        "profile_mib": live.guest_memory_mib
                    })
                })
            })
            .take(64)
            .collect();
        let authority = state.authority.clone();
        let mut invocation = Invocation {
            binding: binding.clone(),
            space: lease.row.space,
            owner: scope,
            input: json!({
                "request_id": lease.row.id,
                "space_id": lease.row.space,
                "account_id": account,
                "scope": scope,
                "machine_class": "hosted",
                "runtime_ns": ns.to_string(),
                "unresolved_generations": unresolved,
                "guests": guests
            }),
        };
        drop(state);
        invocation.input["entitlement_groups"] = json!(groups(&authority, scope)?);
        Ok(Some(invocation))
    }

    pub(crate) fn settle_resource_hook(
        &self,
        account: AccountId,
        lease: Uuid,
        binding: ResourceHookBinding,
        decision: Decision,
    ) -> Result<(), Error> {
        let mut state = self.lock()?;
        let Some(lease_row) = state
            .leases
            .get(&lease)
            .filter(|l| l.row.account == account && l.state == LeaseState::Waiting)
        else {
            return Ok(());
        };
        if state.resource_hooks.get(&lease_row.row.space) != Some(&binding) {
            return Ok(());
        }
        let one_attempt = matches!(decision, Decision::Allow { .. });
        let (reason, after, memory_mib) = match decision {
            Decision::Allow { memory_mib } => (None, Duration::from_secs(1), memory_mib),
            Decision::Queue {
                reason,
                retry_after_ms,
            } => (
                Some(clean_reason(reason)?),
                Duration::from_millis(u64::from(retry_after_ms.clamp(250, 60_000))),
                None,
            ),
            Decision::Refuse { reason } => {
                let reason = clean_reason(reason)?;
                state.end_lease(lease, &reason, &self.settings);
                return Ok(());
            }
        };
        state.resource_permits.insert(
            lease,
            Permit {
                binding,
                reason,
                memory_mib,
                renew_after: Instant::now() + after,
            },
        );
        state.place_waiting(&self.settings);
        if one_attempt {
            state.resource_permits.remove(&lease);
        }
        Ok(())
    }
}

impl State {
    pub(super) fn resource_hook_allows(&self, lease: &Lease, machine: &super::Machine) -> bool {
        if lease.row.worker
            || lease.row.background_title
            || lease.pool_diagnostic
            || lease.provider_login
        {
            return true;
        }
        let Some(binding) = self.resource_hooks.get(&lease.row.space) else {
            return true;
        };
        self.resource_permits
            .get(&lease.row.id)
            .is_some_and(|permit| {
                permit.binding == *binding
                    && permit.reason.is_none()
                    && permit.renew_after > Instant::now()
                    && permit.memory_mib.is_none_or(|mib| {
                        machine.live.as_ref().and_then(|live| live.guest_memory_mib) == Some(mib)
                    })
            })
    }
}

pub(super) fn load(conn: &Connection) -> Result<HashMap<SpaceId, ResourceHookBinding>, Error> {
    let mut stmt = conn
        .prepare("SELECT space,package,checksum,state_space FROM resource_hooks")
        .map_err(sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(sql)?;
    rows.map(|row| {
        let (space, package, checksum, state_space) = row.map_err(sql)?;
        Ok((
            SpaceId::parse(&space)?,
            ResourceHookBinding {
                package,
                checksum,
                state_space: state_space.as_deref().map(SpaceId::parse).transpose()?,
            },
        ))
    })
    .collect()
}

fn reason_is_safe(reason: &str) -> bool {
    !reason.trim().is_empty()
        && reason.chars().count() <= 256
        && !reason.chars().any(char::is_control)
}

fn clean_reason(reason: String) -> Result<String, Error> {
    if !reason_is_safe(&reason) {
        Err(Error::invalid_input("invalid resource admission reason"))
    } else {
        Ok(reason)
    }
}

impl MachineService {
    /// Return this already-reviewed package's exact operator binding for a
    /// space. One keyed snapshot under the placement lock.
    pub(crate) fn resource_binding(
        &self,
        space: SpaceId,
        package: &str,
        checksum: &str,
    ) -> Result<ResourceHookBinding, Error> {
        let state = self.lock()?;
        state
            .resource_hooks
            .get(&space)
            .filter(|binding| binding.package == package && binding.checksum == checksum)
            .cloned()
            .ok_or_else(|| Error::forbidden("no resource observation grant"))
    }

    /// Read exact cumulative runtime inside the requested retained window.
    /// The scope and its operator binding are host-resolved, never supplied by the worker.
    pub(crate) fn resource_runtime_window(
        &self,
        space: SpaceId,
        binding: &ResourceHookBinding,
        since_ms: i64,
        until_ms: Option<i64>,
    ) -> Result<Value, Error> {
        let now = db::now_ms();
        let through = until_ms.unwrap_or(now);
        if since_ms > through || through > now || since_ms < now.saturating_sub(35 * 86_400_000) {
            return Err(Error::invalid_input("runtime window is outside retention"));
        }
        let authority = self.lock()?.authority.clone();
        let owner = self
            .resource_owner(space)?
            .ok_or_else(|| Error::not_found("resource scope is not assigned"))?;
        let groups = groups(&authority, Some(owner))?;
        let state = self.lock()?;
        if state.resource_hooks.get(&space) != Some(binding) {
            return Err(Error::forbidden("no resource observation grant"));
        }
        let (used, unresolved) =
            runtime::usage_at(&state.db, owner, since_ms, through, until_ms.is_some())?;
        Ok(
            json!({"running_ns":used.to_string(),"unresolved":unresolved,"since_ms":since_ms,"through_ms":through,"owner":owner,"entitlement_groups":groups}),
        )
    }

    /// Read one scoped page of cumulative guest clock observations. The
    /// operator binding and exact package review are checked across the
    /// authority lookup. Up to 64 retained entries, without machine IDs.
    pub(crate) fn resource_observation_page(
        &self,
        space: SpaceId,
        binding: &ResourceHookBinding,
        after: i64,
    ) -> Result<Value, Error> {
        if self.lock()?.resource_hooks.get(&space) != Some(binding) {
            return Err(Error::forbidden("no resource observation grant"));
        }
        let owner = self
            .resource_owner(space)?
            .ok_or_else(|| Error::not_found("resource scope is not assigned"))?;
        let state = self.lock()?;
        if state.resource_hooks.get(&space) != Some(binding) {
            return Err(Error::forbidden("no resource observation grant"));
        }
        let floor: i64 = state
            .db
            .query_row(
                "SELECT cursor FROM resource_observation_floor WHERE singleton=1",
                [],
                |row| row.get(0),
            )
            .map_err(sql)?;
        if after > 0 && after < floor {
            return Ok(json!({"reset_required":true,"observations":[],"cursor":"0","more":false}));
        }
        let (kind, id) = compute::encode(owner);
        let mut query = state
            .db
            .prepare_cached(
                "SELECT o.cursor,r.generation,r.started_ms,r.running_ns,r.state
             FROM resource_observations o JOIN guest_runtime r USING(machine,generation)
             WHERE o.scope_kind=?1 AND o.scope_id=?2 AND o.cursor>?3 ORDER BY o.cursor LIMIT 65",
            )
            .map_err(sql)?;
        let mut rows = query.query(params![kind, id, after]).map_err(sql)?;
        let mut cursor = after;
        let mut observations = Vec::new();
        let mut more = false;
        while let Some(row) = rows.next().map_err(sql)? {
            if observations.len() == 64 {
                more = true;
                break;
            }
            cursor = row.get::<_, i64>(0).map_err(sql)?;
            let state = match row.get::<_, i64>(4).map_err(sql)? {
                0 => "running",
                1 => "stopped",
                _ => "unknown",
            };
            observations.push(json!({
                "generation": row.get::<_, String>(1).map_err(sql)?,
                "started_at_ms": row.get::<_, i64>(2).map_err(sql)?,
                "running_ns": row.get::<_, i64>(3).map_err(sql)?.to_string(),
                "state": state,
            }));
        }
        Ok(
            json!({"reset_required":false,"observations":observations,"cursor":cursor.to_string(),"more":more}),
        )
    }
}

fn groups(
    authority: &calternal_auth::machines::Machines,
    owner: Option<calternal_types::ResourceOwner>,
) -> Result<Vec<calternal_types::GroupId>, Error> {
    match owner {
        Some(calternal_types::ResourceOwner::Account(account)) => authority
            .resource_groups(account)
            .map_err(compute::authority_error),
        Some(calternal_types::ResourceOwner::Group(group)) => Ok(vec![group]),
        None => Ok(Vec::new()),
    }
}
