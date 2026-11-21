//! Read-only projection diagnostics for the private operator socket.

use super::{replay, runtime};
use crate::{Content, Result, SpaceId, types::internal};
use serde::{Deserialize, Serialize};

/// Operator diagnostics for one space's rebuildable window projection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WindowProjectionStatus {
    /// Recorded over-limit items. Absent while the sidecar cannot be read.
    pub skipped_items: Option<u64>,
    /// Persistent failure category, when a worker or reader has failed.
    pub failure: Option<String>,
    /// Whether automatic retries stopped after reaching the attempt limit.
    pub retries_exhausted: bool,
}

impl Content {
    /// Inspect the persistent reason and count recorded omissions under the
    /// space lifecycle guard. Uses bounded marker I/O and one read-only count
    /// proportional to the skipped items. Opens no writer and does no replay.
    pub fn window_projection_status(&self, space: SpaceId) -> Result<WindowProjectionStatus> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        let source = crate::db::space_path(&self.roots.appdata, space);
        let failure = runtime::read_failure(&source)?;
        let path = replay::path(&source);
        let slot = self.pin_slot(space)?;
        let skipped_items = match slot.windows.gate.try_read() {
            Ok(_read) if path.try_exists().map_err(internal)? => {
                replay::open_sidecar_reader(&path).ok().and_then(|db| {
                    db.query_row("SELECT count(*) FROM window_skipped", [], |row| {
                        row.get::<_, i64>(0)
                    })
                    .ok()
                    .and_then(|n| n.try_into().ok())
                })
            }
            Ok(_) => Some(0),
            Err(_) => None,
        };
        Ok(WindowProjectionStatus {
            skipped_items,
            failure: failure.map(|failure| failure.reason.into()),
            retries_exhausted: failure.is_some_and(runtime::Failure::terminal),
        })
    }
}
