//! Reuse lightweight read-only SQLite handles without opening a space writer.
use super::*;
use crate::journal::lock;
use rusqlite::OptionalExtension;
use std::{collections::VecDeque, sync::Mutex};

const RETAINED: usize = 128;

/// Bounded idle read-only handles; the mutex covers ownership transfers only.
#[derive(Default)]
pub(crate) struct Readers(Mutex<VecDeque<(SpaceId, Connection)>>);

impl Readers {
    pub(crate) fn forget(&self, space: SpaceId) {
        let mut idle = lock(&self.0);
        let entry = idle
            .iter()
            .position(|(id, _)| *id == space)
            .and_then(|i| idle.remove(i));
        drop(idle);
        drop(entry);
    }

    pub(super) fn take(
        &self,
        space: SpaceId,
        appdata: &std::path::Path,
        config: &SearchConfig,
    ) -> Result<Reader> {
        {
            let mut idle = lock(&self.0);
            if let Some(i) = idle.iter().position(|(id, _)| *id == space) {
                return Ok(Reader::Ready(idle.remove(i).expect("position in deque").1));
            }
        }
        let path = crate::db::space_path(appdata, space);
        match std::fs::metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Reader::Empty),
            Err(error) => return Err(internal(error)),
            Ok(_) => {}
        }
        let conn = crate::pool::open_reader(&path).map_err(internal)?;
        let initialized: bool = conn
            .query_row(
                "SELECT count(*)=4 FROM sqlite_schema WHERE type='table'
             AND name IN ('search_configuration','search_docs','search_text','search_title_grams')",
                [],
                |r| r.get(0),
            )
            .map_err(internal)?;
        if !initialized {
            return Ok(Reader::Initialize);
        }
        let saved: Option<(String, String)> = conn
            .query_row(
                "SELECT kinds, (SELECT value FROM binding WHERE id='space')
                 FROM search_configuration WHERE id=1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(internal)?;
        if let Some((_, bound)) = &saved
            && bound != &space.to_string()
        {
            return Err(internal("wrong space binding"));
        }
        if saved.as_ref().map(|(kinds, _)| kinds) != Some(&config.kinds) {
            return Ok(Reader::Initialize);
        }
        conn.pragma_update(None, "cache_size", -256)
            .map_err(internal)?;
        register_functions(&conn)?;
        Ok(Reader::Ready(conn))
    }

    pub(super) fn keep(&self, space: SpaceId, conn: Connection) {
        // Drop a replaced connection after releasing the cache mutex.
        let mut idle = lock(&self.0);
        let evicted = if let Some(i) = idle.iter().position(|(id, _)| *id == space) {
            idle.remove(i)
        } else if idle.len() == RETAINED {
            idle.pop_front()
        } else {
            None
        };
        idle.push_back((space, conn));
        drop(idle);
        drop(evicted);
    }
}

pub(super) enum Reader {
    Empty,
    Initialize,
    Ready(Connection),
}
