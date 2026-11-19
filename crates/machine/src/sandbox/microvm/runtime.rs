//! A bounded machine-side outbox for monotonic guest clock observations.

use crate::protocol::{GuestRuntime, MAX_GUEST_RUNTIME, RuntimeState};
use crate::sandbox::{SandboxError, SandboxResult};
use calternal_store::migrations::{Schema, migrate};
use rusqlite::{Connection, params};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use uuid::Uuid;

const MAX_RECEIPTS: i64 = 256;

/// Machine-owned authority for guest clock receipts.
pub const SCHEMA: Schema<'static> = Schema::new(
    "microvm/runtime.db",
    1,
    &["CREATE TABLE guest_runtime(
         generation TEXT PRIMARY KEY,
         anchor BLOB NOT NULL,
         running_ns INTEGER NOT NULL CHECK(running_ns>=0),
         state INTEGER NOT NULL CHECK(state BETWEEN 0 AND 2)
       ) STRICT, WITHOUT ROWID;"],
    &[],
);

#[derive(Clone)]
pub(super) struct Journal(PathBuf);

pub(super) struct Clock {
    anchor: GuestRuntime,
    started: Instant,
    journal: Journal,
}

impl Clock {
    pub(super) async fn begin(anchor: GuestRuntime, journal: Journal) -> SandboxResult<Self> {
        let started = Instant::now();
        journal
            .run(move |db| {
                let tx = db.transaction()?;
                let count: i64 =
                    tx.query_row("SELECT count(*) FROM guest_runtime", [], |r| r.get(0))?;
                if count >= MAX_RECEIPTS {
                    return Ok(false);
                }
                let bytes = serde_json::to_vec(&anchor)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
                tx.execute(
                    "INSERT INTO guest_runtime VALUES(?1,?2,0,0)",
                    params![anchor.generation.to_string(), bytes],
                )?;
                tx.commit()?;
                Ok(true)
            })
            .await?
            .then_some(())
            .ok_or_else(|| SandboxError("guest clock outbox is full".into()))?;
        Ok(Self {
            anchor,
            started,
            journal,
        })
    }

    pub(super) fn sample(&self) -> (Uuid, u64) {
        let running_ns = u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX);
        (self.anchor.generation, running_ns)
    }

    pub(super) async fn end(self, clean: bool) -> SandboxResult<()> {
        let (id, running_ns) = self.sample();
        self.journal.run(move |db| {
            if clean {
                db.execute(
                    "UPDATE guest_runtime SET running_ns=max(running_ns,?2),state=1 WHERE generation=?1 AND state=0",
                    params![id.to_string(), signed(running_ns)],
                )?;
            } else {
                db.execute("UPDATE guest_runtime SET state=2 WHERE generation=?1 AND state=0",
                           [id.to_string()])?;
            }
            Ok(())
        }).await
    }
}

impl Journal {
    pub(super) async fn open(path: PathBuf) -> SandboxResult<Self> {
        let ledger = Self(path);
        ledger
            .run(|db| {
                migrate(db, SCHEMA)
                    .map_err(|err| rusqlite::Error::ToSqlConversionFailure(Box::new(err)))?;
                // The machine already holds its runtime lock. A previous process's
                // unfinished interval has no trustworthy stop timestamp.
                db.execute("UPDATE guest_runtime SET state=2 WHERE state=0", [])?;
                Ok(())
            })
            .await?;
        Ok(ledger)
    }

