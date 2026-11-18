//! Durable confirmed intervals, accepted only from a matching fenced placement.

use calternal_machine::protocol::{
    GuestRuntime, MAX_GUEST_RUNTIME, ResourceAssignment, RuntimeState,
};
use calternal_types::{Error, ResourceOwner};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use super::{MachineService, State, compute, db, sql};

pub(super) const MIGRATION: &str = include_str!("runtime.sql");
const RETENTION_MS: i64 = 35 * 24 * 60 * 60 * 1_000;
const MAX_MACHINE_GENERATIONS: i64 = 65_536;
const NS_PER_MS: i64 = 1_000_000;

/// Observed ordered runtime in the retained accounting interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeUsage {
    /// Confirmed whole running seconds. Subseconds remain in the durable source.
    pub running_seconds: u64,
    /// Number of generation ends whose final runtime could not be established.
    pub unresolved: u32,
    /// Earliest UTC timestamp included in this sample, in Unix milliseconds.
    pub since_ms: i64,
}

pub(super) struct Window {
    pub(super) ends_ms: i64,
    pub(super) allowed: bool,
}

impl State {
    /// Update a configured scope's cached decision from durable evidence. Read
    /// only on usage changes, a policy update, startup, or window rollover.
    pub(super) fn refresh_runtime(&mut self, owner: ResourceOwner, now: i64) -> Result<(), Error> {
        let Some((budget, seconds)) = self
            .compute_limits
            .get(&owner)
            .and_then(|limits| limits.runtime_seconds.zip(limits.reset_seconds))
        else {
            self.runtime_windows.remove(&owner);
            return Ok(());
        };
        let period = i64::from(seconds) * 1000;
        let start = now - now.rem_euclid(period);
        let (used, unresolved) = usage(&self.db, owner, start, now)?;
        self.runtime_windows.insert(
            owner,
            Window {
                ends_ms: start + period,
                allowed: unresolved == 0 && used < u128::from(budget) * 1_000_000_000,
            },
        );
        Ok(())
    }
}

impl MachineService {
    /// Read a scope's confirmed elapsed seconds over the configured retention.
    /// Costs one bounded index scan under the machine registry lock.
    /// The caller must have operator authority.
    pub fn runtime_usage(&self, owner: ResourceOwner) -> Result<RuntimeUsage, Error> {
        let state = self.lock()?;
        let now = db::now_ms();
        let since = now.saturating_sub(RETENTION_MS);
        let (ns, unresolved) = usage(&state.db, owner, since, now)?;
        Ok(RuntimeUsage {
            running_seconds: (ns / 1_000_000_000).try_into().unwrap_or(u64::MAX),
            unresolved,
            since_ms: since,
        })
    }
}

