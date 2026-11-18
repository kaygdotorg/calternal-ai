//! Operator compute limits stored with placement authority.

use calternal_auth::machines::ResourceOwner;
use calternal_types::{AccountId, Error, GroupId, SpaceId};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::{MachineService, sql};

/// Configured bounds for one account or group's hosted guest admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComputeLimits {
    /// Whether the scope may use otherwise authorized hosted machines.
    pub hosted: bool,
    /// Eligible hosted guest memory profile in MiB, or any profile.
    pub memory_mib: Option<u32>,
    /// Optional hard ceiling per sandbox, in addition to memory headroom.
    pub active_threads: Option<u32>,
    /// Maximum waiting leases for this scope, or no additional queue bound.
    pub queue_bound: Option<u32>,
    /// Stop a bound guest after this many idle seconds, or use the machine default.
    pub idle_seconds: Option<u32>,
    /// Confirmed sandbox seconds available in a reset window.
    pub runtime_seconds: Option<u32>,
    /// Window length in seconds, aligned to the Unix epoch.
    pub reset_seconds: Option<u32>,
}

impl ComputeLimits {
    fn validate(self) -> Result<Self, Error> {
        if self
            .memory_mib
            .is_some_and(|mib| calternal_machine::protocol::GuestSize::from_mib(mib).is_none())
            || self
                .active_threads
                .is_some_and(|n| !(1..=65_536).contains(&n))
            || self.queue_bound.is_some_and(|n| n > 65_536)
            || self
                .idle_seconds
                .is_some_and(|n| !(1..=86_400).contains(&n))
            || self.runtime_seconds.is_some() != self.reset_seconds.is_some()
            || self.runtime_seconds.is_some_and(|n| n > 31 * 86_400)
            || self
                .reset_seconds
                .is_some_and(|n| !(1..=31 * 86_400).contains(&n))
        {
            return Err(Error::invalid_input(
                "compute limits are outside supported bounds",
            ));
        }
        Ok(self)
    }
}

