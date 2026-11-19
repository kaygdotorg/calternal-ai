//! Bounded background discovery, handoff polling and outcome reconciliation.
use super::*;

/// Schedule at most one fixed-size sweep at a time for this process.
pub(crate) fn start<D: Directory>(state: &Shared<D>) {
    let owner = Arc::downgrade(state);
    let mut authority_changes = state.directory.changes();
    tokio::spawn(async move {
        let mut audit_at = now_ms();
        let mut ticks = tokio::time::interval(Duration::from_secs(3));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            let Some(state) = owner.upgrade() else { break };
            let mut changed = false;
            for watcher in &mut authority_changes {
                if watcher.has_changed().unwrap_or(false) {
                    drop(watcher.borrow_and_update());
                    changed = true;
                }
            }
            if changed {
                audit_at = audit_at.saturating_add(1).max(now_ms());
            }
            if let Err(error) = tick(&state, audit_at).await {
                tracing::warn!(message = %error.message, "automation scheduler tick failed");
            }
        }
    });
}

async fn tick<D: Directory>(state: &Shared<D>, audit_at: i64) -> Result<(), Error> {
    let st = state.clone();
    let waiting = blocking(move || {
        if !st.store.appdata().join(PATH).exists() {
            return Ok(Vec::new());
        }
        let mut conn = database(&st.store)?;
        let now = now_ms();
        reconcile(&st.store, &conn, now)?;
        check_access(&st, &mut conn, audit_at)?;
        enqueue_due(&st, &mut conn, now)?;
        let mut stmt = conn
            .prepare(
                "SELECT space,automation,id,scheduled,revision,thread,job,created
            FROM runs WHERE finished IS NULL ORDER BY checked LIMIT ?1",
            )
            .map_err(db_error)?;
        let rows = stmt
            .query_map([BATCH], |row| {
                Ok(Waiting {
                    space: row.get(0)?,
                    automation: row.get(1)?,
                    id: row.get(2)?,
                    scheduled: row.get(3)?,
                    revision: row.get(4)?,
                    thread: row.get(5)?,
                    job: row.get(6)?,
                    created: row.get(7)?,
                })
            })
            .map_err(db_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_error)?;
        drop(stmt);
        for row in &rows {
            conn.execute(
                "UPDATE runs SET checked=?2 WHERE id=?1",
                params![row.id, now],
            )
            .map_err(db_error)?;
        }
        conn.execute(
            "DELETE FROM runs WHERE id IN
            (SELECT id FROM runs WHERE finished < ?1 ORDER BY finished LIMIT ?2)",
            params![now.saturating_sub(RETENTION_MS), BATCH],
        )
        .map_err(db_error)?;
        Ok(rows)
    })
    .await?;
    for waiting in waiting {
        let run = waiting.run()?;
        let result =
            if waiting.job.is_some() || waiting.created < now_ms().saturating_sub(RETENTION_MS) {
                let st = state.clone();
                blocking(move || check_existing(&st, &run, waiting)).await
            } else {
                handoff(state, &run).await.map(|_| ())
            };
        if let Err(error) = result {
            tracing::debug!(message = %error.message, "automation handoff will retry");
        }
    }
    Ok(())
}

struct Waiting {
    space: String,
    automation: String,
    id: String,
    scheduled: Option<i64>,
    revision: String,
    thread: Option<String>,
    job: Option<String>,
    created: i64,
}

impl Waiting {
    fn run(&self) -> Result<Run, Error> {
        Ok(Run {
            space: SpaceId::parse(&self.space)
                .map_err(|_| Error::internal("invalid automation space"))?,
            automation: crate::store::item_id(&self.automation)?,
            id: crate::store::item_id(&self.id)?,
            scheduled: self.scheduled,
            revision: self.revision.clone(),
        })
    }
}

