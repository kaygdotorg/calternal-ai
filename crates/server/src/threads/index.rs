//! Read-only join of saved file metadata and the existing space-local jobs index.
use crate::{
    api::Shared,
    auth::Directory,
    plugins::{job_sql, jobs::JobDatabase},
};
use calternal_types::{AccountId, Error, Page, SpaceId};
use rusqlite::Connection;
use serde_json::Value;
use std::path::Path;
use uuid::Uuid;

/// Indexed thread summary with the saved turn's current display state.
pub(super) struct Row {
    /// Stable content item UUID.
    pub(super) id: Uuid,
    /// Indexed UTC timestamp used for keyset order.
    updated_at: String,
    /// File-derived thread metadata, excluding the body.
    pub(super) metadata: Value,
    /// Current content revision for refreshing the document.
    pub(super) revision: String,
    /// Derived sidebar label for the latest retained job.
    pub(super) turn_state: &'static str,
    /// Caller-scoped explanation from the same index seek.
    pub(super) turn_reason: Value,
}

fn connection<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
) -> Result<std::sync::Arc<JobDatabase>, Error> {
    let conn = state.jobs.conn(&space.to_string())?;
    let attached = {
        let locked = conn.lock().map_err(index_error)?;
        is_attached(&locked)
    };
    if !attached {
        // Initialize the content schema before attaching. Release the jobs lock
        // above first: writer paths acquire content before jobs.
        state.store.content().live_journal(space)?;
    }
    Ok(conn)
}

fn is_attached(conn: &Connection) -> bool {
    conn.db_name(2).is_ok_and(|name| name == "thread_content")
}

fn index_error(error: rusqlite::Error) -> Error {
    tracing::error!(%error, "thread index read failed");
    Error::internal("could not read the thread index")
}

// The unary plus removes the content column's TEXT affinity without changing its
// value. This lets SQLite seek the affinity-free JSON expression in jobs_thread.
const SUMMARIES: &str = "SELECT i.id, i.metadata, i.revision, i.updated_at, (
         SELECT json_array(j.state,j.account,__REASON__) FROM main.jobs j
         WHERE j.operation='agent.turn' AND json_extract(j.input,'$.request.thread_id') = +i.id
         AND json_extract(j.input,'$.check') IS NOT 1
         ORDER BY j.rowid DESC LIMIT 1
     )
     FROM thread_content.items i WHERE i.kind='threads'
     AND i.updated_at<=?1 AND (i.updated_at<?1 OR i.id>?2)
     AND (?3 IS NOT NULL OR json_extract(i.metadata,'$.deleted') IS NOT 1)
     AND (?3 IS NOT NULL OR ?4 OR json_extract(i.metadata,'$.archived') IS NOT 1)
     AND (?3 IS NULL OR json_extract(i.metadata,'$.parent_thread')=?3)
     ORDER BY i.updated_at DESC, i.id LIMIT ?5";

fn summaries() -> &'static str {
    static SQL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    SQL.get_or_init(|| SUMMARIES.replace("__REASON__", &crate::turns::reason::sql("j.")))
}

fn attach(conn: &Connection) -> Result<(), Error> {
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
    Ok(())
}