/// Store one batch in a FULL transaction. Existing generations are keyed by
/// enrolled machine identity and fenced generation. Replays never add time.
pub(super) fn record(
    conn: &mut Connection,
    machine: Uuid,
    reports: &[GuestRuntime],
) -> Result<HashSet<ResourceOwner>, Error> {
    if reports.is_empty() {
        return Ok(HashSet::new());
    }
    if reports.len() > MAX_GUEST_RUNTIME {
        return Err(Error::invalid_input("too many guest clock observations"));
    }
    let received_ms = db::now_ms();
    let machine = machine.to_string();
    let tx = conn.transaction().map_err(sql)?;
    let mut seen = HashSet::with_capacity(reports.len());
    let mut touched = HashSet::new();
    for report in reports {
        let Some(owner) = report.resource_owner else {
            return Err(Error::invalid_input("guest clock has no resource scope"));
        };
        let (kind, id) = compute::encode(owner);
        let generation = report.generation.to_string();
        let lease = report.assignment.lease_id.to_string();
        let fence = checked_i64(report.assignment.fence)?;
        let ns = checked_i64(report.running_ns)?;
        let status = status(report.state);
        if report.generation.is_nil()
            || fence <= 0
            || !seen.insert(report.generation)
            || report.started_at_ms < 0
            || report.started_at_ms.saturating_add(ns / NS_PER_MS)
                > received_ms.saturating_add(60_000)
        {
            return Err(Error::invalid_input("invalid guest clock observation"));
        }
        let existing = tx.query_row(
            "SELECT account=?3 AND scope_kind=?4 AND scope_id=?5 AND lease=?6 AND fence=?7 AND started_ms=?8,
                    running_ns,state
             FROM guest_runtime WHERE machine=?1 AND generation=?2",
            params![machine,generation,report.account_id.to_string(),kind,id,lease,fence,report.started_at_ms],
            |row| Ok((row.get::<_, bool>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?)),
        ).optional().map_err(sql)?;
        if let Some((same, old_ns, old_state)) = existing {
            if !same {
                return Err(Error::conflict(
                    "guest clock generation has a different origin",
                ));
            }
            if old_state == 1 {
                if status == 1 && ns != old_ns {
                    return Err(Error::conflict("final guest clock cannot change"));
                }
                continue;
            }
            if status == 0 && ns < old_ns {
                continue;
            }
            let ns = ns.max(old_ns);
            tx.execute(
                "UPDATE guest_runtime SET running_ns=?3,state=?4,received_ms=?5
                         WHERE machine=?1 AND generation=?2 AND (running_ns<>?3 OR state<>?4)",
                params![machine, generation, ns, status, received_ms],
            )
            .map_err(sql)?;
            touched.insert(owner);
            continue;
        }
        if report.started_at_ms.saturating_add(ns / NS_PER_MS)
            < received_ms.saturating_sub(RETENTION_MS)
        {
            // Retire a source receipt older than the bounded replay horizon.
            continue;
        }
        require_assignment(&tx, &machine, report.assignment, report.account_id, owner)?;
        if tx
            .query_row(
                "SELECT count(*) FROM guest_runtime WHERE machine=?1",
                [&machine],
                |row| row.get::<_, i64>(0),
            )
            .map_err(sql)?
            >= MAX_MACHINE_GENERATIONS
        {
            return Err(Error::unavailable("hosted clock history is full"));
        }
        tx.execute(
            "INSERT INTO guest_runtime VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                machine,
                generation,
                report.account_id.to_string(),
                kind,
                id,
                lease,
                fence,
                report.started_at_ms,
                ns,
                status,
                received_ms
            ],
        )
        .map_err(sql)?;
        touched.insert(owner);
    }
    tx.commit().map_err(sql)?;
    Ok(touched)
}

fn require_assignment(
    conn: &Connection,
    machine: &str,
    assignment: ResourceAssignment,
    account: calternal_types::AccountId,
    owner: ResourceOwner,
) -> Result<(), Error> {
    let (kind, id) = compute::encode(owner);
    let exists = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM resource_assignments a
         JOIN leases l ON l.id=a.lease JOIN lease_resource_owners o ON o.lease=l.id
         WHERE a.lease=?1 AND a.fence=?2 AND a.machine=?3
         AND l.account=?4 AND o.scope_kind=?5 AND o.scope_id=?6)",
            params![
                assignment.lease_id.to_string(),
                checked_i64(assignment.fence)?,
                machine,
                account.to_string(),
                kind,
                id
            ],
            |row| row.get::<_, bool>(0),
        )
        .map_err(sql)?;
    if exists {
        Ok(())
    } else {
        Err(Error::forbidden("guest clock assignment is unknown"))
    }
}

pub(super) fn usage(
    conn: &Connection,
    owner: ResourceOwner,
    since: i64,
    through: i64,
) -> Result<(u128, u32), Error> {
    usage_at(conn, owner, since, through, false)
}

/// In a closed interval, require either a stop or a sample at its upper bound
/// before a live or lost generation may count as fully observed.
pub(super) fn usage_at(
    conn: &Connection,
    owner: ResourceOwner,
    since: i64,
    through: i64,
    closed: bool,
) -> Result<(u128, u32), Error> {
    let (kind, id) = compute::encode(owner);
    let mut query = conn
        .prepare_cached(
            "SELECT started_ms,running_ns,state FROM guest_runtime
         WHERE scope_kind=?1 AND scope_id=?2 AND started_ms<?4
           AND (state<>1 OR started_ms+running_ns/1000000>=?3)",
        )
        .map_err(sql)?;
    let mut rows = query
        .query(params![kind, id, since, through])
        .map_err(sql)?;
    let mut sum = 0u128;
    let mut unresolved = 0u32;
    while let Some(row) = rows.next().map_err(sql)? {
        let start = i128::from(row.get::<_, i64>(0).map_err(sql)?);
        let runtime = i128::from(row.get::<_, i64>(1).map_err(sql)?);
        let low = ((i128::from(since) - start) * i128::from(NS_PER_MS)).max(0);
        let high = ((i128::from(through) - start) * i128::from(NS_PER_MS)).min(runtime);
        sum += (high - low).max(0) as u128;
        let state = row.get::<_, i64>(2).map_err(sql)?;
        let reaches_end = runtime >= (i128::from(through) - start) * i128::from(NS_PER_MS);
        let missing = if closed {
            state != 1 && !reaches_end
        } else {
            state == 2
        };
        unresolved = unresolved.saturating_add(u32::from(missing));
    }
    Ok((sum, unresolved))
}

