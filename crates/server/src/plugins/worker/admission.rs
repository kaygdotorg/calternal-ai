//! Revision-scoped lookup, cold start, capacity arbitration and dispatch.
use super::*;

impl WorkerPool {
    /// Run one operation on the (plugin, space) worker, starting it if needed.
    /// `space` selects the process and is the space its session is pinned to.
    /// Cost: the pool lock for a lookup (never held across I/O); a cold start hashes
    /// the bundle on a blocking task and awaits sandbox startup with a reserved pool
    /// slot; then one channel round trip plus the worker's own frames and effects.
    pub async fn invoke(&self, plugin: &Arc<Plugin>, space: SpaceId, call: Call) -> Outcome {
        let account = call.account;
        self.dispatch(plugin, space, account, Some(call)).await
    }

    /// Establish a fresh candidate worker and require its bounded handshake before reuse.
    /// This gives the candidate no operation, caller capability or space data.
    pub(in crate::plugins) async fn prepare(
        &self,
        plugin: &Arc<Plugin>,
        space: SpaceId,
        account: AccountId,
    ) -> Result<(), Error> {
        match self.dispatch(plugin, space, account, None).await {
            Outcome::Complete(_) => Ok(()),
            Outcome::Failed(error) => Err(error),
            _ => Err(Error::unavailable("plugin candidate did not become ready")),
        }
    }

    pub(in crate::plugins) fn discard_candidate(&self, plugin: &Plugin, space: SpaceId) {
        if let Ok(mut workers) = self.workers.lock() {
            workers.remove(&(plugin.manifest.id.clone(), space, plugin.revision));
        }
    }

