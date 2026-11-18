//! Reuse per-space read-only handles. Each probe checks the current watermark.
use super::*;
use crate::journal::lock;
use std::{collections::VecDeque, sync::Mutex};

const RETAINED: usize = 128;

type Pair = (
    Connection,
    Option<Connection>,
    Option<calternal_store::ProjectionJournal>,
);

/// Idle connections only. The mutex covers ownership transfers and no I/O.
#[derive(Default)]
pub(crate) struct Readers(Mutex<VecDeque<(SpaceId, Pair)>>);

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

    pub(super) fn take(&self, space: SpaceId, appdata: &std::path::Path) -> Result<Pair> {
        let cached = {
            let mut idle = lock(&self.0);
            idle.iter()
                .position(|(id, _)| *id == space)
                .and_then(|i| idle.remove(i).map(|(_, pair)| pair))
        };
        let (source, index, journal) = match cached {
            Some(pair) => pair,
            None => {
                let source = crate::pool::open_reader(&crate::db::space_path(appdata, space))
                    .map_err(internal)?;
                source
                    .pragma_update(None, "cache_size", -256)
                    .map_err(internal)?;
                (source, None, None)
            }
        };
        let journal = match journal {
            Some(reader) => Some(reader),
            None => calternal_store::ProjectionJournal::open(appdata, space).map_err(internal)?,
        };
        if index.is_some() {
            return Ok((source, index, journal));
        }
        let path = worker::db_path(appdata, space);
        if !path.is_file() {
            return Ok((source, None, journal));
        }
        let open = match while_available(|| {
            let open = crate::pool::open_reader(&path)?;
            open.busy_timeout(std::time::Duration::from_millis(20))?;
            open.pragma_update(None, "cache_size", -256)?;
            Ok(open)
        })? {
            Some(open) => open,
            None => return Ok((source, None, journal)),
        };
        register_functions(&open)?;
        Ok((source, Some(open), journal))
    }

    pub(super) fn keep(&self, space: SpaceId, pair: Pair) {
        let mut idle = lock(&self.0);
        let evicted = if let Some(i) = idle.iter().position(|(id, _)| *id == space) {
            idle.remove(i)
        } else if idle.len() == RETAINED {
            idle.pop_front()
        } else {
            None
        };
        idle.push_back((space, pair));
        drop(idle);
        drop(evicted);
    }
}

/// Keep the newest four background writers warm. Ownership transfers hold
/// the mutex briefly, and eviction or space erasure closes outside the lock.
#[derive(Default)]
pub(crate) struct Writers(Mutex<VecDeque<(SpaceId, Connection)>>);
impl Writers {
    pub(super) fn take(&self, space: SpaceId) -> Option<Connection> {
        let mut idle = lock(&self.0);
        let position = idle.iter().position(|(id, _)| *id == space)?;
        idle.remove(position).map(|(_, connection)| connection)
    }

    pub(crate) fn forget(&self, space: SpaceId) {
        drop(self.take(space));
    }

    pub(super) fn keep(&self, space: SpaceId, connection: Connection) {
        let mut idle = lock(&self.0);
        let old = if idle.len() == 4 {
            idle.pop_front()
        } else {
            None
        };
        idle.push_back((space, connection));
        drop(idle);
        drop(old);
    }
}

/// The worker can hold a transient exclusive lock while opening its WAL.
pub(super) fn while_available<T>(read: impl FnOnce() -> rusqlite::Result<T>) -> Result<Option<T>> {
    match read() {
        Ok(item) => Ok(Some(item)),
        Err(error)
            if matches!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
            ) =>
        {
            Ok(None)
        }
        Err(error) => Err(internal(error)),
    }
}
