//! Synchronous bridge to the inherited asynchronous per-space journal pool.
//! A dedicated runtime keeps synchronous host callers independent from their executor.
use crate::types::internal;
use crate::{Error, Receipt, Result};
use calternal_types::{AccountId, SpaceId};
use calternal_store::{
    Commit, IdemWrite, JournalEntry, Options, SpaceStore, SpaceStores,
    StampedEvent,
};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
use uuid::Uuid;

type Job = Box<dyn FnOnce(&tokio::runtime::Runtime, &SpaceStores) + Send>;
pub(crate) struct Journals {
    appdata: PathBuf,
    secrets: PathBuf,
    sender: Option<mpsc::SyncSender<Job>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Journals {
    pub(crate) fn open(
        appdata: PathBuf,
        secrets: PathBuf,
        sweep: impl Fn() + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(32);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(internal)?;
        let journal_appdata = appdata.clone();
        let journal_secrets = secrets.clone();
        let thread = thread::Builder::new()
            .name("content-journals".into())
            .spawn(move || {
                // Keep preview request replay permanent, matching the previous store contract.
                let options = Options {
                    idempotency_retention: Duration::from_millis(i64::MAX as u64),
                    ..Options::default()
                };
                let pool =
                    SpaceStores::new(journal_appdata, journal_secrets, options, Duration::ZERO);
                loop {
                    match receiver.recv_timeout(Duration::from_secs(30)) {
                        Ok(job) => job(&runtime, &pool),
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    sweep();
                    pool.sweep();
                }
                // Pool shutdown drains writers before its runtime is dropped.
                drop(pool);
            })
            .map_err(internal)?;
        Ok(Self {
            appdata,
            secrets,
            sender: Some(sender),
            thread: Some(thread),
        })
    }
    fn prepare(&self, space: SpaceId) -> Result<()> {
        let dir = self.appdata.join("spaces").join(space.to_string());
        calternal_fsio::private::directory(&dir).map_err(internal)?;
        calternal_fsio::private::file(&dir.join("index.db")).map_err(internal)?;
        calternal_fsio::private::directory(&self.secrets.join("spaces").join(space.to_string()))
            .map_err(internal)?;
        Ok(())
    }
    pub(crate) fn handle(&self, space: SpaceId) -> Result<SpaceStore> {
        self.prepare(space)?;
        self.run(move |rt, pool| {
            rt.block_on(async { pool.get(space).await.map_err(internal) })
        })
    }
    fn run<T: Send + 'static>(
        &self,
        f: impl FnOnce(&tokio::runtime::Runtime, &SpaceStores) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .as_ref()
            .ok_or_else(|| internal("stopped"))?
            .send(Box::new(move |rt, pool| {
                let _ = sender.send(f(rt, pool));
            }))
            .map_err(internal)?;
        receiver.recv().map_err(internal)?
    }
    pub(crate) fn receipt(
        &self,
        space: SpaceId,
        account: AccountId,
        id: &str,
    ) -> Result<Option<Receipt>> {
        let id = id.to_owned();
        self.run(move |rt, pool| {
            rt.block_on(async {
                let store = pool.get(space).await.map_err(internal)?;
                store
                    .idempotency(account, &id)
                    .await
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
            })
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn publish(
        &self,
        space: SpaceId,
        account: AccountId,
        id: String,
        operation: String,
        fingerprint: String,
        item: Option<Uuid>,
        revision: String,
        result: Value,
    ) -> Result<()> {
        self.run(move |rt, pool| {
            rt.block_on(async {
                let store = pool.get(space).await.map_err(internal)?;
                let prior = store
                    .idempotency(account, &id)
                    .await
                    .map_err(internal)?;
                if let Some(prior) = prior {
                    if prior.action != operation || prior.payload_hash != fingerprint {
                        return Err(Error::request_reused());
                    }
                } else {
                    store
                        .commit(Commit {
                            events: vec![StampedEvent {
                                author: account,
                                kind: operation.clone(),
                                item: item.map(|id| id.to_string()).unwrap_or_default(),
                                data: serde_json::json!({"revision":revision}),
                                installation: None,
                                request_id: Some(id.clone()),
                            }],
                            idempotency: Some(IdemWrite {
                                account,
                                key: id.clone(),
                                action: operation,
                                payload_hash: fingerprint.clone(),
                                result: serde_json::json!({"payload":result,"content_item":item}),
                            }),
                        })
                        .await
                        .map_err(internal)?;
                }
                // A new FULL-synchronous permanent publication transaction makes the
                // preceding receipt/event WAL durable before the outbox can retire.
                store
                    .commit_once(
                        account,
                        format!("content/{id}/{revision}"),
                        fingerprint,
                        vec![],
                    )
                    .await
                    .map_err(internal)?;
                Ok(())
            })
        })
    }
    pub(crate) fn entries(&self, space: SpaceId) -> Result<Vec<JournalEntry>> {
        self.run(move |rt, pool| {
            rt.block_on(async {
                pool.get(space)
                    .await
                    .map_err(internal)?
                    .since(0, 10_000)
                    .await
                    .map_err(internal)
            })
        })
    }
}
impl Drop for Journals {
    fn drop(&mut self) {
        drop(self.sender.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|e| e.into_inner())
}
pub(crate) type Shared<T> = Arc<Mutex<T>>;