    async fn dispatch(
        &self,
        plugin: &Arc<Plugin>,
        space: SpaceId,
        account: AccountId,
        call: Option<Call>,
    ) -> Outcome {
        if self.closing.load(Ordering::Acquire) {
            return Outcome::Failed(Error::unavailable("worker pool is stopping"));
        }
        let Some(backend) = &self.backend else {
            return Outcome::Failed(Error::unavailable("worker execution unavailable"));
        };
        if let Some(why) = &plugin.unavailable {
            return Outcome::Failed(Error::unavailable(format!("plugin unavailable: {why}")));
        }
        let Some(bundle) = &plugin.worker else {
            return Outcome::Failed(Error::unavailable("plugin has no worker"));
        };
        let requires_ready = call.is_none();
        let (reply, answer) = oneshot::channel();
        let mut pending = call.map(|call| {
            Box::new(Pending {
                deadline: Instant::now() + call.timeout,
                call,
                reply,
            })
        });
        // A worker that exited idle between lookup and send drops its receiver;
        // the send then fails and one fresh worker is started.
        let key = (plugin.manifest.id.clone(), space, plugin.revision);
        // A per-key admission lock prevents duplicate cold starts. The small map
        // lock is released before waiting or I/O; weak entries are pruned on use.
        let gate = {
            let mut starts = self.starts.lock().unwrap_or_else(|p| p.into_inner());
            starts.retain(|_, weak| weak.strong_count() > 0);
            if let Some(gate) = starts.get(&key).and_then(std::sync::Weak::upgrade) {
                gate
            } else {
                if starts.len() >= self.max_workers.saturating_mul(2) {
                    return Outcome::Failed(
                        Error::unavailable("worker admission capacity exhausted")
                            .with_details(serde_json::json!({"reason":"worker_capacity"})),
                    );
                }
                let gate = Arc::new(StartKey {
                    admission: Arc::new(tokio::sync::Mutex::new(())),
                    worker: Arc::new(tokio::sync::Semaphore::new(1)),
                });
                starts.insert(key.clone(), Arc::downgrade(&gate));
                gate
            }
        };
        let startup_deadline = tokio::time::Instant::now() + WORKER_START_TIMEOUT;
        let mut admission =
            match tokio::time::timeout_at(startup_deadline, gate.admission.clone().lock_owned())
                .await
            {
                Ok(guard) => guard,
                Err(_) => {
                    return Outcome::Failed(Error::unavailable(
                        "worker admission timed out; retry shortly",
                    ));
                }
            };
        if let Some(pending) = pending.as_mut() {
            pending.deadline = Instant::now() + pending.call.timeout;
        }
        for _ in 0..2 {
            let start = {
                let mut workers = match self.workers.lock() {
                    Ok(workers) => workers,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                if workers.get(&key).is_some_and(Worker::usable) {
                    false
                } else {
                    workers.remove(&key);
                    if let Err(error) = self.make_room(&mut workers) {
                        return Outcome::Failed(error);
                    }
                    true
                }
            };
            if start {
                let key_permit = match tokio::time::timeout_at(
                    startup_deadline,
                    gate.worker.clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(permit)) => permit,
                    _ => {
                        return Outcome::Failed(Error::unavailable(
                            "the previous worker is still draining; retry shortly",
                        ));
                    }
                };
                // A permit survives retirement until actual sandbox teardown,
                // bounding physical workers even while an old session drains.
                let permit = match tokio::time::timeout_at(
                    startup_deadline,
                    self.permits.clone().acquire_owned(),
                )
                .await
                {
                    Ok(Ok(permit)) => permit,
                    _ => {
                        return Outcome::Failed(Error::unavailable(
                            "worker capacity is still draining; retry shortly",
                        ));
                    }
                };
                let timeout = pending
                    .as_ref()
                    .map_or(WORKER_START_TIMEOUT, |p| p.call.timeout);
                let budget = if backend.name() == "machine" && timeout == WORKER_TIMEOUT {
                    timeout + REMOTE_WORKER_START_ALLOWANCE
                } else {
                    timeout
                };
                let deadline = Instant::now() + budget;
                if let Some(pending) = pending.as_mut() {
                    pending.deadline = deadline;
                }
                let execution_deadline =
                    startup_deadline.min(tokio::time::Instant::from_std(deadline));
                let (node, plugin2, bundle2, in_flight) = (
                    self.node.clone(),
                    plugin.clone(),
                    bundle.clone(),
                    self.in_flight.clone(),
                );
                // Admission and lifetime permits travel with the start. A cancelled
                // caller cannot release either while a detached startup runs.
                let backend = backend.clone();
                let runtime = tokio::runtime::Handle::current();
                let start_key = gate.clone();
                let start_task = tokio::spawn(async move {
                    (
                        admission,
                        spawn(
                            node.as_ref(),
                            permit,
                            key_permit,
                            start_key,
                            backend,
                            runtime,
                            account,
                            &plugin2,
                            &bundle2,
                            space,
                            in_flight,
                            requires_ready,
                        )
                        .await,
                    )
                });
                let spawned = match tokio::time::timeout_at(execution_deadline, start_task).await {
                    Ok(result) => result,
                    Err(_) => {
                        return Outcome::Failed(Error::unavailable(
                            if Instant::now() >= deadline {
                                "extension timed out"
                            } else {
                                "worker startup timed out; check the enrolled machine runtime"
                            },
                        ));
                    }
                };
                let (guard, worker) = match spawned {
                    Ok((guard, Ok(worker))) => (guard, worker),
                    Ok((_, Err(error))) => return Outcome::Failed(error),
                    Err(_) => return Outcome::Failed(Error::internal("worker start failed")),
                };
                admission = guard;
                let mut workers = match self.workers.lock() {
                    Ok(w) => w,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                if self.closing.load(Ordering::Acquire) {
                    return Outcome::Failed(Error::unavailable("worker pool is stopping"));
                }
                // A concurrent cold start may have won; keep one worker per key.
                if workers.get(&key).is_some_and(Worker::usable) {
                    drop(worker); // stops the losing duplicate
                } else {
                    workers.insert(key.clone(), worker);
                }
            }
            let Some(request) = pending.take() else {
                return Outcome::Complete(serde_json::Value::Null);
            };
            let sent = {
                let workers = match self.workers.lock() {
                    Ok(w) => w,
                    Err(_) => return Outcome::Failed(Error::internal("worker pool poisoned")),
                };
                let Some(worker) = workers.get(&key) else {
                    pending = Some(request);
                    continue;
                };
                if worker
                    .busy
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                        (n < calternal_plugin_host::MAX_IN_FLIGHT).then_some(n + 1)
                    })
                    .is_err()
                {
                    return Outcome::Failed(Error::unavailable(
                        "extension worker is busy; retry shortly",
                    ));
                }
                self.in_flight.fetch_add(1, Ordering::AcqRel);
                if let Some(cancel) = &request.call.cancel {
                    cancel.bind(&worker.events);
                }
                match worker.events.try_send(Event::Invoke(request)) {
                    Ok(()) => None,
                    Err(mpsc::TrySendError::Full(_)) => {
                        worker.busy.fetch_sub(1, Ordering::AcqRel);
                        self.in_flight.fetch_sub(1, Ordering::AcqRel);
                        return Outcome::Failed(Error::unavailable(
                            "extension worker input queue is full",
                        ));
                    }
                    Err(mpsc::TrySendError::Disconnected(event)) => {
                        worker.busy.fetch_sub(1, Ordering::AcqRel);
                        self.in_flight.fetch_sub(1, Ordering::AcqRel);
                        drop(workers);
                        if let Ok(mut workers) = self.workers.lock() {
                            workers.remove(&key);
                        }
                        match event {
                            Event::Invoke(p) => Some(p),
                            _ => unreachable!("only invocations are sent here"),
                        }
                    }
                }
            };
            match sent {
                None => {
                    drop(admission);
                    return answer.await.unwrap_or_else(|_| {
                        Outcome::Failed(Error::unavailable("extension worker stopped"))
                    });
                }
                Some(p) => pending = Some(p),
            }
        }
        Outcome::Failed(Error::unavailable("extension worker could not start"))
    }

    /// Evict one idle worker if the pool is full; refuse when every worker is busy.
    fn make_room(&self, workers: &mut HashMap<WorkerKey, Worker>) -> Result<(), Error> {
        workers.retain(|_, w| w.usable());
        if self.permits.available_permits() > 0 {
            return Ok(());
        }
        let idle = workers
            .iter()
            .find(|(_, w)| w.busy.load(Ordering::Acquire) == 0)
            .map(|(k, _)| k.clone());
        match idle {
            Some(key) => {
                tracing::debug!("evicting idle worker");
                workers.remove(&key); // dropping the handle stops it
                Ok(())
            }
            None if workers.len() < self.max_workers => Ok(()),
            None => Err(
                Error::unavailable("extension workers are busy; retry shortly")
                    .with_details(serde_json::json!({ "reason": "worker_capacity" })),
            ),
        }
    }
}
