//! Publish durably recorded saved-turn changes through the space journal.
use crate::plugins::jobs::JobDatabase;
use calternal_content::Content;
use calternal_store::{Commit, ErrorCode, IdemWrite, SpaceStore, StampedEvent};
use calternal_types::{AccountId, SpaceId};
use serde_json::json;
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;

const PAGE: i64 = 64;
const RETIRE_DELAY: Duration = Duration::from_millis(250);

/// Journal kind shared by publication and caller-scoped delivery.
pub(crate) const KIND: &str = "threads.turn_state_changed";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS saved_turn_events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL,
  thread TEXT NOT NULL,
  job TEXT NOT NULL,
  account TEXT NOT NULL,
  state TEXT NOT NULL,
  reason_checkpoint TEXT NOT NULL,
  created_at INTEGER NOT NULL
) STRICT;
CREATE INDEX IF NOT EXISTS saved_turn_events_age ON saved_turn_events(created_at);
CREATE TRIGGER IF NOT EXISTS saved_turn_admitted AFTER INSERT ON jobs
WHEN NEW.operation = 'agent.turn' AND json_type(NEW.input, '$.thread_request') = 'object'
BEGIN
  DELETE FROM saved_turn_events WHERE created_at < unixepoch() - 604800;
  INSERT INTO saved_turn_events(event_id,thread,job,account,state,reason_checkpoint,created_at)
  VALUES(lower(hex(randomblob(16))),json_extract(NEW.input,'$.request.thread_id'),NEW.id,NEW.account,CASE NEW.state WHEN 'placing' THEN 'admitted' ELSE NEW.state END,__REASON__,unixepoch());
END;
CREATE TRIGGER IF NOT EXISTS saved_turn_transition AFTER UPDATE OF state,checkpoint ON jobs
WHEN ((NEW.state != OLD.state AND NEW.state != 'placing')
  OR (NEW.state='placing' AND json_extract(NEW.checkpoint,'$.placement') IS NOT json_extract(OLD.checkpoint,'$.placement')))
 AND NEW.operation = 'agent.turn'
 AND json_type(NEW.input, '$.thread_request') = 'object'
BEGIN
  DELETE FROM saved_turn_events WHERE created_at < unixepoch() - 604800;
  INSERT INTO saved_turn_events(event_id,thread,job,account,state,reason_checkpoint,created_at)
  VALUES(lower(hex(randomblob(16))),json_extract(NEW.input,'$.request.thread_id'),NEW.id,NEW.account,CASE NEW.state WHEN 'placing' THEN 'admitted' ELSE NEW.state END,__REASON__,unixepoch());
END;
";

/// Build the saved-turn outbox schema once. Triggers capture only the fields
/// needed by the journal and roll back together with the originating change.
pub(crate) fn schema() -> &'static str {
    static SQL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SQL.get_or_init(|| SCHEMA.replace("__REASON__", &crate::turns::reason::sql("NEW.")))
}

/// Publishes through the existing content service. The jobs connection
/// coalesces concurrent wakes without taking an installation-wide lock.
pub(crate) struct Publisher {
    content: Arc<Content>,
}

impl Publisher {
    /// Share the current content journal service. Cost: no I/O or connections.
    pub(crate) fn new(content: Arc<Content>) -> Self {
        Self { content }
    }

    /// Schedule publication after commit. The caller has released the jobs lock.
    /// The task obtains the content journal separately and never holds the jobs
    /// lock during a journal write. Cost: one short per-space lock and at most one task.
    pub(crate) fn wake(self: &Arc<Self>, space: &str, conn: Arc<JobDatabase>) {
        let Ok(space) = SpaceId::parse(space) else {
            return;
        };
        if !conn.wake_publication() {
            return;
        }
        let publisher = self.clone();
        tokio::spawn(async move {
            let mut through = 0;
            let mut retire_at = None;
            let mut journal = None;
            loop {
                let more = match publisher
                    .drain_page(space, &conn, &mut journal, &mut through)
                    .await
                {
                    Ok(more) => more,
                    Err(error) => {
                        tracing::warn!(%space, %error, "saved turn journal publication deferred");
                        false
                    }
                };
                if through > 0 && retire_at.is_none() {
                    retire_at = Some(Instant::now() + RETIRE_DELAY);
                }
                if retire_at.is_some_and(|at| at <= Instant::now()) {
                    let write = conn.clone();
                    let retry = match blocking(move || write.retire_turn_events(through, PAGE))
                        .await
                    {
                        Ok(retry) => retry,
                        Err(error) => {
                            tracing::warn!(%space, %error, "saved turn outbox retirement deferred");
                            true
                        }
                    };
                    retire_at = retry.then(|| Instant::now() + RETIRE_DELAY);
                    if !retry {
                        through = 0;
                    }
                }
                if more || conn.continue_publication(retire_at.is_none()) {
                    continue;
                }
                let Some(at) = retire_at else { return };
                let _ = tokio::time::timeout_at(at, conn.wait_for_publication()).await;
            }
        });
    }

    /// Advance the in-memory frontier only after each durable journal
    /// publication. A restart begins at zero and the event IDs deduplicate.
    async fn drain_page(
        &self,
        space: SpaceId,
        conn: &Arc<JobDatabase>,
        journal: &mut Option<SpaceStore>,
        through: &mut i64,
    ) -> Result<bool, String> {
        let read = conn.clone();
        let after = *through;
        let events = blocking(move || read_page(&read, after)).await?;
        let more = events.len() == PAGE as usize;
        if events.is_empty() {
            return Ok(false);
        }
        if journal.is_none() {
            let content = self.content.clone();
            *journal = Some(blocking(move || content.live_journal(space)).await?);
        }
        let journal = journal.as_ref().expect("opened above");
        for event in events {
            let seq = event.seq;
            event.publish(journal).await?;
            *through = seq;
        }
        Ok(more)
    }
}