impl MachineService {
    /// Read the operator-selected accounting owner. Auth resolves personal
    /// spaces and validates a configured group using stable UUIDs. Accounting
    /// policy never confers space membership. Cost: one policy lookup and up to
    /// two indexed auth reads. The caller must authorize use of the space.
    pub(crate) fn resource_owner(&self, space: SpaceId) -> Result<Option<ResourceOwner>, Error> {
        let (authority, group) = {
            let state = self.lock()?;
            let group: Option<String> = state
                .db
                .query_row(
                    "SELECT group_id FROM space_resource_groups WHERE space=?1",
                    [space.to_string()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(sql)?;
            (state.authority.clone(), group)
        };
        if let Some(account) = authority.resource_account(space).map_err(authority_error)? {
            return Ok(Some(ResourceOwner::Account(account)));
        }
        let Some(group) = group else { return Ok(None) };
        let owner = ResourceOwner::Group(GroupId::parse(&group)?);
        match authority.require_resource_owner(owner) {
            Ok(()) => Ok(Some(owner)),
            Err(error) if error.code() == calternal_auth::ErrorCode::NotFound => Ok(None),
            Err(error) => Err(authority_error(error)),
        }
    }

    /// Bind one shared space to an existing accounting group. Costs at most
    /// three indexed auth reads and one policy transaction. Holds the machine
    /// registry lock only for writing its own state. Operator authority required.
    pub(crate) fn set_resource_group(
        &self,
        space: SpaceId,
        group: Option<GroupId>,
    ) -> Result<(), Error> {
        let authority = self.lock()?.authority.clone();
        authority
            .require_resource_space(space)
            .map_err(authority_error)?;
        if authority
            .resource_account(space)
            .map_err(authority_error)?
            .is_some()
        {
            return Err(Error::invalid_input(
                "personal space resources belong to the account",
            ));
        }
        if let Some(group) = group {
            authority
                .require_resource_owner(ResourceOwner::Group(group))
                .map_err(authority_error)?;
            self.lock()?.db.execute(
                "INSERT INTO space_resource_groups VALUES(?1,?2)
                 ON CONFLICT(space) DO UPDATE SET group_id=excluded.group_id WHERE group_id<>excluded.group_id",
                params![space.to_string(), group.to_string()],
            ).map_err(sql)?;
        } else {
            self.lock()?
                .db
                .execute(
                    "DELETE FROM space_resource_groups WHERE space=?1",
                    [space.to_string()],
                )
                .map_err(sql)?;
        }
        Ok(())
    }

    /// Replace or clear one scope's manually configured compute limits.
    /// The caller must have operator authority. One bounded config commit, then
    /// reconsider existing waits under the shared placement lock.
    pub fn set_compute_limits(
        &self,
        owner: ResourceOwner,
        limits: Option<ComputeLimits>,
    ) -> Result<(), Error> {
        let limits = limits.map(ComputeLimits::validate).transpose()?;
        if limits.is_some() {
            let authority = self.lock()?.authority.clone();
            authority
                .require_resource_owner(owner)
                .map_err(authority_error)?;
        }
        let (kind, id) = encode(owner);
        let mut state = self.lock()?;
        if let Some(limits) = limits {
            state
                .db
                .execute(
                    "INSERT INTO compute_limits VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)
                 ON CONFLICT(scope_kind,scope_id) DO UPDATE SET hosted=excluded.hosted,
                 memory_mib=excluded.memory_mib,active_threads=excluded.active_threads,
                 queue_bound=excluded.queue_bound,idle_seconds=excluded.idle_seconds,
                 runtime_seconds=excluded.runtime_seconds,reset_seconds=excluded.reset_seconds",
                    params![
                        kind,
                        id,
                        limits.hosted,
                        limits.memory_mib,
                        limits.active_threads,
                        limits.queue_bound,
                        limits.idle_seconds,
                        limits.runtime_seconds,
                        limits.reset_seconds
                    ],
                )
                .map_err(sql)?;
            state.compute_limits.insert(owner, limits);
        } else {
            state
                .db
                .execute(
                    "DELETE FROM compute_limits WHERE scope_kind=?1 AND scope_id=?2",
                    params![kind, id],
                )
                .map_err(sql)?;
            state.compute_limits.remove(&owner);
        }
        state.refresh_runtime(owner, super::db::now_ms())?;
        state.place_waiting(&self.settings);
        Ok(())
    }
}

pub(super) fn load(conn: &Connection) -> Result<HashMap<ResourceOwner, ComputeLimits>, Error> {
    let mut stmt = conn.prepare(
        "SELECT scope_kind,scope_id,hosted,memory_mib,active_threads,queue_bound,idle_seconds,runtime_seconds,reset_seconds FROM compute_limits"
    ).map_err(sql)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                decode(row, 0, 1)?,
                ComputeLimits {
                    hosted: row.get(2)?,
                    memory_mib: row.get(3)?,
                    active_threads: row.get(4)?,
                    queue_bound: row.get(5)?,
                    idle_seconds: row.get(6)?,
                    runtime_seconds: row.get(7)?,
                    reset_seconds: row.get(8)?,
                },
            ))
        })
        .map_err(sql)?;
    rows.map(|row| row.map_err(sql)).collect()
}

pub(super) fn encode(owner: ResourceOwner) -> (&'static str, String) {
    match owner {
        ResourceOwner::Account(id) => ("account", id.to_string()),
        ResourceOwner::Group(id) => ("group", id.to_string()),
    }
}

pub(super) fn decode(row: &Row<'_>, kind: usize, id: usize) -> rusqlite::Result<ResourceOwner> {
    let value: String = row.get(id)?;
    let owner = match row.get::<_, String>(kind)?.as_str() {
        "account" => AccountId::parse(&value).map(ResourceOwner::Account),
        "group" => GroupId::parse(&value).map(ResourceOwner::Group),
        _ => Err(Error::invalid_input("invalid resource owner kind")),
    };
    owner.map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(id, rusqlite::types::Type::Text, Box::new(error))
    })
}

pub(super) fn authority_error(error: calternal_auth::Error) -> Error {
    tracing::warn!(%error, "resource authority");
    match error.code() {
        calternal_auth::ErrorCode::NotFound => {
            Error::not_found("resource account, group or space not found")
        }
        calternal_auth::ErrorCode::InvalidInput => {
            Error::invalid_input("resource scope cannot be set for this space")
        }
        _ => Error::unavailable("resource authority is unavailable"),
    }
}

pub(super) const MIGRATION: &str = include_str!("compute_limits.sql");
