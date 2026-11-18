//! Per-space worker session, authorization and deadline enforcement.
use super::*;

/// Adapts the per-invocation hosts to the library's broker interface.
/// The grant in each authority is the host-issued invocation ID.
struct Broker<'a>(&'a BTreeMap<String, Active>);

impl HostBroker for Broker<'_> {
    fn authorize(&mut self, authority: &Authority, required: ManifestCapability) -> bool {
        self.0.get(&authority.grant).is_some_and(|a| {
            !a.cancel.as_ref().is_some_and(|c| c.is_cancelled()) && a.host.authorize(required)
        })
    }
    fn authorize_pinned(&mut self, authority: &Authority, required: ManifestCapability) -> bool {
        self.0.get(&authority.grant).is_some_and(|a| {
            !a.cancel.as_ref().is_some_and(|c| c.is_cancelled())
                && a.host.authorize_pinned(required)
        })
    }
    fn authorize_delivery(&mut self, authority: &Authority, required: ManifestCapability) -> bool {
        self.0.get(&authority.grant).is_some_and(|a| {
            !a.cancel.as_ref().is_some_and(|c| c.is_cancelled())
                && a.host.authorize_delivery(required)
        })
    }
    fn call(
        &mut self,
        authority: &Authority,
        method: calternal_core::HostCallKind,
        input: &Value,
    ) -> Result<Value, calternal_plugin_host::Error> {
        let active = self
            .0
            .get(&authority.grant)
            .ok_or(calternal_plugin_host::Error::Refused)?;
        active.host.call(method, input)
    }
}

/// State owned by one worker's session thread.
pub(super) struct Thread {
    pub(super) _process: Process,
    pub(super) session: WorkerSession,
    pub(super) space: SpaceId,
    pub(super) frames: SyncSender<Vec<u8>>,
    pub(super) active: BTreeMap<String, Active>,
    pub(super) next: u32,
    pub(super) alive: Arc<AtomicBool>,
    pub(super) busy: Arc<AtomicUsize>,
    pub(super) in_flight: Arc<AtomicUsize>,
}

