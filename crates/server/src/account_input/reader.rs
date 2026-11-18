//! A live account feed retains its existing SQLite connection and admission lease.
//! This prevents last-connection checkpoint/delete/reopen on each committed save.
use super::{Inputs, internal};
use crate::{
    api::Shared,
    auth::{Directory, Identity},
};
use calternal_types::Error;
use rusqlite::Connection;
use std::fs::File;

/// One connection per admitted stream, with a 64 KiB SQLite page-cache setting.
/// The existing listener ceiling bounds active and closing connections together.
/// No transaction, SQLite statement or mutex is retained across an async wait.
pub(super) struct Reader<D: Directory> {
    connection: Option<Connection>,
    admission: Option<File>,
    state: Shared<D>,
}
impl<D: Directory> Reader<D> {
    /// Attach the validated account connection on a blocking worker. Refuses a
    /// failed cache configuration; costs one PRAGMA and no durability barrier.
    pub(super) fn new(
        connection: Connection,
        admission: File,
        state: Shared<D>,
    ) -> Result<Self, Error> {
        connection
            .pragma_update(None, "cache_size", -64i64)
            .map_err(internal)?;
        Ok(Self {
            connection: Some(connection),
            admission: Some(admission),
            state,
        })
    }
    /// Replay at most 64 content-free bookmarks on a blocking worker. Mutations
    /// already clean before commit; authority wakes and the absolute one-second
    /// maintenance deadline recheck visibility/age here. Actual content fetches
    /// always perform current authorization. Failure terminates the feed.
    pub(super) fn page(
        &mut self,
        identity: &Identity,
        after: &str,
        maintenance: bool,
    ) -> Result<Option<Vec<String>>, Error> {
        let _step = self.state.inputs.step(identity.id)?;
        let connection = self.connection.as_mut().expect("live account connection");
        if maintenance {
            self.state
                .inputs
                .clean(connection, &self.state.directory, identity)?;
        }
        Inputs::page(connection, after)
    }
}
impl<D: Directory> Drop for Reader<D> {
    /// SQLite close can checkpoint and fsync. Reuse the host blocking pool, keeping
    /// admission occupied and installation root guards alive until close finishes.
    /// Pending close tasks are bounded by the same live-listener ceiling.
    fn drop(&mut self) {
        if let (Some(connection), Some(admission)) = (self.connection.take(), self.admission.take())
        {
            let state = self.state.clone();
            let close = move || {
                drop(connection);
                drop(admission);
                drop(state);
            };
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(close);
            } else {
                // Outside the host runtime this is already a synchronous caller.
                close();
            }
        }
    }
}
