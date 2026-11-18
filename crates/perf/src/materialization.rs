//! Read-only confirmation after file and directory durability, outside ack timing.
use crate::http::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

/// Report metric independent of HTTP save and live delivery time.
pub(crate) const METRIC: &str = "userdata.ack_to_durable_file";

/// A placement observation. On timeout, the elapsed span is a lower bound.
pub(crate) struct Observation {
    /// Time from acknowledgment until a matching mark was observed or timed out.
    pub(crate) elapsed: Duration,
    /// A matching mark was not observed within the selected maximum.
    pub(crate) breached: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn placement_uses_ack_time_and_requires_the_matching_revision() {
        let observer = Probe(Connection::open_in_memory().unwrap());
        observer
            .0
            .execute_batch(
                "CREATE TABLE items(id TEXT PRIMARY KEY, revision TEXT, mat_state TEXT);
                 INSERT INTO items VALUES('item', 'current', 'materialized');",
            )
            .unwrap();
        let start = Instant::now() - Duration::from_secs(2);
        let budget = Duration::from_secs(1);
        let current = json!({"id": "item", "revision": "current"});
        let observed = observer.observe(&current, Instant::now(), budget).unwrap();
        assert!(start.elapsed() > budget);
        assert!(!observed.breached);

        let newer = json!({"id": "item", "revision": "newer"});
        let missed = observer.observe(&newer, start, budget).unwrap();
        assert!(missed.breached && missed.elapsed >= budget);
    }
}

/// One read-only observer for the selected space; never changes product state.
pub(crate) struct Probe(Connection);
impl Probe {
    /// Open one read-only SQLite connection; refuse absent/unreadable state.
    pub(crate) fn open(root: &Path, space: &str) -> Result<Self> {
        Ok(Self(Connection::open_with_flags(
            root.join("appdata/spaces").join(space).join("content.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?))
    }
    /// Verify placement for callers that do not record a separate measurement.
    pub(crate) fn wait(&self, note: &Value) -> Result<()> {
        if self
            .observe(note, Instant::now(), Duration::from_secs(1))?
            .breached
        {
            return Err("userdata materialization exceeded one second".into());
        }
        Ok(())
    }

    /// Observe the matching mark after the HTTP caller received its response.
    /// Each iteration does an indexed read. A missing mark at the deadline
    /// reports the observation time without discarding it.
    pub(crate) fn observe(
        &self,
        note: &Value,
        acknowledged: Instant,
        budget: Duration,
    ) -> Result<Observation> {
        loop {
            let (revision, state): (String, String) = self.0.query_row(
                "SELECT revision,mat_state FROM items WHERE id=?1",
                [note["id"].as_str().ok_or("missing item")?],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let elapsed = acknowledged.elapsed();
            let done = state == "materialized" && note["revision"] == revision;
            if done || elapsed >= budget {
                return Ok(Observation {
                    elapsed,
                    breached: elapsed > budget || !done,
                });
            }
            std::thread::sleep((budget - elapsed).min(Duration::from_millis(1)));
        }
    }
}