    pub(super) async fn report(
        &self,
        samples: Vec<(Uuid, u64)>,
    ) -> SandboxResult<Vec<GuestRuntime>> {
        self.run(move |db| {
            let tx = db.transaction()?;
            {
                let mut update = tx.prepare_cached(
                    "UPDATE guest_runtime SET running_ns=?2
                     WHERE generation=?1 AND state=0 AND running_ns<?2",
                )?;
                for (generation, elapsed) in samples {
                    update.execute(params![generation.to_string(), signed(elapsed)])?;
                }
            }
            let reports = {
                let mut read = tx.prepare(
                    "SELECT anchor,running_ns,state FROM guest_runtime
                     ORDER BY state=0,generation LIMIT ?1",
                )?;
                read.query_map([MAX_GUEST_RUNTIME as i64], |row| {
                    let anchor: Vec<u8> = row.get(0)?;
                    let mut report: GuestRuntime =
                        serde_json::from_slice(&anchor).map_err(|err| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Blob,
                                Box::new(err),
                            )
                        })?;
                    report.running_ns = row.get::<_, i64>(1)? as u64;
                    report.state = match row.get::<_, i64>(2)? {
                        1 => RuntimeState::Stopped,
                        2 => RuntimeState::Unknown,
                        _ => RuntimeState::Running,
                    };
                    Ok(report)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
            };
            tx.commit()?;
            Ok(reports)
        })
        .await
    }

    pub(super) async fn acknowledge(&self, reports: &[GuestRuntime]) -> SandboxResult<()> {
        let terminal: Vec<_> = reports
            .iter()
            .filter(|report| report.state != RuntimeState::Running)
            .map(|report| (report.generation, report.running_ns))
            .collect();
        if terminal.is_empty() {
            return Ok(());
        }
        self.run(move |db| {
            let tx = db.transaction()?;
            {
                let mut delete = tx.prepare_cached(
                    "DELETE FROM guest_runtime WHERE generation=?1 AND state!=0 AND running_ns=?2",
                )?;
                for (generation, elapsed) in terminal {
                    delete.execute(params![generation.to_string(), signed(elapsed)])?;
                }
            }
            tx.commit()
        })
        .await
    }

    async fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Connection) -> rusqlite::Result<T> + Send + 'static,
    ) -> SandboxResult<T> {
        let path = self.0.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = calternal_fsio::private::file(Path::new(&path)) {
                return Err(SandboxError::new(error));
            }
            let mut db = Connection::open(&path).map_err(SandboxError::new)?;
            db.busy_timeout(Duration::from_secs(5))
                .map_err(SandboxError::new)?;
            db.pragma_update(None, "synchronous", "FULL")
                .map_err(SandboxError::new)?;
            f(&mut db).map_err(SandboxError::new)
        })
        .await
        .map_err(SandboxError::new)?
    }
}

fn signed(ns: u64) -> i64 {
    i64::try_from(ns).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::ResourceAssignment;
    use calternal_types::{AccountId, ResourceOwner};

    #[tokio::test]
    async fn unresolved_clock_retains_the_last_confirmed_sample_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime.db");
        let journal = Journal::open(path.clone()).await.unwrap();
        let account_id = AccountId::new();
        let anchor = GuestRuntime {
            account_id,
            generation: Uuid::new_v4(),
            resource_owner: Some(ResourceOwner::Account(account_id)),
            assignment: ResourceAssignment {
                lease_id: Uuid::new_v4(),
                fence: 1,
            },
            started_at_ms: 120,
            running_ns: 0,
            state: RuntimeState::Running,
        };
        let clock = Clock::begin(anchor, journal.clone()).await.unwrap();
        let ns = 23_000_000_018;
        assert_eq!(
            journal.report(vec![(anchor.generation, ns)]).await.unwrap()[0].running_ns,
            ns
        );
        drop(clock);
        let restarted = Journal::open(path).await.unwrap();
        let receipts = restarted.report(Vec::new()).await.unwrap();
        assert_eq!(
            receipts,
            [GuestRuntime {
                running_ns: ns,
                state: RuntimeState::Unknown,
                ..anchor
            }]
        );
        assert_eq!(restarted.report(Vec::new()).await.unwrap(), receipts);
        restarted.acknowledge(&receipts).await.unwrap();
        assert!(restarted.report(Vec::new()).await.unwrap().is_empty());
    }
}