/// List the authorized space's threads with a single indexed query and no body
/// reads. Filters run before the limit, the last delivered key continues the
/// page, and a cold connection attaches the same-space index read-only.
pub(super) fn page<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    account: AccountId,
    parent: Option<&str>,
    archived: bool,
    cursor: Option<&str>,
    limit: usize,
) -> Result<Page<Row>, Error> {
    let position: Option<(String, String)> = cursor
        .map(serde_json::from_str)
        .transpose()
        .map_err(|_| Error::invalid_input("invalid page cursor"))?;
    // RFC 3339 timestamps sort before the first-page bound.
    let (updated_at, id) = position.unwrap_or_else(|| ("~".into(), String::new()));
    let conn = connection(state, space)?;
    let conn = conn.lock().map_err(index_error)?;
    attach(&conn)?;
    let mut stmt = conn.prepare_cached(summaries()).map_err(index_error)?;
    let rows = stmt
        .query_map(
            rusqlite::params![updated_at, id, parent, archived, (limit + 1) as i64],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .map_err(index_error)?;
    let account = account.to_string();
    let rows = rows
        .map(|r| {
            let (id, metadata, revision, updated_at, status) = r.map_err(index_error)?;
            let status: Value = status
                .as_deref()
                .map(serde_json::from_str)
                .transpose()
                .map_err(|_| Error::internal("invalid indexed turn state"))?
                .unwrap_or_default();
            let state = status[0].as_str().map(job_sql::parse_state);
            let reason = crate::turns::reason::describe(state, &status[2], status[1] == account);
            Ok(Row {
                id: Uuid::parse_str(&id)
                    .map_err(|_| Error::internal("invalid indexed thread ID"))?,
                metadata: serde_json::from_str(&metadata)
                    .map_err(|_| Error::internal("invalid indexed thread metadata"))?,
                revision,
                updated_at,
                turn_state: state.map_or("idle", |s| super::turn_status::label(s)),
                turn_reason: reason,
            })
        })
        .collect::<Result<_, Error>>()?;
    Ok(Page::from_lookahead(rows, limit, |row| {
        serde_json::json!([row.updated_at, row.id]).to_string()
    }))
}

/// Read existence, the caller's current pin, and activation controls together.
/// One statement over the indexed thread, latest job and fixed control prefix.
/// No content bytes, journal reads or account-private details leave the query.
pub(crate) fn readiness_context<D: Directory>(
    state: &Shared<D>,
    space: SpaceId,
    thread: Option<Uuid>,
    account: calternal_types::AccountId,
) -> Result<(Option<Uuid>, Value), Error> {
    let conn = connection(state, space)?;
    let conn = conn.lock().map_err(index_error)?;
    attach(&conn)?;
    let mut stmt = conn.prepare_cached(
        "SELECT i.id, (
            SELECT CASE WHEN j.account=?2 AND j.state IN ('waiting','admitted','placing','running','interrupted','retry_waiting')
                THEN json_extract(j.checkpoint,'$.request_machine') END
            FROM main.jobs j WHERE j.operation='agent.turn'
                AND json_extract(j.input,'$.request.thread_id')=+i.id
                AND json_extract(j.input,'$.check') IS NOT 1
            ORDER BY j.rowid DESC LIMIT 1
        ), (
            SELECT json_group_object(key,json(value)) FROM thread_content.controls
            WHERE key >= ?3 AND key < ?4
        )
        FROM (SELECT 1)
        LEFT JOIN thread_content.items i ON i.id=?1 AND i.kind='threads'
          AND json_extract(i.metadata,'$.deleted') IS NOT 1",
    ).map_err(index_error)?;
    let start = crate::store::plugin_key("");
    let end = format!("{}0", start.trim_end_matches('/'));
    let (found, pin, controls): (Option<String>, Option<String>, String) = stmt
        .query_row(
            rusqlite::params![
                thread.map(|id| id.to_string()),
                account.to_string(),
                start,
                end
            ],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(index_error)?;
    if thread.is_some() && found.is_none() {
        return Err(Error::not_found("thread not found"));
    }
    let pin = pin.and_then(|id| Uuid::parse_str(&id).ok());
    let controls = serde_json::from_str(&controls)
        .map_err(|_| Error::internal("invalid indexed plugin state"))?;
    Ok((pin, controls))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::Jobs;

    #[test]
    fn list_seeks_the_jobs_thread_index_for_each_summary() {
        let dir = tempfile::tempdir().unwrap();
        let jobs = Jobs::open(dir.path()).unwrap();
        let space = SpaceId::new().to_string();
        let conn = jobs.conn(&space).unwrap();
        let conn = conn.lock().unwrap();
        conn.execute_batch(
            "ATTACH DATABASE ':memory:' AS thread_content;
             CREATE TABLE thread_content.items(
                id TEXT PRIMARY KEY, kind TEXT NOT NULL, metadata TEXT NOT NULL,
                revision TEXT NOT NULL, updated_at TEXT NOT NULL
             ) STRICT;
             CREATE INDEX thread_content.item_list ON items(kind,updated_at DESC,id);",
        )
        .unwrap();

        let mut stmt = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {}", summaries()))
            .unwrap();
        let nodes: Vec<String> = stmt
            .query_map(
                rusqlite::params!["~", "", Option::<&str>::None, false, 101],
                |row| row.get(3),
            )
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            nodes
                .iter()
                .any(|row| row.contains("SEARCH i USING INDEX item_list")
                    && row.contains("updated_at<?")),
            "{nodes:?}"
        );
        assert!(
            nodes
                .iter()
                .any(|row| { row.contains("SEARCH j USING INDEX jobs_thread") }),
            "{nodes:?}"
        );
    }
}