fn check_access<D: Directory>(
    state: &Shared<D>,
    conn: &mut Connection,
    audit_at: i64,
) -> Result<(), Error> {
    let rows = conn
        .prepare(
            "SELECT space,id,creator,revision FROM schedules
         WHERE enabled=1 AND stop IS NULL AND acl_checked<?1
         ORDER BY acl_checked LIMIT ?2",
        )
        .map_err(db_error)?
        .query_map(params![audit_at, BATCH], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    if rows.is_empty() {
        return Ok(());
    }
    let verified = rows
        .into_iter()
        .map(|(space_key, id, creator, revision)| {
            let space = SpaceId::parse(&space_key)
                .map_err(|_| Error::internal("invalid automation space"))?;
            let account = AccountId::parse(&creator)
                .map_err(|_| Error::internal("invalid automation creator"))?;
            let has_access = authority(state, account, space);
            Ok((space_key, id, revision, has_access))
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    for (space, id, revision, has_access) in verified {
        if has_access {
            tx.execute(
                "UPDATE schedules SET acl_checked=?4 WHERE space=?1 AND id=?2 AND revision=?3",
                params![space, id, revision, audit_at],
            )
            .map_err(db_error)?;
        } else {
            tx.execute(
                "UPDATE schedules SET stop=?4,next=NULL WHERE space=?1 AND id=?2 AND revision=?3",
                params![space, id, revision, ACCESS_LOST],
            )
            .map_err(db_error)?;
        }
    }
    tx.commit().map_err(db_error)
}

fn enqueue_due<D: Directory>(
    state: &Shared<D>,
    conn: &mut Connection,
    now: i64,
) -> Result<(), Error> {
    let mut stmt = conn
        .prepare(
            "SELECT space,id,revision,next,creator FROM schedules
        WHERE enabled=1 AND stop IS NULL AND next IS NOT NULL
          AND MAX(next,checked)<=?1 ORDER BY MAX(next,checked) LIMIT ?2",
        )
        .map_err(db_error)?;
    let rows = stmt
        .query_map(params![now, BATCH], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(db_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    drop(stmt);
    for (space_key, id, revision, cursor, creator) in rows {
        conn.execute(
            "UPDATE schedules SET checked=?3 WHERE space=?1 AND id=?2",
            params![space_key, id, now.saturating_add(5_000)],
        )
        .map_err(db_error)?;
        let space =
            SpaceId::parse(&space_key).map_err(|_| Error::internal("invalid automation space"))?;
        let item_id = crate::store::item_id(&id)?;
        let creator = AccountId::parse(&creator)
            .map_err(|_| Error::internal("invalid automation creator"))?;
        if !authority(state, creator, space) {
            conn.execute(
                "UPDATE schedules SET stop=?3,next=NULL WHERE space=?1 AND id=?2",
                params![space_key, id, ACCESS_LOST],
            )
            .map_err(db_error)?;
            continue;
        }
        let item = match state.store.content().get(space, item_id) {
            Ok(item) => item,
            Err(error) if error.code == calternal_types::ErrorCode::NotFound => {
                conn.execute(
                    "DELETE FROM schedules WHERE space=?1 AND id=?2",
                    params![space_key, id],
                )
                .map_err(db_error)?;
                continue;
            }
            Err(error) => return Err(error),
        };
        let Ok((d, _)) = Definition::from_item(&item) else {
            continue;
        };
        if !d.enabled || revision != item.revision {
            // The file won a race with this sweep, or its operation response was
            // interrupted after file commit. Reconcile prospectively.
            register(&state.store, &item, &d)?;
            continue;
        }
        let schedule = d.calendar()?;
        let Some(latest) = schedule.latest(&d.timezone, now).filter(|at| *at >= cursor) else {
            continue;
        };
        let today =
            calternal_core::automation_schedule::same_day(&d.timezone, latest, now) == Some(true);
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(db_error)?;
        let unchanged: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schedules WHERE
            space=?1 AND id=?2 AND revision=?3 AND next=?4 AND stop IS NULL)",
                params![space_key, id, revision, cursor],
                |row| row.get(0),
            )
            .map_err(db_error)?;
        if !unchanged {
            continue;
        }
        if today && queue(&tx, space, item_id, revision, Some(latest), None, now)?.is_none() {
            continue;
        }
        if !today {
            let skipped = AutomationOutcome {
                status: "skipped".into(),
                scheduled_at_ms: Some(latest),
                at_ms: now,
                thread_id: None,
                reason: Some("The scheduled day ended before the run could start.".into()),
            };
            record(&tx, space, item_id, &skipped)?;
        } else if latest > cursor {
            let skipped = AutomationOutcome {
                status: "skipped".into(),
                scheduled_at_ms: Some(cursor),
                at_ms: now,
                thread_id: None,
                reason: Some("Earlier scheduled days ended before runs could start.".into()),
            };
            record(&tx, space, item_id, &skipped)?;
        }
        tx.execute(
            "UPDATE schedules SET next=?3,last_due=?4 WHERE space=?1 AND id=?2",
            params![space_key, id, schedule.next(&d.timezone, latest), latest],
        )
        .map_err(db_error)?;
        if today {
            crate::fault::hit(crate::fault::FaultPoint::AutomationBeforeQueueCommit);
        }
        tx.commit().map_err(db_error)?;
        if today {
            crate::fault::hit(crate::fault::FaultPoint::AutomationAfterQueueCommit);
        }
    }
    Ok(())
}

fn check_existing<D: Directory>(
    state: &Shared<D>,
    run: &Run,
    waiting: Waiting,
) -> Result<(), Error> {
    let conn = database(&state.store)?;
    let Some(job_id) = waiting.job else {
        return finish(
            &conn,
            run,
            "skipped",
            None,
            Some("The run expired before a machine became available."),
        );
    };
    let Some(job) = state.jobs.get(&waiting.space, &job_id)? else {
        return finish(
            &conn,
            run,
            "outcome_unknown",
            waiting.thread,
            Some("The turn outcome is no longer retained."),
        );
    };
    if !authority(state, job.account, run.space) {
        state.jobs.cancel(&waiting.space, &job_id)?;
        state
            .jobs
            .agent_revoke_run(&waiting.space, &job_id, job.runs)?;
        if let Some(machines) = state.store.machines() {
            crate::turns::release(&machines, &job)?;
        }
        return complete(
            &conn,
            run,
            "stopped",
            waiting.thread,
            Some(ACCESS_LOST),
            true,
        );
    }
    if (job.runs == 0 || job.checkpoint["interruption"]["why"] == "automation_day_ended")
        && !within_scheduled_day(&job)
    {
        state.jobs.cancel(&waiting.space, &job_id)?;
        return finish(
            &conn,
            run,
            "skipped",
            waiting.thread,
            Some("The scheduled day ended before the machine was available."),
        );
    }
    if job.state == JobState::Admitted
        && job.runs == 0
        && let Some(machines) = state.store.machines()
    {
        crate::turns::schedule_thread(
            state,
            &machines,
            waiting.space.clone(),
            job.id.clone(),
            job.account,
        );
    }
    let status = match job.state {
        JobState::Done => "completed",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
        JobState::OutcomeUnknown => "outcome_unknown",
        JobState::Interrupted => "interrupted",
        JobState::Waiting => "waiting",
        JobState::Running => "running",
        JobState::Admitted | JobState::RetryWaiting => "queued",
    };
    if job.state.is_final() || job.state == JobState::Interrupted {
        finish(&conn, run, status, waiting.thread, None)
    } else {
        Ok(())
    }
}