/// Fence an unobservable clock until a later sample recovers its watermark.
pub(super) fn mark_missing(
    conn: &Connection,
    machine: Option<Uuid>,
) -> Result<Vec<ResourceOwner>, Error> {
    let machine = machine.map(|id| id.to_string());
    let mut stmt = conn
        .prepare_cached(
            "UPDATE guest_runtime SET state=2 WHERE state=0 AND (?1 IS NULL OR machine=?1)
         RETURNING scope_kind,scope_id",
        )
        .map_err(sql)?;
    let rows = stmt
        .query_map([machine], |row| compute::decode(row, 0, 1))
        .map_err(sql)?;
    rows.map(|row| row.map_err(sql)).collect()
}

pub(super) fn retire(conn: &Connection, now: i64) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM guest_runtime WHERE state=1 AND received_ms<?1",
        [now.saturating_sub(RETENTION_MS)],
    )
    .map_err(sql)?;
    Ok(())
}

fn checked_i64(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::invalid_input("guest clock overflow"))
}

const fn status(state: RuntimeState) -> i64 {
    match state {
        RuntimeState::Running => 0,
        RuntimeState::Stopped => 1,
        RuntimeState::Unknown => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_types::{AccountId, GroupId, SpaceId};

    #[test]
    fn fenced_cumulative_samples_survive_restart_and_count_the_exact_interval() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("machines.db");
        let mut conn = db::open(&file).unwrap();
        let account = AccountId::new();
        let owner = ResourceOwner::Group(GroupId::new());
        let machine = Uuid::new_v4();
        let started_at_ms = db::now_ms() - 5_000;
        let lease = db::LeaseRow {
            id: Uuid::new_v4(),
            space: SpaceId::new(),
            account,
            shared_sponsor: None,
            resource_owner: Some(owner),
            thread: Uuid::new_v4(),
            worker: false,
            background_title: false,
            image: None,
            harness: None,
            machine: Some(machine),
            last_machine: Some(machine),
            pinned_machine: None,
            fence: 1,
            state: "running".into(),
            reason: None,
            created_ms: started_at_ms,
        };
        db::put_scoped_lease(&mut conn, &lease).unwrap();
        db::put_assignment(&mut conn, &lease, true).unwrap();
        let mut sample = GuestRuntime {
            account_id: account,
            generation: Uuid::new_v4(),
            resource_owner: Some(owner),
            assignment: ResourceAssignment {
                lease_id: lease.id,
                fence: 1,
            },
            started_at_ms,
            running_ns: 2_000_000_019,
            state: RuntimeState::Running,
        };
        assert!(record(&mut conn, Uuid::new_v4(), &[sample]).is_err());
        record(&mut conn, machine, &[sample]).unwrap();
        record(&mut conn, machine, &[sample]).unwrap();
        assert_eq!(
            usage_at(&conn, owner, started_at_ms, started_at_ms + 3_000, true).unwrap(),
            (2_000_000_019, 1)
        );
        assert_eq!(
            usage_at(&conn, owner, started_at_ms, started_at_ms + 1_000, true).unwrap(),
            (1_000_000_000, 0)
        );
        drop(conn);
        let mut conn = db::open(&file).unwrap();
        assert_eq!(mark_missing(&conn, None).unwrap(), [owner]);
        assert_eq!(
            usage(&conn, owner, started_at_ms + 1_000, db::now_ms()).unwrap(),
            (1_000_000_019, 1)
        );
        assert_eq!(
            usage(
                &conn,
                ResourceOwner::Account(account),
                started_at_ms,
                db::now_ms()
            )
            .unwrap(),
            (0, 0)
        );
        sample.running_ns += 500;
        sample.state = RuntimeState::Stopped;
        record(&mut conn, machine, &[sample]).unwrap();
        record(&mut conn, machine, &[sample]).unwrap();
        assert_eq!(
            usage(&conn, owner, started_at_ms, db::now_ms()).unwrap(),
            (2_000_000_519, 0)
        );
        sample.running_ns += 1;
        assert!(record(&mut conn, machine, &[sample]).is_err());
    }
}
