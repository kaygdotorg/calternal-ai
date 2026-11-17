//! Read-only join of saved file metadata and the existing space-local jobs index.
use crate::{
    api::Shared,
    auth::Directory,
    plugins::{job_sql, jobs::JobDatabase},
};
use calternal_types::{Error, SpaceId};
use rusqlite::Connection;
use serde_json::Value;
use std::path::Path;
use uuid::Uuid;

/// Indexed thread summary with the saved turn's current display state.
pub(super) struct Row {
    /// Stable content item UUID.
    pub(super) id: Uuid,
    /// File-derived thread metadata, excluding the body.
    pub(super) metadata: Value,
    /// Current content revision for refreshing the document.
    pub(super) revision: String,
    /// Derived sidebar label for the latest retained job.
    pub(super) turn_state: &'static str,
}

/// Fetch the existing bounded content index page and current job states in a
/// single SQL statement. No message bytes, stored job payloads or file stats.
/// Only cold attachment prepares the same-space content database and attaches
/// its read-only handle to the cached jobs connection. Neither store writes here.
pub(super) fn page<D: Directory>(state: &Shared<D>, space: SpaceId) -> Result<Vec<Row>, Error> {
    let conn = state.jobs.conn(&space.to_string())?;
    if !is_attached(&conn.lock().unwrap_or_else(|p| p.into_inner())) {
        // Initialize the content schema before attaching. Release the jobs lock
        // above first: writer paths acquire content before jobs.
        state.store.content().live_journal(space)?;
    }
    read(&conn)
}

fn is_attached(conn: &Connection) -> bool {
    conn.db_name(2).is_ok_and(|name| name == "thread_content")
}

fn index_error(error: rusqlite::Error) -> Error {
    tracing::error!(%error, "thread index read failed");
    Error::internal("could not read the thread index")
}

fn read(conn: &JobDatabase) -> Result<Vec<Row>, Error> {
    let conn = conn.lock().unwrap_or_else(|p| p.into_inner());
    if !is_attached(&conn) {
        let database = conn
            .path()
            .ok_or_else(|| Error::internal("jobs database has no path"))?;
        let path = Path::new(database).with_file_name("content.db");
        let mut uri = String::from("file:");
        // The installation path may contain URI delimiters such as # and ?.
        for &byte in path.as_os_str().as_encoded_bytes() {
            if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
                uri.push(char::from(byte));
            } else {
                use std::fmt::Write;
                write!(uri, "%{byte:02X}").expect("formatting into String");
            }
        }
        uri.push_str("?mode=ro");
        conn.execute("ATTACH DATABASE ?1 AS thread_content", [&uri])
            .map_err(index_error)?;
    }
    let mut stmt = conn
        .prepare_cached(
            "SELECT i.id, i.metadata, i.revision, (
             SELECT j.state FROM main.jobs j
             WHERE j.operation='agent.turn' AND json_extract(j.input,'$.request.thread_id')=i.id
             ORDER BY j.rowid DESC LIMIT 1
         )
         FROM thread_content.items i WHERE i.kind='threads'
         ORDER BY i.updated_at DESC, i.id LIMIT 1000",
        )
        .map_err(index_error)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(index_error)?;
    rows.map(|r| {
        let (id, metadata, revision, state) = r.map_err(index_error)?;
        Ok(Row {
            id: Uuid::parse_str(&id).map_err(|_| Error::internal("invalid indexed thread ID"))?,
            metadata: serde_json::from_str(&metadata)
                .map_err(|_| Error::internal("invalid indexed thread metadata"))?,
            revision,
            turn_state: state.as_deref().map_or("idle", |state| {
                super::turn_status::label(job_sql::parse_state(state))
            }),
        })
    })
    .collect()
}
