//! Server-issued small-VM checkpoint numbers and restore authorization.
use super::{sql, state::State};
use calternal_types::{Error, SpaceId};
use rusqlite::{Connection, OptionalExtension as _, params};
use std::collections::HashSet;
use uuid::Uuid;

/// Resolve acknowledged affinity and machines still within the erase retry limit.
/// The scheduler calls this only for cold placement.
pub(super) fn placement(
    db: &Connection,
    space: SpaceId,
) -> Result<(Option<Uuid>, HashSet<Uuid>), Error> {
    let space = space.to_string();
    let holder = db
        .query_row(
            "SELECT acknowledged FROM space_runtime_checkpoints WHERE space=?1",
            [&space],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(sql)?
        .flatten();
    let holder = holder
        .map(|value| super::erasure_queue::parse_uuid(value, 0))
        .transpose()
        .map_err(sql)?;
    let mut query = db
        .prepare_cached("SELECT machine FROM space_runtime_erasures WHERE space=?1 AND failures<?2")
        .map_err(sql)?;
    let pending = query
        .query_map(
            params![space, super::erasure_queue::RUNTIME_RETRIES],
            |row| super::erasure_queue::uuid(row, 0),
        )
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)?;
    Ok((holder, pending))
}

/// Authorize exactly the current acknowledged snapshot on the assigned machine.
pub(super) fn proof(db: &Connection, space: SpaceId, machine: Uuid) -> Result<Option<u64>, Error> {
    db.query_row(
        "SELECT counter FROM space_runtime_checkpoints WHERE space=?1 AND acknowledged=?2
         AND NOT EXISTS(SELECT 1 FROM space_runtime_erasures WHERE space=?1 AND machine=?2 AND (through IS NULL OR through>=space_runtime_checkpoints.counter))",
        params![space.to_string(), machine.to_string()], |row| row.get::<_, i64>(0),
    ).optional().map(|v| v.map(|counter| counter as u64)).map_err(sql)
}

fn retire_others(
    tx: &Connection,
    space: &str,
    holder: &str,
    through: i64,
) -> Result<Vec<Uuid>, Error> {
    let mut query = tx.prepare(
        "INSERT INTO space_runtime_erasures(space,machine,request_id,through)
         SELECT space,machine,lower(hex(randomblob(16))),?3
         FROM (SELECT DISTINCT space,machine FROM space_runtime_locations WHERE space=?1 AND machine!=?2) WHERE true
         ON CONFLICT(space,machine) DO UPDATE SET
           request_id=excluded.request_id, through=excluded.through
         WHERE space_runtime_erasures.through<excluded.through
         RETURNING machine",
    ).map_err(sql)?;
    query
        .query_map(params![space, holder, through], |row| {
            super::erasure_queue::uuid(row, 0)
        })
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)
}

impl State {
    /// Persist the next number and retire other copies before emitting SleepSpace.
    pub(super) fn advance_checkpoint(
        &mut self,
        space: SpaceId,
        machine: Uuid,
        lease: Uuid,
    ) -> Result<(), Error> {
        let tx = self.db.transaction().map_err(sql)?;
        let space = space.to_string();
        let counter = tx.query_row(
            "INSERT INTO space_runtime_checkpoints VALUES(?1,1,?2,NULL)
             ON CONFLICT(space) DO UPDATE SET counter=counter+1,lease=excluded.lease,acknowledged=NULL
             RETURNING counter",
            params![space, lease.to_string()], |row| row.get(0),
        ).map_err(sql)?;
        let stale = retire_others(&tx, &space, &machine.to_string(), counter)?;
        tx.commit().map_err(sql)?;
        self.notify_runtime_erasures(stale);
        Ok(())
    }

    /// A cold move retires the old restore proof before a new guest can run.
    pub(super) fn prepare_checkpoint_placement(
        &mut self,
        space: SpaceId,
        machine: Uuid,
    ) -> Result<(), Error> {
        let tx = self.db.transaction().map_err(sql)?;
        let space = space.to_string();
        let holder = machine.to_string();
        let counter = tx.query_row(
            "UPDATE space_runtime_checkpoints SET acknowledged=NULL, lease=''
             WHERE space=?1 AND lease!='' AND (acknowledged IS NULL OR acknowledged!=?2) RETURNING counter",
            params![space,holder], |row| row.get(0),
        ).optional().map_err(sql)?;
        let Some(counter) = counter else {
            return Ok(());
        };
        let stale = retire_others(&tx, &space, &holder, counter)?;
        tx.commit().map_err(sql)?;
        self.notify_runtime_erasures(stale);
        Ok(())
    }

    fn notify_runtime_erasures(&mut self, stale: Vec<Uuid>) {
        self.pending_erasure_machines.extend(stale.iter().copied());
        for machine in stale {
            if let Some(live) = self.machines.get(&machine).and_then(|m| m.live.as_ref()) {
                live.notify.notify_one();
            }
        }
    }
}

/// Read the persistently authorized SleepSpace number for this lease.
pub(super) fn sleep_counter(
    conn: &Connection,
    lease: Uuid,
    space: SpaceId,
) -> Result<Option<u64>, Error> {
    conn.query_row(
        "SELECT counter FROM space_runtime_checkpoints WHERE space=?1 AND lease=?2",
        params![space.to_string(), lease.to_string()],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|v| v.map(|counter| counter as u64))
    .map_err(sql)
}

/// A late or superseded report cannot become a restore proof.
pub(super) fn acknowledge(
    conn: &Connection,
    space: SpaceId,
    lease: Uuid,
    machine: Uuid,
    counter: u64,
) -> Result<(), Error> {
    let Ok(counter) = i64::try_from(counter) else {
        return Ok(());
    };
    conn.execute(
        "UPDATE space_runtime_checkpoints SET acknowledged=?4
         WHERE space=?1 AND lease=?2 AND counter=?3
           AND NOT EXISTS(SELECT 1 FROM space_runtime_erasures WHERE space=?1 AND machine=?4 AND (through IS NULL OR through>=?3))",
        params![space.to_string(), lease.to_string(), counter, machine.to_string()],
    ).map(|_| ()).map_err(sql)
}
