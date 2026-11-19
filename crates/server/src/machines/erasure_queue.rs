//! Durable delivery of erased owners' machine-role storage instructions.
use super::{db, sql, state::State};
use calternal_machine::protocol::{DiscardReport, Down, OWNER_ERASURE_BATCH};
use calternal_types::{Error, Owner};
use rusqlite::{Connection, OptionalExtension as _, params};
use std::collections::HashSet;
use uuid::Uuid;

/// After these failed deletes the copy stays unusable and cold placement can proceed.
pub(super) const RUNTIME_RETRIES: u32 = 3;

/// Load the set used to keep idle polls free of queue reads.
pub(super) fn pending_machines(db: &Connection) -> Result<HashSet<Uuid>, Error> {
    let mut statement = db
        .prepare(
            "SELECT machine FROM machine_erasures UNION SELECT machine FROM space_runtime_erasures",
        )
        .map_err(sql)?;
    statement
        .query_map([], |row| uuid(row, 0))
        .map_err(sql)?
        .collect::<rusqlite::Result<_>>()
        .map_err(sql)
}

/// Record an account's own machine before admitting it, including direct hosts.
/// Account revocation can remove enrolment before startup replays retirement.
pub(super) fn remember_account_machine(
    db: &Connection,
    account: calternal_types::AccountId,
    machine: Uuid,
) -> Result<(), Error> {
    db.execute(
        "INSERT OR IGNORE INTO account_home_locations VALUES(?1,?2)",
        params![account.to_string(), machine.to_string()],
    )
    .map_err(sql)?;
    Ok(())
}

/// Convert placement provenance and any existing discards to durable erasures.
/// This shares the lease retirement transaction, so restart sees either all
/// source locations or all corresponding erase requests.
pub(super) fn queue(
    db: &Connection,
    owner: Owner,
    personal_machines: &[Uuid],
) -> Result<Vec<Uuid>, Error> {
    let id = owner.uuid().to_string();
    let selection = match owner {
        Owner::Account(_) => "SELECT machine,account,'' AS thread FROM account_home_locations WHERE account=?1
            UNION SELECT machine,account,'' FROM workspace_discards WHERE account=?1 AND state IN ('pending','sent','failed')",
        Owner::Space(_) => "SELECT machine,account,thread FROM workspace_locations WHERE space=?1
            UNION SELECT d.machine,d.account,d.thread FROM workspace_discards d
                JOIN leases l ON l.id=d.lease WHERE l.space=?1 AND d.state IN ('pending','sent','failed')",
    };
    let mut insert = db.prepare(&format!(
        "INSERT OR IGNORE INTO machine_erasures(machine,account,thread,request_id)
         SELECT machine,account,thread,lower(hex(randomblob(16))) FROM ({selection}) RETURNING machine"
    )).map_err(sql)?;
    let mut added = insert
        .query_map([&id], |row| uuid(row, 0))
        .map_err(sql)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql)?;
    if let Owner::Account(_) = owner {
        for machine in personal_machines {
            if db
                .execute(
                    "INSERT OR IGNORE INTO machine_erasures(machine,account,thread,request_id)
                 VALUES(?1,?2,'',?3)",
                    params![machine.to_string(), id, Uuid::now_v7().simple().to_string()],
                )
                .map_err(sql)?
                != 0
            {
                added.push(*machine);
            }
        }
        db.execute("DELETE FROM account_home_locations WHERE account=?1", [&id])
            .map_err(sql)?;
        db.execute("DELETE FROM workspace_locations WHERE account=?1", [&id])
            .map_err(sql)?;
    } else {
        db.execute("DELETE FROM workspace_locations WHERE space=?1", [&id])
            .map_err(sql)?;
    }
    let owner_field = match owner {
        Owner::Account(_) => "account",
        Owner::Space(_) => "space",
    };
    let mut runtime = db.prepare(&format!(
        "INSERT INTO space_runtime_erasures(space,machine,request_id,through)
         SELECT space,machine,lower(hex(randomblob(16))),NULL
         FROM (SELECT DISTINCT space,machine FROM space_runtime_locations WHERE {owner_field}=?1) WHERE true
         ON CONFLICT(space,machine) DO UPDATE SET
           request_id=excluded.request_id,through=NULL,failures=0,retry_after_ms=0
         WHERE space_runtime_erasures.through IS NOT NULL RETURNING machine"
    )).map_err(sql)?;
    added.extend(
        runtime
            .query_map([&id], |row| uuid(row, 0))
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?,
    );
    if let Owner::Space(_) = owner {
        db.execute(
            "DELETE FROM space_runtime_checkpoints WHERE space=?1",
            [&id],
        )
        .map_err(sql)?;
    } else {
        db.execute(
            "UPDATE space_runtime_checkpoints AS checkpoint SET acknowledged=NULL,lease=''
             WHERE EXISTS(SELECT 1 FROM space_runtime_erasures erase JOIN leases old ON old.id=checkpoint.lease
                          WHERE erase.space=checkpoint.space AND erase.machine=old.machine AND erase.through IS NULL)", [],
        ).map_err(sql)?;
    }
    db.execute(
        &format!("DELETE FROM space_runtime_locations WHERE {owner_field}=?1"),
        [&id],
    )
    .map_err(sql)?;
    Ok(added)
}

