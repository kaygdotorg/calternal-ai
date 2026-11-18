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
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::sync::{broadcast, mpsc, oneshot};

/// How many commits may wait in line before callers are asked to slow down.
const QUEUE: usize = 64;

pub(crate) enum Cmd {
    Commit(Commit, oneshot::Sender<Result<Vec<JournalEntry>>>),
    /// Content holds its space slot through this pruning command.
    Prune {
        now_ms: i64,
        pinned: Vec<String>,
        reply: oneshot::Sender<Result<()>>,
    },
    /// The journal went back in time: start a new epoch (the epoch the
    /// caller saw, so two proofs rotate once).
    Rotate(String, oneshot::Sender<Result<()>>),
}

/// Starts the writer thread and returns its drained connection for shutdown.
/// It finishes queued commits before handing the connection to the flush
/// worker. Receipt pruning runs only when the content owner supplies its pins.
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
            while let Some(cmd) = rx.blocking_recv() {
                let head = &shared.head;
                match cmd {
                    Cmd::Prune { now_ms, pinned, reply } => {
                        let result = shared.step().and_then(|_step| prune(&conn, options, now_ms, pinned));
                        let _ = reply.send(result);
                    }
                    Cmd::Rotate(observed, reply) => {
                        let result = shared.step().and_then(|_step| restore::rotate(&mut conn, &shared, &observed, None));
                        let _ = reply.send(result);
                    }
                    Cmd::Commit(c, reply) => {
                        let _step = match shared.step() {
                            Ok(step) => step,
                            Err(error) => {
                                let _ = reply.send(Err(error));
                                continue;
                            }
                        };
                        let durable = c.durable;
                        let r = if durable {
                            conn.pragma_update(None, "synchronous", "FULL")
                                .map_err(Error::internal)
                                .and_then(|()| commit(&mut conn, &shared, c))
                        } else {
                            commit(&mut conn, &shared, c)
                        };
                        if durable
                            && let Err(e) = conn.pragma_update(None, "synchronous", "NORMAL")
                        {
                            tracing::warn!(%e, "journal retains stronger sync after a durable commit");
                        }
                        if let Ok(committed) = &r
                            && let Some(last) = committed.entries.last()
                        {
                            head.store(last.seq, Ordering::Relaxed);
                            // Sent from here, in commit order, so listeners
                            // never see entries out of order. Having no
                            // listeners is fine.
                            for e in &committed.entries {
                                let _ = live.send(Arc::new(e.clone()));
                            }
                            #[cfg(any(feature = "perf-timings", all(feature = "test-timings", debug_assertions)))]
                            if let Some(at_ns) = committed.at_ns {
                                for entry in &committed.entries {
                                    if let Some(request) = &entry.request_id {
                                        crate::timings::emit_commit(
                                            request,
                                            at_ns,
                                            #[cfg(feature = "perf-timings")]
                                            committed.commit_ns,
                                        );
                                    }
                                }
                            }
                        }
                        let _ = reply.send(r.map(|committed| committed.entries));
                    }
                }
            }
            conn
        })
        .map_err(Error::internal)?;
    Ok((tx, thread))
}

/// Delete expired receipts except those still needed to replay a content intent.
fn prune(conn: &Connection, options: Options, at_ms: i64, pinned: Vec<String>) -> Result<()> {
    let retention = i64::try_from(options.idempotency_retention.as_millis()).unwrap_or(i64::MAX);
    let cutoff = at_ms.saturating_sub(retention);
    let pins = serde_json::to_string(&pinned).map_err(Error::internal)?;
    conn.execute(
        "DELETE FROM idempotency WHERE at_ms < ?1 AND key NOT IN (SELECT value FROM json_each(?2))",
        params![cutoff, pins],
    )
    .map_err(Error::internal)?;
    Ok(())
}

struct Committed {
    entries: Vec<JournalEntry>,
    #[cfg(any(
        feature = "perf-timings",
        all(feature = "test-timings", debug_assertions)
    ))]
    at_ns: Option<u64>,
    #[cfg(feature = "perf-timings")]
    commit_ns: u64,
}

/// Commits one batch, then retains only its newest bounded set of signed tokens.
fn commit(conn: &mut Connection, shared: &Shared, c: Commit) -> Result<Committed> {
    let at_ms = now_ms();
    let epoch = shared.epoch();
    let tx = conn.transaction().map_err(Error::internal)?;
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
    #[cfg(feature = "perf-timings")]
    let commit_start = c.durable.then(calternal_types::perf_monotonic_ns);
    tx.commit().map_err(Error::internal)?;
    #[cfg(any(
        feature = "perf-timings",
        all(feature = "test-timings", debug_assertions)
    ))]
    let at_ns = c.durable.then(calternal_types::perf_monotonic_ns);
    if shared.guard.is_some() {
        for entry in entries.iter().rev().take(MAX_ISSUED_TOKENS) {
            shared.remember_issued(&epoch, entry.seq, &entry.token);
        }
    }
    if let (Some(e), Some(_)) = (entries.last(), last_check) {
        shared.remember_proof(&epoch, &e.token, e.seq);
    }
    Ok(Committed {
        entries,
        #[cfg(any(
            feature = "perf-timings",
            all(feature = "test-timings", debug_assertions)
        ))]
        at_ns,
        #[cfg(feature = "perf-timings")]
        commit_ns: at_ns
            .zip(commit_start)
            .map_or(0, |(end, start)| end - start),
    })
}
