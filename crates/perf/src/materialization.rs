//! Read-only confirmation after file and directory durability, outside ack timing.
use crate::http::Result;
use rusqlite::Connection;
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(crate) struct Probe(Connection);
impl Probe {
    pub(crate) fn open(root: &Path, space: &str) -> Result<Self> {
        Ok(Self(Connection::open_with_flags(
            root.join("appdata/spaces").join(space).join("content.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?))
    }
    /// The bookkeeping mark follows both fsyncs. Poll only after ack, never inside
    /// its timed span. A one-second timeout is a failure, not a relaxed budget.
    pub(crate) fn wait(&self, note: &Value) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let (revision, state): (String, String) = self.0.query_row(
                "SELECT revision,mat_state FROM items WHERE id=?1",
                [note["id"].as_str().ok_or("missing item")?],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if state == "materialized" && note["revision"] == revision {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("userdata materialization exceeded one second".into());
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