/// Retire only success reports bound to this machine. A failed removal stays
/// pending and becomes eligible again after a short backoff.
pub(super) fn acknowledge(
    db: &Connection,
    machine: Uuid,
    reports: &[DiscardReport],
) -> Result<bool, Error> {
    let machine = machine.to_string();
    for report in reports {
        // SQLite-generated request UUIDs are stored in simple form. The
        // individually queued UUID uses the same canonical representation.
        let request = report.request_id.simple().to_string();
        if report.success {
            db.execute(
                "DELETE FROM machine_erasures WHERE request_id=?1 AND machine=?2",
                params![request, machine],
            )
            .map_err(sql)?;
            let space = db.query_row(
                "DELETE FROM space_runtime_erasures WHERE request_id=?1 AND machine=?2 RETURNING space",
                params![request, machine], |row| row.get::<_, String>(0),
            ).optional().map_err(sql)?;
            if let Some(space) = space {
                db.execute(
                    "DELETE FROM space_runtime_locations WHERE space=?1 AND machine=?2
                     AND NOT EXISTS(SELECT 1 FROM space_runtime_checkpoints WHERE space=?1 AND acknowledged=?2)
                     AND NOT EXISTS(SELECT 1 FROM leases WHERE space=?1 AND machine=?2 AND state!='ended'
                         AND image=?3)",
                    params![space, machine, calternal_machine::sandbox::worker::SPACE_VM_IMAGE],
                ).map_err(sql)?;
            }
        } else {
            db.execute(
                "UPDATE machine_erasures SET retry_after_ms=?3 WHERE request_id=?1 AND machine=?2",
                params![request, machine, db::now_ms() + 10_000],
            )
            .map_err(sql)?;
            db.execute(
                "UPDATE space_runtime_erasures SET retry_after_ms=?3,failures=min(failures+1,?4)
                 WHERE request_id=?1 AND machine=?2",
                params![request, machine, db::now_ms() + 10_000, RUNTIME_RETRIES],
            )
            .map_err(sql)?;
        }
    }
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM machine_erasures WHERE machine=?1 UNION ALL SELECT 1 FROM space_runtime_erasures WHERE machine=?1)",
        [machine],
        |r| r.get(0),
    )
    .map_err(sql)
}

/// Read status without opening or changing a machine session.
pub(super) fn count(db: &Connection, machine: Uuid) -> Result<u64, Error> {
    db.query_row(
        "SELECT (SELECT count(*) FROM machine_erasures WHERE machine=?1) + (SELECT count(*) FROM space_runtime_erasures WHERE machine=?1)",
        [machine.to_string()],
        |r| Ok(r.get::<_, i64>(0)? as u64),
    )
    .map_err(sql)
}

