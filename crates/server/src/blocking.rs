//! Foreground blocking work, classified by the existing operation declarations.
use std::{future::Future, sync::Arc};
use tokio::{runtime::Runtime, sync::Semaphore, task::JoinError};

/// Per-host foreground executors, isolated from unscoped background work.
/// Two reader and up to 32 writer threads each have a separate 64-job bound.
/// Further callers await admission without occupying a thread. This partitions
/// scheduling only: per-space locks and auth publication retain their ordering.
/// Both lanes retire threads after ten idle seconds; started callbacks finish.
#[derive(Clone)]
pub struct RequestWorkers(Arc<Workers>);
struct Workers {
    reads: Lane,
    writes: Lane,
}
struct Lane {
    runtime: Option<Runtime>,
    admitted: Arc<Semaphore>,
}
impl Lane {
    fn new(threads: usize, name: &str) -> std::io::Result<Self> {
        Ok(Self {
            runtime: Some(
                tokio::runtime::Builder::new_current_thread()
                    .max_blocking_threads(threads)
                    .thread_keep_alive(std::time::Duration::from_secs(10))
                    .thread_name(name)
                    .build()?,
            ),
            admitted: Arc::new(Semaphore::new(64)),
        })
    }
}
impl Drop for Lane {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}
tokio::task_local! { static REQUEST: (RequestWorkers, bool); }
impl RequestWorkers {
    /// Create lazy two-reader/32-writer blocking executors; no thread starts
    /// until work arrives. Returns an OS/runtime error if a driver cannot start.
    pub fn new() -> std::io::Result<Self> {
        Ok(Self(Arc::new(Workers {
            reads: Lane::new(2, "request-read")?,
            writes: Lane::new(32, "request-write")?,
        })))
    }
    /// Retain read scheduling while an HTTP stream body is polled after its
    /// handler returns. Cost: one pinned box per connection and an Arc clone per
    /// poll; items/errors pass through unchanged without another replay buffer.
    /// Dropping the body drops the stream and its existing listener bounds.
    pub(crate) fn read_stream<S>(
        &self,
        stream: S,
    ) -> impl futures_util::Stream<Item = S::Item> + Send + 'static
    where
        S: futures_util::Stream + Send + 'static,
    {
        let workers = self.clone();
        let mut stream = Box::pin(stream);
        futures_util::stream::poll_fn(move |cx| {
            REQUEST.sync_scope((workers.clone(), false), || stream.as_mut().poll_next(cx))
        })
    }
    /// Run an operation with its declared mutation class. Read scopes clear any
    /// outer writer context. No I/O, grants or results are cached by the scope.
    pub(crate) async fn scope<F: Future>(&self, mutates: bool, future: F) -> F::Output {
        REQUEST.scope((self.clone(), mutates), future).await
    }
}
/// Await one blocking callback. Outside an operation scope this uses the caller's
/// runtime; foreground scopes use their selected bounded executor. Panics
/// propagate as JoinError exactly as with Tokio's blocking adapter.
pub(crate) async fn spawn<F, R>(f: F) -> Result<R, JoinError>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let selected = REQUEST.try_with(Clone::clone).ok();
    if let Some((workers, mutates)) = selected {
        let lane = if mutates {
            &workers.0.writes
        } else {
            &workers.0.reads
        };
        let permit = lane
            .admitted
            .clone()
            .acquire_owned()
            .await
            .expect("request admission is never closed");
        let caller = tokio::runtime::Handle::current();
        lane.runtime
            .as_ref()
            .expect("request runtime")
            .spawn_blocking(move || {
                let _permit = permit;
                // Existing synchronous adapters may enter the listener for async
                // services. Keep that context; only this callback uses foreground slots.
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
    fn streaming_read_runs_when_writer_and_background_slots_are_occupied() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(2)
            .enable_all()
            .build()
            .unwrap();
        let workers = RequestWorkers::new().unwrap();
        runtime.block_on(async {
            let mut releases = Vec::new();
            let mut jobs = Vec::new();
            for _ in 0..32 {
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
            for _ in 0..2 {
                let (entered, ready) = tokio::sync::oneshot::channel();
                let (release, wait) = std::sync::mpsc::channel();
                releases.push(release);
                jobs.push(tokio::spawn(async move {
                    tokio::task::spawn_blocking(move || {
                        entered.send(()).unwrap();
                        wait.recv().unwrap();
                    })
                    .await
                    .unwrap();
                }));
                ready.await.unwrap();
            }
            use futures_util::StreamExt;
            // Axum polls a response body after the handler's task-local scope
            // has returned; the stream must retain its own read scheduling.
            let stream = workers.read_stream(futures_util::stream::once(spawn(|| 42)));
            tokio::pin!(stream);
            let read = tokio::time::timeout(std::time::Duration::from_secs(1), stream.next()).await;
            for release in releases {
                release.send(()).unwrap();
            }
            for job in jobs {
                job.await.unwrap();
            }
            assert_eq!(
                read.ok().flatten().and_then(Result::ok),
                Some(42),
                "read queued behind writer or background work"
            );
        });
    }
}
