//! The single writer for one space's `index.db`.
//!
//! It runs on its own thread and owns the only write connection. Commits
//! arrive over a channel and run one at a time, each as one short
//! transaction. Callers never hold a lock while the disk works: they wait
//! on a reply instead, and the queue keeps everyone in fair arrival order.

use crate::proof::MAX_ISSUED_TOKENS;
use crate::shared::Shared;
use crate::{Commit, Options, now_ms, restore};
use crate::{Error, ErrorCode, JournalEntry, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, oneshot};

/// How many commits may wait in line before callers are asked to slow down.
const QUEUE: usize = 64;

pub(crate) enum Cmd {
    Commit(Commit, oneshot::Sender<Result<Vec<JournalEntry>>>),
    Once(
        String,
        String,
        Vec<crate::StampedEvent>,
        oneshot::Sender<Result<crate::SyncToken>>,
    ),
    /// The journal went back in time: start a new epoch (the epoch the
    /// caller saw, so two proofs rotate once).
    Rotate(String, oneshot::Sender<Result<()>>),
}

/// Starts the writer thread and returns its drained connection for shutdown.
/// It finishes queued commits before handing the connection to the flush
/// worker. Old idempotency records are swept at start and after commits.
pub(crate) fn spawn(
    conn: Connection,
    shared: Arc<Shared>,
    live: broadcast::Sender<Arc<JournalEntry>>,
    path: &Path,
    options: Options,
) -> Result<(mpsc::Sender<Cmd>, std::thread::JoinHandle<Connection>)> {
    let (tx, mut rx) = mpsc::channel::<Cmd>(QUEUE);
    let name = format!(
        "store-{}",
        path.parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().chars().take(8).collect::<String>())
            .unwrap_or_default()
    );
    let thread = std::thread::Builder::new()
        .name(name)
        .spawn(move || {
            let mut conn = conn;
            prune(&conn, options);
            let mut last_prune = Instant::now();
            while let Some(cmd) = rx.blocking_recv() {
                let head = &shared.head;
                match cmd {
                    Cmd::Rotate(observed, reply) => {
                        let _ = reply.send(restore::rotate(&mut conn, &shared, &observed, None));
                    }
                    Cmd::Once(key, digest, events, reply) => {
                        let result = conn
                            .pragma_update(None, "synchronous", "FULL")
                            .map_err(Error::internal)
                            .and_then(|()| {
                                commit_inner(
                                    &mut conn,
                                    &shared,
                                    Commit {
                                        events,
                                        idempotency: None,
                                    },
                                    Some((key, digest)),
                                )
                            });
                        if let Err(e) = conn.pragma_update(None, "synchronous", "NORMAL") {
                            tracing::warn!(%e, "journal retains stronger sync after publication");
                        }
                        let result = result.map(|(entries, seq)| {
                            if let Some(last) = entries.last() {
                                head.store(last.seq, Ordering::Relaxed);
                                for e in entries {
                                    let _ = live.send(Arc::new(e));
                                }
                            }
                            crate::SyncToken {
                                epoch: shared.epoch().to_string(),
                                seq,
                            }
                        });
                        let _ = reply.send(result);
                    }
                    Cmd::Commit(c, reply) => {
                        let r = commit(&mut conn, &shared, c);
                        if let Ok(entries) = &r
                            && let Some(last) = entries.last()
                        {
                            head.store(last.seq, Ordering::Relaxed);
                            // Sent from here, in commit order, so listeners
                            // never see entries out of order. Having no
                            // listeners is fine.
                            for e in entries {
                                let _ = live.send(Arc::new(e.clone()));
                            }
                        }
                        let _ = reply.send(r);
                        if last_prune.elapsed() >= options.prune_every {
                            prune(&conn, options);
                            last_prune = Instant::now();
                        }
                    }
                }
            }
            conn
        })
        .map_err(Error::internal)?;
    Ok((tx, thread))
}

