//! SQLite schedule cursor and recoverable thread handoffs.
use super::{definitions::Definition, *};
mod worker;
use calternal_core::{AutomationOutcome, AutomationTarget, JobState};
use calternal_store::migrations::{Schema, migrate};
use calternal_types::Capability;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use uuid::Uuid;
pub(crate) use worker::start;

const MAX_QUEUED: i64 = 32;
const RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const BATCH: i64 = 32;
const ACCESS_LOST: &str = "The creator no longer has access to run automations in this space.";

pub(crate) const PATH: &str = "system/automations.db";

pub(crate) const SCHEMA: Schema<'static> = Schema::new("automation scheduler", 1, &["
CREATE TABLE schedules (
    space TEXT NOT NULL, id TEXT NOT NULL, revision TEXT NOT NULL,
    creator TEXT NOT NULL, epoch TEXT NOT NULL, enabled INTEGER NOT NULL, next INTEGER,
    stop TEXT, last TEXT, last_due INTEGER, checked INTEGER NOT NULL DEFAULT 0, acl_checked INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(space,id)
) WITHOUT ROWID;
CREATE TABLE watched_spaces (space TEXT PRIMARY KEY, checked INTEGER NOT NULL DEFAULT 0) WITHOUT ROWID;
CREATE INDEX watched_order ON watched_spaces(checked);
CREATE INDEX schedules_access ON schedules(acl_checked) WHERE enabled=1 AND stop IS NULL;
CREATE INDEX schedules_due ON schedules(MAX(next,checked)) WHERE enabled=1 AND stop IS NULL AND next IS NOT NULL;
CREATE TABLE runs (
    space TEXT NOT NULL, automation TEXT NOT NULL, id TEXT PRIMARY KEY,
    manual_key TEXT, scheduled INTEGER, created INTEGER NOT NULL,
    revision TEXT NOT NULL, thread TEXT, job TEXT, finished INTEGER, checked INTEGER NOT NULL DEFAULT 0, claimed_until INTEGER NOT NULL DEFAULT 0, machine TEXT,
    UNIQUE(space,automation,scheduled), UNIQUE(space,manual_key)
);
CREATE INDEX runs_waiting ON runs(space,automation,created) WHERE finished IS NULL;
CREATE INDEX runs_poll ON runs(checked) WHERE finished IS NULL;
CREATE INDEX runs_sweep ON runs(finished) WHERE finished IS NOT NULL;
"], &[]);

fn database(store: &crate::store::Store) -> Result<Connection, Error> {
    let conn = Connection::open(store.appdata().join(PATH)).map_err(db_error)?;
    conn.busy_timeout(Duration::from_secs(3))
        .map_err(db_error)?;
    conn.execute_batch("PRAGMA synchronous=FULL;")
        .map_err(db_error)?;
    migrate(&conn, SCHEMA).map_err(db_error)?;
    Ok(conn)
}

fn db_error(error: rusqlite::Error) -> Error {
    tracing::warn!(%error, "automation scheduler storage");
    Error::internal("automation scheduler storage failed")
}

#[derive(Default)]
pub(super) struct Status {
    pub(super) next_run_ms: Option<i64>,
    pub(super) stop_reason: Option<String>,
    pub(super) last: Option<AutomationOutcome>,
}

pub(super) fn status(
    store: &crate::store::Store,
    space: SpaceId,
    id: Uuid,
) -> Result<Status, Error> {
    if !store.appdata().join(PATH).exists() {
        return Ok(Status::default());
    }
    let conn = database(store)?;
    let row = conn
        .query_row(
            "SELECT next, stop, last FROM schedules WHERE space=?1 AND id=?2",
            params![space.to_string(), id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, Option<String>>(2)?)),
        )
        .optional()
        .map_err(db_error)?;
    row.map(|(next, stop, last)| decode_status(next, stop, last))
        .transpose()
        .map(Option::unwrap_or_default)
}

pub(super) fn statuses(
    store: &crate::store::Store,
    space: SpaceId,
) -> Result<BTreeMap<String, Status>, Error> {
    if !store.appdata().join(PATH).exists() {
        return Ok(BTreeMap::new());
    }
    let conn = database(store)?;
    let mut stmt = conn
        .prepare("SELECT id, next, stop, last FROM schedules WHERE space=?1")
        .map_err(db_error)?;
    let mut rows = stmt.query([space.to_string()]).map_err(db_error)?;
    let mut statuses = BTreeMap::new();
    while let Some(row) = rows.next().map_err(db_error)? {
        statuses.insert(
            row.get(0).map_err(db_error)?,
            decode_status(
                row.get(1).map_err(db_error)?,
                row.get(2).map_err(db_error)?,
                row.get(3).map_err(db_error)?,
            )?,
        );
    }
    Ok(statuses)
}

