//! Direct calls into each space's journal store ([`calternal_store::SpaceStore`])
//! from the caller's own blocking thread.
//!
//! The store's API is async. Rather than funnel every space through one bridge
//! thread, each call runs its future to completion on the calling thread while a
//! small private runtime (two workers) drives the store's timers, blocking reads
//! and witness flusher. Calls for different spaces therefore run concurrently;
//! calls for one space are already serialized by that space's lock in
//! [`crate::Content`].
use crate::types::internal;
use crate::{Error, Receipt, Result};
use calternal_store::{Commit, IdemWrite, JournalEntry, Options, SpaceStore, StampedEvent};
use calternal_types::{AccountId, SpaceId};
use serde_json::Value;
use std::{
    future::Future,
    path::PathBuf,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use uuid::Uuid;

/// The journal side of [`crate::Content`]: roots, store options and the runtime
/// that drives store background work.
pub(crate) struct Journals {
    appdata: PathBuf,
    secrets: PathBuf,
    options: Options,
    runtime: Option<tokio::runtime::Runtime>,
}

impl Journals {
    /// Starts the two-worker driver runtime. Cost: two threads; no I/O.
    pub(crate) fn open(appdata: PathBuf, secrets: PathBuf) -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("content-journal")
            .enable_all()
            .build()
            .map_err(internal)?;
        Ok(Self {
            appdata,
            secrets,
            // Request replay stays permanent, as before; bounding it is an open owner
            // question (review finding 11, checklist item 7).
            options: Options {
                idempotency_retention: std::time::Duration::from_millis(i64::MAX as u64),
                ..Options::default()
            },
            runtime: Some(runtime),
        })
    }

    /// Runs one store future on the calling thread. Must not be called from inside
    /// an async task: callers are blocking host threads (or startup).
    fn run<F: Future>(&self, future: F) -> F::Output {
        let _context = self.runtime.as_ref().expect("journal runtime").enter();
        block_on(future)
    }

    /// Opens one space's journal: private directories, then [`SpaceStore::open_with`].
    /// Cost: directory preparation and the store's open (SQLite, key and witness reads).
    pub(crate) fn open_store(&self, space: SpaceId) -> Result<SpaceStore> {
        let dir = self.appdata.join("spaces").join(space.to_string());
        calternal_fsio::private::directory(&dir).map_err(internal)?;
        calternal_fsio::private::file(&dir.join("index.db")).map_err(internal)?;
        calternal_fsio::private::directory(&self.secrets.join("spaces").join(space.to_string()))
            .map_err(internal)?;
        self.run(SpaceStore::open_with(
            &self.appdata,
            space,
            &self.secrets,
            self.options,
        ))
        .map_err(internal)
    }

    /// The account's receipt for a request in this space. Cost: one indexed read on
    /// a short-lived read connection; no writer queue.
    pub(crate) fn receipt(
        &self,
        store: &SpaceStore,
        account: AccountId,
        id: &str,
    ) -> Result<Option<Receipt>> {
        self.run(store.idempotency(account, id))
            .map_err(internal)?
            .map(|r| {
                let item = r
                    .result
                    .get("content_item")
                    .and_then(Value::as_str)
                    .map(Uuid::parse_str)
                    .transpose()
                    .map_err(internal)?;
                Ok(Receipt {
                    operation: r.action,
                    fingerprint: r.payload_hash,
                    result: r.result["payload"].clone(),
                    item,
                })
            })
            .transpose()
    }

    /// Records the receipt and journal event of one mutation, idempotently, then
    /// makes it durable. A different operation or input under the same request ID is
    /// a conflict. Cost: one indexed read, one NORMAL writer transaction and one FULL
    /// writer transaction (the durability barrier) on the space's writer thread.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish(
        &self,
        store: &SpaceStore,
        account: AccountId,
        id: &str,
        operation: &str,
        fingerprint: &str,
        item: Option<Uuid>,
        revision: &str,
        result: Value,
    ) -> Result<()> {
        self.run(async {
            let prior = store.idempotency(account, id).await.map_err(internal)?;
            if let Some(prior) = prior {
                if prior.action != operation || prior.payload_hash != fingerprint {
                    return Err(Error::request_reused());
                }
            } else {
                store
                    .commit(Commit {
                        events: vec![StampedEvent {
                            author: account,
                            kind: operation.to_owned(),
                            item: item.map(|id| id.to_string()).unwrap_or_default(),
                            data: serde_json::json!({ "revision": revision }),
                            installation: None,
                            request_id: Some(id.to_owned()),
                        }],
                        idempotency: Some(IdemWrite {
                            account,
                            key: id.to_owned(),
                            action: operation.to_owned(),
                            payload_hash: fingerprint.to_owned(),
                            result: serde_json::json!({ "payload": result, "content_item": item }),
                        }),
                    })
                    .await
                    .map_err(internal)?;
            }
            // A FULL-synchronous publication makes the preceding NORMAL receipt and
            // event durable before the caller retires its outbox row.
            store
                .commit_once(
                    account,
                    format!("content/{id}/{revision}"),
                    fingerprint.to_owned(),
                    vec![],
                )
                .await
                .map_err(internal)?;
            Ok(())
        })
    }

    /// The first 10,000 journal entries. Cost: one read connection and page query.
    pub(crate) fn entries(&self, store: &SpaceStore) -> Result<Vec<JournalEntry>> {
        self.run(store.since(0, 10_000)).map_err(internal)
    }
}

impl Drop for Journals {
    /// Stops the driver without blocking, so dropping content inside an async task
    /// is allowed. Stores are dropped first by [`crate::Content`].
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

/// Polls `future` on this thread, parking between wake-ups. Cost: one Arc per call.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
        std::thread::park();
    }
}

/// Locks a mutex, recovering from poisoning (state is validated by its users).
pub(crate) fn lock<T>(value: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|e| e.into_inner())
}