/// Why a worker process is being stopped.
enum Stop {
    Idle,
    Fault(&'static str),
}

impl Thread {
    pub(super) fn run(mut self, rx: Receiver<Event>) {
        let mut idle_since = Instant::now();
        let stop = loop {
            let cancelled: Vec<_> = self
                .active
                .values()
                .filter_map(|a| a.cancel.clone())
                .filter(|c| c.is_cancelled())
                .collect();
            for flag in cancelled {
                self.cancel(&flag);
            }
            let wait = self
                .active
                .values()
                .map(|a| a.deadline.saturating_duration_since(Instant::now()))
                .min()
                .unwrap_or_else(|| WORKER_IDLE.saturating_sub(idle_since.elapsed()));
            let was_busy = !self.active.is_empty();
            match rx.recv_timeout(wait) {
                Ok(Event::Invoke(p)) => self.invoke(*p),
                Ok(Event::Line(line)) => {
                    if let Err(why) = self.line(&line) {
                        break Stop::Fault(why);
                    }
                }
                Ok(Event::Cancel(flag)) => self.cancel(&flag),
                Ok(Event::Overflow) => break Stop::Fault("extension output exceeded limit"),
                Ok(Event::Closed) => break Stop::Fault("extension worker stopped"),
                Ok(Event::Shutdown) if self.active.is_empty() => break Stop::Idle,
                Ok(Event::Shutdown) => self.retire(),
                Err(RecvTimeoutError::Timeout) if self.active.is_empty() => {
                    // Mark unusable first; an invocation that raced in is still served.
                    self.alive.store(false, Ordering::Release);
                    match rx.try_recv() {
                        Ok(Event::Invoke(p)) => self.invoke(*p),
                        _ => break Stop::Idle,
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break Stop::Idle,
            }
            if was_busy || !self.active.is_empty() {
                idle_since = Instant::now();
            }
            if self.active.is_empty() && idle_since.elapsed() >= WORKER_IDLE {
                break Stop::Idle;
            }
            // Checked after every event, so a chatty worker cannot outrun its deadline.
            if self.active.values().any(|a| a.deadline <= Instant::now()) {
                break Stop::Fault("extension timed out");
            }
            if !self.alive.load(Ordering::Acquire) && self.active.is_empty() {
                break Stop::Idle;
            }
        };
        self.alive.store(false, Ordering::Release);
        tracing::debug!(space=%self.space,"worker session stopped");
        let message = match stop {
            Stop::Idle => "extension worker stopped",
            Stop::Fault(why) => {
                tracing::warn!(reason = why, "worker stopped");
                why
            }
        };
        for id in self.active.keys().cloned().collect::<Vec<_>>() {
            self.finish(&id, Outcome::Failed(Error::unavailable(message)));
        }
        // Reply to invocations queued behind the stop instead of dropping them silently.
        while let Ok(event) = rx.try_recv() {
            if let Event::Invoke(p) = event {
                self.busy.fetch_sub(1, Ordering::AcqRel);
                self.in_flight.fetch_sub(1, Ordering::AcqRel);
                let _ = p.reply.send(Outcome::Failed(Error::unavailable(message)));
            }
        }
    }

    /// Stop accepting new work; the thread exits once in-flight invocations finish.
    fn retire(&mut self) {
        self.alive.store(false, Ordering::Release);
    }

    fn invoke(&mut self, p: Pending) {
        if p.deadline <= Instant::now() {
            self.busy.fetch_sub(1, Ordering::AcqRel);
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            let _ = p
                .reply
                .send(Outcome::Failed(Error::unavailable("extension timed out")));
            return;
        }
        self.next += 1;
        if self.next >= MAX_LIFETIME_INVOCATIONS {
            // Serve this last one; the pool starts a replacement for the next call.
            self.retire();
        }
        let id = format!("i{}", self.next);
        let authority = Authority {
            account_id: p.call.account,
            space_id: self.space,
            grant: id.clone(),
        };
        self.active.insert(
            id.clone(),
            Active {
                host: p.call.host,
                cancel: p.call.cancel.clone(),
                reply: p.reply,
                deadline: p.deadline,
            },
        );
        let admitted = self.session.invoke(
            id.clone(),
            p.call.operation,
            p.call.input,
            authority,
            &mut Broker(&self.active),
        );
        if admitted.is_ok()
            && let Some(job) = &p.call.job
            && self.session.attach_job(&id, job).is_err()
        {
            let _ = self.session.cancel(&id);
            self.finish(&id, Outcome::Refused);
            self.flush(&id);
            return;
        }
        match admitted {
            Ok(()) if p.call.cancel.as_ref().is_some_and(|c| c.is_cancelled()) => {
                if let Some(flag) = p.call.cancel {
                    self.cancel(&flag);
                }
            }
            Ok(()) => self.flush(&id),
            Err(calternal_plugin_host::Error::Invalid) => self.finish(&id, Outcome::InvalidInput),
            Err(calternal_plugin_host::Error::Refused) => self.finish(&id, Outcome::Refused),
            Err(_) => self.finish(
                &id,
                Outcome::Failed(
                    Error::unavailable("extension worker is busy; retry shortly")
                        .with_details(serde_json::json!({ "reason": "worker_capacity" })),
                ),
            ),
        }
    }

    /// Handle one worker frame. `Err` means the process must be stopped.
    fn line(&mut self, line: &[u8]) -> Result<(), &'static str> {
        let frame = decode_frame(line).map_err(|_| "extension returned invalid output")?;
        let id = frame.invocation_id.clone();
        if !self.active.contains_key(&id) {
            // Late output for an invocation already answered: discard it.
            return Ok(());
        }
        let delivers = matches!(
            frame.message,
            Message::Complete { .. } | Message::Failed {} | Message::Chunk { .. }
        );
        match self.session.receive(frame, &mut Broker(&self.active)) {
            Ok(()) => {}
            Err(calternal_plugin_host::Error::Refused) => {
                // Undeclared/forged host call, or authority no longer current.
                let _ = self.session.cancel(&id);
                self.finish(&id, Outcome::Refused);
            }
            Err(calternal_plugin_host::Error::BackPressure) => {
                return Err("extension exceeded its queue");
            }
            Err(_) => {
                let _ = self.session.cancel(&id);
                self.finish(
                    &id,
                    Outcome::Failed(Error::unavailable("extension returned invalid output")),
                );
            }
        }
        if delivers {
            self.deliver(&id);
        }
        self.flush(&id);
        Ok(())
    }

    /// Cancel the invocation holding `flag`: invalidate it in the session, queue the
    /// worker's `cancel` frame and answer [`Outcome::Cancelled`].
    fn cancel(&mut self, flag: &Arc<CancelFlag>) {
        let Some(id) = self
            .active
            .iter()
            .find(|(_, a)| a.cancel.as_ref().is_some_and(|c| Arc::ptr_eq(c, flag)))
            .map(|(id, _)| id.clone())
        else {
            return;
        };
        let _ = self.session.cancel(&id);
        self.finish(&id, Outcome::Cancelled);
        self.flush(&id);
    }

    /// Drain the delivery for `id` that a chunk or completion just queued. A chunk
    /// goes to the invocation's host; a completion answers the caller.
    fn deliver(&mut self, id: &str) {
        if !self.active.contains_key(id) {
            return;
        }
        let outcome = match self.session.next_delivery(&mut Broker(&self.active)) {
            Ok(Some(Frame {
                message: Message::Chunk { job_id, data, .. },
                ..
            })) => {
                if let Some(active) = self.active.get(id) {
                    active.host.chunk(&job_id, &data);
                }
                return;
            }
            Ok(Some(Frame {
                message: Message::Complete { data },
                ..
            })) => Outcome::Complete(data),
            Ok(Some(_)) => Outcome::Failed(Error::unavailable("extension failed")),
            Ok(None) => return,
            Err(_) => Outcome::Refused,
        };
        self.finish(id, outcome);
    }

    /// Send every queued host frame. Every enqueue is followed by a flush on this
    /// thread, so queued frames belong to `id` (or are cancellations, which bypass
    /// the authority check). A frame refused because `id`'s authority no longer
    /// holds is discarded and `id` refused. A full transport queue stops the worker.
    fn flush(&mut self, id: &str) {
        loop {
            let frame = match self.session.next_outgoing(&mut Broker(&self.active)) {
                Ok(Some(frame)) => frame,
                Ok(None) => return,
                Err(calternal_plugin_host::Error::Refused) => {
                    let _ = self.session.cancel(id);
                    self.finish(id, Outcome::Refused);
                    continue;
                }
                Err(_) => continue,
            };
            let Ok(bytes) = encode_frame(&frame) else {
                continue;
            };
            if self.frames.try_send(bytes).is_err() {
                // The worker stopped reading: stop it rather than block this thread.
                self.alive.store(false, Ordering::Release);
                return;
            }
        }
    }

    fn finish(&mut self, id: &str, outcome: Outcome) {
        if let Some(active) = self.active.remove(id) {
            self.busy.fetch_sub(1, Ordering::AcqRel);
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            let _ = active.reply.send(outcome);
        }
    }
}
