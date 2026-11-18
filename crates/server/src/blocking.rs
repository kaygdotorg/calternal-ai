//! Foreground blocking work, classified by the existing operation declarations.
use std::{future::Future, sync::Arc};
use tokio::{runtime::Runtime, sync::Semaphore, task::JoinError};

/// Per-host mutation executor, separate from the listener's two read slots.
/// Six writer threads preserve the previous eight-slot total. At most 64 jobs
/// are running/queued; further callers await admission without blocking a thread.
/// This partitions scheduling only: per-space locks and the auth publication
/// barrier retain their ordering. Drop lets already started work finish.
#[derive(Clone)]
pub struct RequestWorkers(Arc<Workers>);
struct Workers {
    runtime: Option<Runtime>,
    admitted: Arc<Semaphore>,
}
impl Drop for Workers {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
tokio::task_local! { static MUTATION: Option<RequestWorkers>; }
impl RequestWorkers {
    /// Create a lazy six-thread blocking executor; no thread starts until work
    /// arrives. Returns an OS/runtime error if the driver cannot be created.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self(Arc::new(Workers {
            runtime: Some(
                tokio::runtime::Builder::new_current_thread()
                    .max_blocking_threads(6)
                    .thread_name("request-write")
                    .build()?,
            ),
            admitted: Arc::new(Semaphore::new(64)),
        })))
    }
    /// Run an operation with its declared mutation class. Read scopes clear any
    /// outer writer context. No I/O, grants or results are cached by the scope.
    pub(crate) async fn scope<F: Future>(&self, mutates: bool, future: F) -> F::Output {
        MUTATION.scope(mutates.then(|| self.clone()), future).await
    }
}
/// Await one blocking callback. Outside an operation scope this uses the caller's
/// runtime; foreground mutation scopes use the bounded writer executor. Panics
/// propagate as JoinError exactly as with Tokio's blocking adapter.
pub(crate) async fn spawn<F, R>(f: F) -> Result<R, JoinError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let selected = MUTATION.try_with(Clone::clone).ok().flatten();
    if let Some(workers) = selected {
        let permit = workers
            .0
            .admitted
            .clone()
            .acquire_owned()
            .await
            .expect("writer admission is never closed");
        let caller = tokio::runtime::Handle::current();
        workers
            .0
            .runtime
            .as_ref()
            .expect("writer runtime")
            .spawn_blocking(move || {
                let _permit = permit;
                // Existing synchronous adapters may enter the listener for async
                // services. Keep that context; only this callback uses writer slots.
                let _context = caller.enter();
                f()
            })
            .await
    } else {
        tokio::task::spawn_blocking(f).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readers_run_while_writer_slots_are_occupied() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let workers = RequestWorkers::new().unwrap();
        runtime.block_on(async {
            let mut releases = Vec::new();
            let mut jobs = Vec::new();
            for _ in 0..6 {
                let (entered, ready) = tokio::sync::oneshot::channel();
                let (release, wait) = std::sync::mpsc::channel();
                releases.push(release);
                let pool = workers.clone();
                jobs.push(tokio::spawn(async move {
                    pool.scope(
                        true,
                        spawn(move || {
                            entered.send(()).unwrap();
                            wait.recv().unwrap();
                        }),
                    )
                    .await
                    .unwrap();
                }));
                ready.await.unwrap();
            }
            let read = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                workers.scope(false, spawn(|| 42)),
            )
            .await;
            for release in releases {
                release.send(()).unwrap();
            }
            for job in jobs {
                job.await.unwrap();
            }
            assert_eq!(
                read.ok().and_then(Result::ok),
                Some(42),
                "read queued behind writers"
            );
        });
    }
}