impl super::MachineService {
    /// Summarize the durable queue for the operator, including revoked and
    /// removed machines. One grouped query and in-memory registry lookup.
    pub(super) fn pending_erasures(
        &self,
    ) -> Result<Vec<super::admin::MachineErasureSummary>, Error> {
        let state = self.lock()?;
        let mut query = state
            .db
            .prepare(
                "SELECT machine,count(*),sum(unusable) FROM (
                    SELECT machine,0 AS unusable FROM machine_erasures UNION ALL
                    SELECT machine,failures>=?1 FROM space_runtime_erasures
                 ) GROUP BY machine ORDER BY machine",
            )
            .map_err(sql)?;
        let rows = query
            .query_map([RUNTIME_RETRIES], |row| {
                let machine = uuid(row, 0)?;
                Ok(super::admin::MachineErasureSummary {
                    machine_id: machine,
                    pending: row.get::<_, i64>(1)? as u64,
                    unusable_checkpoints: row.get::<_, i64>(2)? as u64,
                    operator_wipe: state.machines.get(&machine).is_none_or(|m| m.row.revoked),
                })
            })
            .map_err(sql)?;
        rows.collect::<rusqlite::Result<_>>().map_err(sql)
    }
}

impl State {
    /// Add at most one bounded page to the session outbox. Transport ACKs
    /// alone never retire requests. This runs on the blocking sync worker.
    pub(super) fn deliver_erasures(&mut self, machine: Uuid) -> Result<(), Error> {
        if !self.pending_erasure_machines.contains(&machine) {
            return Ok(());
        }
        let batch = {
            let mut q = self.db.prepare_cached(
                "SELECT request_id,account,thread,space,through,failures FROM (
                    SELECT request_id,account,thread,'' AS space,NULL AS through,retry_after_ms,0 AS failures
                    FROM machine_erasures WHERE machine=?1 AND retry_after_ms<=?2
                    UNION ALL
                    SELECT request_id,'','',space,through,retry_after_ms,failures
                    FROM space_runtime_erasures WHERE machine=?1 AND retry_after_ms<=?2
                 ) ORDER BY retry_after_ms,request_id LIMIT ?3"
            ).map_err(sql)?;
            q.query_map(
                params![
                    machine.to_string(),
                    db::now_ms(),
                    OWNER_ERASURE_BATCH as i64
                ],
                |row| {
                    let request_id = uuid(row, 0)?;
                    let space: String = row.get(3)?;
                    if !space.is_empty() {
                        let space_id = space.parse().map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                3,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                        return Ok(Down::EraseSpaceRuntime {
                            request_id,
                            space_id,
                            through: row.get::<_, Option<i64>>(4)?.map(|counter| counter as u64),
                            background: row.get::<_, u32>(5)? >= RUNTIME_RETRIES,
                        });
                    }
                    let thread: String = row.get(2)?;
                    Ok(Down::Erase {
                        request_id,
                        account_id: uuid(row, 1)?,
                        thread_id: if thread.is_empty() {
                            None
                        } else {
                            Some(parse_uuid(thread, 2)?)
                        },
                    })
                },
            )
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?
        };
        let queued: HashSet<_> = self
            .machines
            .get(&machine)
            .and_then(|m| m.live.as_ref())
            .into_iter()
            .flat_map(|m| &m.outbox)
            .filter_map(|e| match e.message {
                Down::Erase { request_id, .. } | Down::EraseSpaceRuntime { request_id, .. } => {
                    Some(request_id)
                }
                _ => None,
            })
            .collect();
        for message in batch {
            if matches!(&message, Down::Erase { request_id, .. } | Down::EraseSpaceRuntime { request_id, .. } if !queued.contains(request_id))
            {
                self.push(machine, message);
            }
        }
        Ok(())
    }
}

/// Decode a stored machine or request identifier.
pub(super) fn uuid(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<Uuid> {
    parse_uuid(row.get(column)?, column)
}

/// Turn a malformed stored UUID into a SQLite error.
pub(super) fn parse_uuid(text: String, column: usize) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(&text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
