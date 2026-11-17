//! Publish durably recorded saved-turn changes through the space journal.
use crate::plugins::jobs::JobDatabase;
use calternal_content::Content;
use calternal_store::{Commit, ErrorCode, IdemWrite, SpaceStore, StampedEvent};
use calternal_types::{AccountId, SpaceId};
use serde_json::json;
use std::sync::Arc;

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
            loop {
                if let Err(error) = publisher.drain(space, &conn).await {
                    tracing::warn!(%space, %error, "saved turn journal publication deferred");
                }
                if !conn.continue_publication() {
                    return;
                }
            }
        });
    }

    async fn drain(&self, space: SpaceId, conn: &Arc<JobDatabase>) -> Result<(), String> {
        let content = self.content.clone();
        let journal = blocking(move || content.live_journal(space)).await?;
        loop {
            let read = conn.clone();
            let events = blocking(move || read_page(&read)).await?;
            let Some(last) = events.last().map(|e| e.seq) else {
                return Ok(());
            };
            for event in events {
                event.publish(&journal).await?;
            }
            let write = conn.clone();
            blocking(move || {
                write
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .execute("DELETE FROM saved_turn_events WHERE seq <= ?1", [last])
            })
            .await?;
        }
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

fn read_page(conn: &JobDatabase) -> Result<Vec<Event>, rusqlite::Error> {
    let conn = conn.lock().unwrap_or_else(|e| e.into_inner());
    let mut stmt = conn.prepare_cached(
        "SELECT seq,event_id,thread,job,account,state,reason_checkpoint FROM saved_turn_events ORDER BY seq LIMIT 64",
    )?;
    stmt.query_map([], |row| {
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
    tokio::task::spawn_blocking(move || run().map_err(|e| e.to_string()))
        .await
        .map_err(|e| e.to_string())?
}