fn decode_status(
    next_run_ms: Option<i64>,
    stop_reason: Option<String>,
    last: Option<String>,
) -> Result<Status, Error> {
    let last = last
        .map(|value| serde_json::from_str(&value))
        .transpose()
        .map_err(|_| Error::internal("invalid automation status"))?;
    Ok(Status {
        next_run_ms,
        stop_reason,
        last,
    })
}

pub(super) fn register(
    store: &crate::store::Store,
    item: &calternal_content::Item,
    d: &Definition,
) -> Result<(), Error> {
    let mut conn = database(store)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let space = item.space.to_string();
    let id = item.id.to_string();
    let previous = tx
        .query_row(
            "SELECT epoch,last_due FROM schedules WHERE space=?1 AND id=?2",
            params![space, id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()
        .map_err(db_error)?;
    let epoch = d.schedule_epoch.to_string();
    let reset = previous.as_ref().is_none_or(|(old, _)| old != &epoch);
    let floor = previous.and_then(|(_, floor)| floor);
    let cursor = if d.enabled {
        schedule(d)?.next(&d.timezone, d.schedule_start_ms.max(floor.unwrap_or(0)))
    } else {
        None
    };
    tx.execute("INSERT INTO schedules(space,id,revision,creator,enabled,next,epoch)
        VALUES(?1,?2,?3,?4,?5,?6,?8)
        ON CONFLICT(space,id) DO UPDATE SET revision=excluded.revision,enabled=excluded.enabled,epoch=excluded.epoch,
          next=CASE WHEN ?7 THEN excluded.next ELSE schedules.next END,
          stop=CASE WHEN ?9 THEN NULL ELSE schedules.stop END
        WHERE schedules.revision<>excluded.revision",
        params![space,id,item.revision,d.creator.to_string(),d.enabled,cursor,reset,epoch,d.clear_stop]).map_err(db_error)?;
    if reset || !d.enabled {
        tx.execute(
            "UPDATE runs SET finished=?3 WHERE space=?1 AND automation=?2 AND job IS NULL
            AND finished IS NULL AND manual_key IS NULL",
            params![space, id, now_ms()],
        )
        .map_err(db_error)?;
    }
    tx.commit().map_err(db_error)
}

pub(super) fn watch(store: &crate::store::Store, space: SpaceId) -> Result<(), Error> {
    database(store)?
        .execute(
            "INSERT OR IGNORE INTO watched_spaces(space) VALUES(?1)",
            [space.to_string()],
        )
        .map_err(db_error)?;
    Ok(())
}

fn reconcile(store: &crate::store::Store, conn: &Connection, now: i64) -> Result<(), Error> {
    let key: Option<String> = conn
        .query_row(
            "SELECT space FROM watched_spaces ORDER BY checked LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    let Some(key) = key else { return Ok(()) };
    conn.execute(
        "UPDATE watched_spaces SET checked=?2 WHERE space=?1",
        params![key, now],
    )
    .map_err(db_error)?;
    let space = SpaceId::parse(&key).map_err(|_| Error::internal("invalid automation space"))?;
    let content = store.content();
    let known: std::collections::BTreeMap<String, String> = conn
        .prepare("SELECT id,revision FROM schedules WHERE space=?1")
        .map_err(db_error)?
        .query_map([&key], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(db_error)?
        .collect::<Result<_, _>>()
        .map_err(db_error)?;
    for row in content.list(space, KIND, 128)? {
        if known.get(&row.id.to_string()) == Some(&row.revision) {
            continue;
        }
        let item = content.get(space, row.id)?;
        if let Ok((definition, _)) = Definition::from_item(&item) {
            register(store, &item, &definition)?;
        }
    }
    Ok(())
}

pub(super) fn remove(store: &crate::store::Store, space: SpaceId, id: Uuid) -> Result<(), Error> {
    let mut conn = database(store)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    tx.execute(
        "DELETE FROM schedules WHERE space=?1 AND id=?2",
        params![space.to_string(), id.to_string()],
    )
    .map_err(db_error)?;
    tx.execute(
        "UPDATE runs SET finished=?3 WHERE space=?1 AND automation=?2 AND job IS NULL
        AND finished IS NULL",
        params![space.to_string(), id.to_string(), now_ms()],
    )
    .map_err(db_error)?;
    tx.commit().map_err(db_error)
}

fn schedule(d: &Definition) -> Result<Schedule, Error> {
    Schedule::from_expression(&d.schedule)
        .ok_or_else(|| Error::unavailable("unsupported calendar expression"))
}

fn authority<D: Directory>(state: &Shared<D>, account: AccountId, space: SpaceId) -> bool {
    state
        .directory
        .capabilities(account, space)
        .is_some_and(|caps| {
            caps.contains(Capability::ContentWrite) && caps.contains(Capability::PluginsUse)
        })
}

pub(super) fn validate_machine<D: Directory>(
    state: &Shared<D>,
    creator: AccountId,
    raw: &str,
) -> Result<(), Error> {
    let id = Uuid::parse_str(raw).map_err(|_| Error::invalid_input("invalid machine UUID"))?;
    state
        .store
        .machines()
        .ok_or_else(|| Error::unavailable("machines are unavailable"))?
        .check_target(creator, Some(id), false)?;
    Ok(())
}

#[derive(Clone)]
struct Run {
    space: SpaceId,
    automation: Uuid,
    id: Uuid,
    scheduled: Option<i64>,
    revision: String,
}

fn queue(
    tx: &rusqlite::Transaction<'_>,
    space: SpaceId,
    automation: Uuid,
    revision: String,
    when: Option<i64>,
    manual_key: Option<&str>,
    now: i64,
) -> Result<Option<Run>, Error> {
    let space_key = space.to_string();
    let item_id = automation.to_string();
    let (count, overlaps): (i64, bool) = tx.query_row(
        "SELECT COUNT(job IS NULL OR NULL), COALESCE(MAX(automation=?2),0) FROM runs WHERE space=?1 AND finished IS NULL",
        params![space_key,item_id], |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(db_error)?;
    if count >= MAX_QUEUED || overlaps {
        return Ok(None);
    }
    let id = Uuid::now_v7();
    tx.execute(
        "INSERT INTO runs(space,automation,id,manual_key,scheduled,created,revision)
        VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            space_key,
            item_id,
            id.to_string(),
            manual_key,
            when,
            now,
            revision
        ],
    )
    .map_err(db_error)?;
    Ok(Some(Run {
        space,
        automation,
        id,
        scheduled: when,
        revision,
    }))
}

fn outcome(
    run: &Run,
    status: &str,
    thread: Option<String>,
    reason: Option<String>,
) -> AutomationOutcome {
    AutomationOutcome {
        status: status.into(),
        scheduled_at_ms: run.scheduled,
        at_ms: now_ms(),
        thread_id: thread,
        reason,
    }
}

fn record(
    conn: &Connection,
    space: SpaceId,
    automation: Uuid,
    outcome: &AutomationOutcome,
) -> Result<(), Error> {
    let last = serde_json::to_string(outcome)
        .map_err(|_| Error::internal("invalid automation outcome"))?;
    conn.execute(
        "UPDATE schedules SET last=?3 WHERE space=?1 AND id=?2",
        params![space.to_string(), automation.to_string(), last],
    )
    .map_err(db_error)?;
    Ok(())
}

pub(super) async fn manual<D: Directory>(
    state: Shared<D>,
    who: Identity,
    session: Session,
    space: SpaceId,
    rid: &str,
    input: AutomationTarget,
) -> Result<Value, Error> {
    if matches!(session, Session::Agent { .. }) {
        return Err(Error::forbidden("a person must start the automation"));
    }
    let id = crate::store::item_id(&input.id)?;
    let st = state.clone();
    let rid = format!("{}/{}", who.id, rid);
    let (run, saved) = blocking(move || {
        let item = st.store.content().get(space,id)?;
        let (d, _) = Definition::from_item(&item)?;
        if status(&st.store,space,id)?.stop_reason.is_some() {
            return Err(Error::conflict("enable this stopped automation before running it"));
        }
        if d.creator != who.id || !authority(&st, who.id, space) {
            return Err(Error::forbidden("only the creator can start this automation"));
        }
        let mut conn = database(&st.store)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(db_error)?;
        let existing = tx.query_row(
            "SELECT id,revision,thread,finished,automation FROM runs WHERE space=?1 AND manual_key=?2",
            params![space.to_string(),rid],
            |row| Ok((row.get::<_, String>(0)?,row.get::<_, String>(1)?,row.get::<_, Option<String>>(2)?,row.get::<_, Option<i64>>(3)?,row.get::<_, String>(4)?)),
        ).optional().map_err(db_error)?;
        if let Some((run,revision,thread,finished,automation)) = existing {
            if automation != id.to_string() { return Err(Error::request_reused()); }
            if finished.is_some() && thread.is_none() {
                return Err(Error::conflict("the original manual run expired"));
            }
            return Ok((Run { space, automation:id, id: crate::store::item_id(&run)?, scheduled:None, revision }, thread));
        }
        let run = queue(&tx, space, id, item.revision, None, Some(&rid), now_ms())?
            .ok_or_else(|| Error::unavailable("the automation run queue is full or this automation is already running"))?;
        tx.commit().map_err(db_error)?;
        Ok((run,None))
    }).await?;
    if let Some(thread) = saved {
        return Ok(json!({"thread_id":thread}));
    }
    let thread = handoff(&state, &run).await?.ok_or_else(|| {
        Error::unavailable("the automation is queued until its machine is available")
    })?;
    Ok(json!({"thread_id":thread}))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("automation worker stopped"))?
}

async fn handoff<D: Directory>(state: &Shared<D>, run: &Run) -> Result<Option<String>, Error> {
    let st = state.clone();
    let pending = run.clone();
    let Some((d, body, machine)) = blocking(move || {
        let run = pending;
        let conn = database(&st.store)?;
        let item = match st.store.content().get(run.space, run.automation) {
            Ok(item) => item,
            Err(error) if error.code == calternal_types::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let (d, body) = match Definition::from_item(&item) {
            Ok(pair) => pair,
            Err(error) if error.code == calternal_types::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !authority(&st, d.creator, run.space) {
            stop(&conn, &run, ACCESS_LOST)?;
            return Ok(None);
        }
        let live: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runs r JOIN schedules s ON s.space=r.space AND s.id=r.automation
            WHERE r.id=?1 AND r.finished IS NULL AND r.job IS NULL AND s.stop IS NULL)",
                [run.id.to_string()],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !live {
            return Ok(None);
        }
        if item.revision != run.revision || (!d.enabled && run.scheduled.is_some()) {
            finish(
                &conn,
                &run,
                "skipped",
                None,
                Some("The automation changed before the run started."),
            )?;
            return Ok(None);
        }
        if run.scheduled.is_some_and(|at| {
            calternal_core::automation_schedule::same_day(&d.timezone, at, now_ms()) != Some(true)
        }) {
            finish(
                &conn,
                &run,
                "skipped",
                None,
                Some("The machine was unavailable on the scheduled day."),
            )?;
            return Ok(None);
        }
        if let Err(error) =
            st.store
                .content()
                .verify_file(run.space, KIND, run.automation, &run.revision)
        {
            if error
                .details
                .as_ref()
                .is_some_and(|details| details["reason"] == "external_change")
            {
                stop(
                    &conn,
                    &run,
                    "The automation file was changed outside Calternal.",
                )?;
                return Ok(None);
            }
            return Err(error);
        }
        let existing: Option<String> = conn
            .query_row(
                "SELECT machine FROM runs WHERE id=?1",
                [run.id.to_string()],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        let pin = d
            .machine_id
            .as_deref()
            .map(Uuid::parse_str)
            .transpose()
            .map_err(|_| Error::internal("invalid saved machine"))?;
        let candidate = match existing {
            Some(machine) => Some(machine),
            None => st
                .store
                .machines()
                .map(|m| m.automation_target(d.creator, pin))
                .transpose()?
                .flatten()
                .map(|id| id.to_string()),
        };
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        let now = now_ms();
        let claimed = conn
            .execute(
                "UPDATE runs SET claimed_until=?3,machine=COALESCE(machine,?2)
            WHERE id=?1 AND finished IS NULL AND job IS NULL AND claimed_until<=?4
                AND (machine IS NULL OR machine=?2)",
                params![
                    run.id.to_string(),
                    candidate,
                    now.saturating_add(30_000),
                    now
                ],
            )
            .map_err(db_error)?;
        Ok((claimed != 0).then(|| {
            crate::fault::hit(crate::fault::FaultPoint::AutomationAfterClaim);
            (d, body, candidate)
        }))
    })
    .await?
    else {
        return Ok(None);
    };
    let (thread, job) = crate::threads::automation::admit(
        state,
        Identity { id: d.creator },
        run.space,
        run.automation,
        run.id,
        d.title,
        body,
        d.mode,
        Some(machine),
        run.scheduled,
        d.timezone,
    )
    .await?;
    let st = state.clone();
    let run = run.clone();
    let (saved_thread, saved_job) = (thread.clone(), job.clone());
    let space = run.space;
    let admitted = blocking(move || {
        let mut conn = database(&st.store)?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let changed = tx
            .execute(
                "UPDATE runs SET thread=?2,job=?3 WHERE id=?1 AND finished IS NULL",
                params![run.id.to_string(), saved_thread, saved_job],
            )
            .map_err(db_error)?;
        if changed != 0 {
            record(
                &tx,
                run.space,
                run.automation,
                &outcome(&run, "queued", Some(saved_thread), None),
            )?;
        }
        tx.commit().map_err(db_error)?;
        Ok(changed != 0)
    })
    .await?;
    if !admitted {
        let st = state.clone();
        let space = space.to_string();
        blocking(move || st.jobs.cancel(&space, &job).map(|_| ())).await?;
        return Ok(None);
    }
    crate::fault::hit(crate::fault::FaultPoint::AutomationAfterRelease);
    if let Some(machines) = state.store.machines() {
        crate::turns::schedule_thread(state, &machines, space.to_string(), job, d.creator);
    }
    Ok(Some(thread))
}

/// Recheck the job's release fence and time limit immediately before the first attempt.
pub(crate) fn check_launch<D: Directory>(
    state: &Shared<D>,
    job: &crate::plugins::jobs::Job,
    space: SpaceId,
) -> Result<bool, Error> {
    let origin = &job.input["thread_request"]["automation_run"];
    let Some(run) = origin["id"].as_str() else {
        return Ok(true);
    };
    if !authority(state, job.account, space) {
        return Ok(false);
    }
    let released: bool = database(&state.store)?.query_row(
        "SELECT EXISTS(SELECT 1 FROM runs r JOIN schedules s ON s.space=r.space AND s.id=r.automation
            WHERE r.id=?1 AND r.space=?2 AND r.job=?3 AND r.finished IS NULL
              AND s.stop IS NULL AND s.revision=r.revision
              AND (r.scheduled IS NULL OR s.enabled=1))",
        params![run,job.space,job.id], |row| row.get(0),
    ).map_err(db_error)?;
    Ok(released && within_scheduled_day(job))
}

/// Whether the first placement is still within the scheduled civil day.
/// Reads only the saved turn input and bundled rules.
pub(crate) fn within_scheduled_day(job: &crate::plugins::jobs::Job) -> bool {
    let origin = &job.input["thread_request"]["automation_run"];
    origin["scheduled"].as_i64().is_none_or(|at| {
        origin["timezone"]
            .as_str()
            .and_then(|zone| calternal_core::automation_schedule::same_day(zone, at, now_ms()))
            == Some(true)
    })
}

fn stop(conn: &Connection, run: &Run, reason: &str) -> Result<(), Error> {
    complete(conn, run, "stopped", None, Some(reason), true)
}

fn finish(
    conn: &Connection,
    run: &Run,
    status: &str,
    thread: Option<String>,
    reason: Option<&str>,
) -> Result<(), Error> {
    complete(conn, run, status, thread, reason, false)
}

fn complete(
    conn: &Connection,
    run: &Run,
    status: &str,
    thread: Option<String>,
    reason: Option<&str>,
    stop: bool,
) -> Result<(), Error> {
    let tx = conn.unchecked_transaction().map_err(db_error)?;
    let changed = tx
        .execute(
            "UPDATE runs SET finished=?2 WHERE id=?1 AND finished IS NULL",
            params![run.id.to_string(), now_ms()],
        )
        .map_err(db_error)?;
    if changed != 0 {
        if stop {
            tx.execute(
                "UPDATE schedules SET stop=?3,next=NULL WHERE space=?1 AND id=?2",
                params![run.space.to_string(), run.automation.to_string(), reason],
            )
            .map_err(db_error)?;
        }
        record(
            &tx,
            run.space,
            run.automation,
            &outcome(run, status, thread, reason.map(str::to_owned)),
        )?;
    }
    tx.commit().map_err(db_error)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod contention;