struct Event {
    seq: i64,
    event_id: String,
    thread: String,
    job: String,
    account: AccountId,
    state: String,
    checkpoint: String,
}

impl Event {
    async fn publish(self, journal: &SpaceStore) -> Result<(), String> {
        let key = format!("threads/state/{}", self.event_id);
        let job_state = crate::plugins::job_sql::parse_state(&self.state);
        let checkpoint = serde_json::from_str(&self.checkpoint).map_err(|e| e.to_string())?;
        let reason = crate::turns::reason::describe(Some(job_state), &checkpoint, true);
        let state = super::turn_status::label(job_state);
        let result = journal
            .commit(Commit {
                events: vec![StampedEvent {
                    author: self.account,
                    kind: KIND.into(),
                    item: self.thread,
                    data: json!({"job_id":self.job,"state":state,"job_state":self.state,"turn_reason":reason}),
                    installation: None,
                    request_id: None,
                }],
                idempotency: Some(IdemWrite {
                    account: self.account,
                    key,
                    action: KIND.into(),
                    payload_hash: self.event_id,
                    result: json!({}),
                }),
                durable: true,
            })
            .await;
        match result {
            Ok(_) => Ok(()),
            Err(error) if error.code == ErrorCode::IdempotencyConflict => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
}

fn read_page(conn: &JobDatabase, after: i64) -> Result<Vec<Event>, rusqlite::Error> {
    let conn = conn.lock()?;
    let mut stmt = conn.prepare_cached(
        "SELECT seq,event_id,thread,job,account,state,reason_checkpoint
         FROM saved_turn_events WHERE seq>?1 ORDER BY seq LIMIT ?2",
    )?;
    stmt.query_map([after, PAGE], |row| {
        let account = row.get::<_, String>(4)?.parse().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(Event {
            seq: row.get(0)?,
            event_id: row.get(1)?,
            thread: row.get(2)?,
            job: row.get(3)?,
            account,
            state: row.get(5)?,
            checkpoint: row.get(6)?,
        })
    })?
    .collect()
}

async fn blocking<T: Send + 'static, E: std::fmt::Display>(
    run: impl FnOnce() -> Result<T, E> + Send + 'static,
) -> Result<T, String> {
    crate::blocking::spawn(move || run().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saved_turn_retirement_batches_published_rows_and_replays_once() {
        let dir = tempfile::tempdir().unwrap();
        let roots = calternal_roots::Roots {
            appdata: dir.path().join("appdata"),
            userdata_ssd: dir.path().join("ssd"),
            userdata_hdd: dir.path().join("hdd"),
        };
        let content = Arc::new(
            Content::open(
                roots.clone(),
                dir.path().join("secrets"),
                Duration::from_secs(60),
                calternal_content::DEFAULT_RECEIPT_RETENTION,
                |_| {},
            )
            .unwrap(),
        );
        let jobs = crate::plugins::Jobs::open(&roots.appdata).unwrap();
        let space = SpaceId::new();
        let conn = jobs.conn(&space.to_string()).unwrap();
        let account = AccountId::new().to_string();
        let insert = |seq: i64| {
            conn.lock()
                .unwrap()
                .execute(
                    "INSERT INTO saved_turn_events VALUES
                        (?1,'evt-'||?1,'thread','job',?2,'admitted','{}',unixepoch())",
                    rusqlite::params![seq, account],
                )
                .unwrap();
        };
        insert(1);
        insert(2);
        let publisher = Publisher::new(content.clone());
        let mut through = 0;
        let mut journal = None;
        assert!(
            !publisher
                .drain_page(space, &conn, &mut journal, &mut through)
                .await
                .unwrap()
        );
        assert_eq!(through, 2);
        assert_eq!(read_page(&conn, 0).unwrap().len(), 2);
        let mut after_restart = 0;
        publisher
            .drain_page(space, &conn, &mut journal, &mut after_restart)
            .await
            .unwrap();
        assert_eq!(after_restart, through);
        assert_eq!(content.journal(space).unwrap().len(), 2);
        insert(3);

        // A foreground transaction holds the SQLite write lock. The
        // retirement connection returns promptly and leaves all rows.
        let held = conn.lock().unwrap();
        held.execute_batch("BEGIN IMMEDIATE").unwrap();
        std::thread::scope(|scope| {
            let (send, receive) = std::sync::mpsc::channel();
            let db = &conn;
            scope.spawn(move || {
                let _ = send.send(db.retire_turn_events(through, 1));
            });
            let result = receive.recv_timeout(Duration::from_secs(2));
            held.execute_batch("ROLLBACK").unwrap();
            drop(held);
            assert!(result.unwrap().unwrap());
        });
        assert_eq!(read_page(&conn, 0).unwrap().len(), 3);
        assert!(conn.retire_turn_events(through, 1).unwrap());
        assert_eq!(read_page(&conn, 0).unwrap()[0].seq, 2);
        assert!(!conn.retire_turn_events(through, 1).unwrap());
        let remaining = read_page(&conn, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].seq, 3);
    }
}