/// Deletes idempotency records older than the retention window. A failure
/// only means they stay a little longer, so it is logged and ignored.
fn prune(conn: &Connection, options: Options) {
    let cutoff = now_ms().saturating_sub(options.idempotency_retention.as_millis() as i64);
    match conn.execute("DELETE FROM idempotency WHERE at_ms < ?1", [cutoff]) {
        Ok(0) => {}
        Ok(n) => tracing::debug!(removed = n, "pruned old idempotency records"),
        Err(e) => tracing::warn!(%e, "could not prune idempotency records"),
    }
}

fn commit(conn: &mut Connection, shared: &Shared, c: Commit) -> Result<Vec<JournalEntry>> {
    commit_inner(conn, shared, c, None).map(|(entries, _)| entries)
}

/// Commits one batch, then retains only its newest bounded set of signed tokens.
fn commit_inner(
    conn: &mut Connection,
    shared: &Shared,
    c: Commit,
    once: Option<(String, String)>,
) -> Result<(Vec<JournalEntry>, i64)> {
    let at_ms = now_ms();
    let epoch = shared.epoch();
    let tx = conn.transaction().map_err(Error::internal)?;
    if let Some((key, digest)) = &once {
        let prior: Option<(String, i64)> = tx
            .query_row(
                "SELECT digest, seq FROM publication_receipts WHERE key = ?1",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(Error::internal)?;
        if let Some((previous, seq)) = prior {
            if previous != *digest {
                return Err(Error::new(
                    ErrorCode::IdempotencyConflict,
                    "publication key changed",
                ));
            }
            return Ok((vec![], seq));
        }
    }
    let mut entries = Vec::with_capacity(c.events.len());
    let mut last_check = None;
    {
        let mut insert = tx
            .prepare_cached(
                "INSERT INTO journal (at_ms, kind, item, data, installation, request_id, author)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )
            .map_err(Error::internal)?;
        for e in c.events {
            let data = serde_json::to_string(&e.data).map_err(Error::internal)?;
            insert
                .execute(params![
                    at_ms,
                    e.kind,
                    e.item,
                    data,
                    e.installation,
                    e.request_id,
                    e.author.to_string()
                ])
                .map_err(Error::internal)?;
            let seq = tx.last_insert_rowid();
            let (token, check) = shared.mint_with_check(
                &epoch,
                seq,
                at_ms,
                &e.kind,
                &e.item,
                e.request_id.as_deref(),
            );
            last_check = check;
            entries.push(JournalEntry {
                author: e.author,
                seq,
                token,
                at_ms,
                kind: e.kind,
                item: e.item,
                data: e.data,
                installation: e.installation,
                request_id: e.request_id,
            });
        }
    }
    let seq = match entries.last() {
        Some(e) => e.seq,
        None => crate::schema::read_head(&tx, false)?.0,
    };
    if let Some(idem) = c.idempotency {
        let result = serde_json::to_string(&idem.result).map_err(Error::internal)?;
        let inserted = tx
            .execute(
                "INSERT INTO idempotency (key, action, payload_hash, result, seq, at_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (key) DO NOTHING",
                params![idem.key, idem.action, idem.payload_hash, result, seq, at_ms],
            )
            .map_err(Error::internal)?;
        if inserted == 0 {
            // Someone already finished this key. Roll everything back so the
            // change is never recorded twice.
            return Err(Error::new(
                ErrorCode::IdempotencyConflict,
                format!("idempotency key {} already stored", idem.key),
            ));
        }
    }
    if let Some((key, digest)) = once {
        tx.execute(
            "INSERT INTO publication_receipts (key, digest, seq) VALUES (?1, ?2, ?3)",
            params![key, digest, seq],
        )
        .map_err(Error::internal)?;
    }
    tx.commit().map_err(Error::internal)?;
    if shared.guard.is_some() {
        for entry in entries.iter().rev().take(MAX_ISSUED_TOKENS) {
            shared.remember_issued(&epoch, entry.seq, &entry.token);
        }
    }
    if let (Some(e), Some(_)) = (entries.last(), last_check) {
        shared.remember_proof(&epoch, &e.token, e.seq);
    }
    Ok((entries, seq))
}
