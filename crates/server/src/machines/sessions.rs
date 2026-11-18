//! The machine-facing side of [`MachineService`]: enrolment, signed sessions,
//! the long-poll `sync` and output uploads. Wire types are in
//! [`calternal_machine::protocol`].

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use calternal_auth::token::{hash32, normalize_setup_token, random32};
use calternal_machine::protocol::{
    Capacity, ChunkEvent, Down, EnrolRequest, EnrolResponse, Envelope, OutputAck, OutputPost,
    Owner, PROTOCOL_VERSION, SessionFinish, SessionGranted, SyncRequest, session_message,
};
use tokio::sync::Notify;
use uuid::Uuid;

use super::state::{Exec, ExecState, Live, Machine};
use super::{LeaseState, MachineService, blocking, db};
use calternal_types::Error;

/// Pending challenges kept at most; beyond it `session/start` answers 503.
const MAX_CHALLENGES: usize = 1024;
/// A challenge must be signed within this time.
const CHALLENGE_TTL: Duration = Duration::from_secs(60);

/// A fresh `cm1_` bearer token and its BLAKE3 hash (the only form kept).
fn new_token() -> Result<(String, [u8; 32]), Error> {
    let raw = random32().map_err(|_| Error::internal("no randomness"))?;
    let token = format!("cm1_{}", hex::encode(raw));
    let hash = hash32(&token);
    Ok((token, hash))
}

impl MachineService {
    fn owner_step(&self, owner: Owner) -> Result<Option<calternal_types::OwnerStep>, Error> {
        let Some(lifecycle) = self.lifecycle.get() else {
            return Ok(None);
        };
        match owner {
            Owner::Pool => Ok(None),
            Owner::Account { id } => lifecycle
                .step(id.as_uuid())
                .map(Some)
                .map_err(|_| Error::forbidden("machine owner revoked")),
        }
    }

    /// Redeem a code for a new machine bound to the presented public key.
    /// Blocks (one protected authority commit). Unknown, used and expired codes
    /// look the same.
    pub fn enrol(&self, req: &EnrolRequest) -> Result<EnrolResponse, Error> {
        let invalid = || Error::not_found("unknown, used or expired enrolment code");
        let canon = normalize_setup_token(&req.code).ok_or_else(invalid)?;
        let key = B64
            .decode(&req.public_key)
            .ok()
            .filter(|k| k.len() == 32)
            .ok_or_else(|| Error::invalid_input("public_key must be a base64 Ed25519 key"))?;
        let clean = |s: &str, n: usize| -> String {
            s.chars().filter(|c| !c.is_control()).take(n).collect()
        };
        let mut st = self.lock()?;
        let record = st
            .authority
            .redeem_code(
                hash32(&canon),
                key,
                clean(&req.name, 64),
                clean(&req.os, 32),
                clean(&req.arch, 32),
                req.direct,
            )
            .map_err(super::authority_error)?
            .ok_or_else(invalid)?;
        if let Some(account) = record.owner {
            super::erasure_queue::remember_account_machine(&st.db, account, record.id)?;
        }
        let row = db::MachineRow::new(record, false);
        let resp = EnrolResponse {
            machine_id: row.id,
            owner: row.owner,
        };
        let machine = Machine::new(row, Instant::now());
        st.readiness.insert(&machine);
        st.machines.insert(machine.row.id, machine);
        st.readiness_changed(resp.owner);
        Ok(resp)
    }

    /// Issue a 60-second single-use challenge for a known, unrevoked machine.
    /// No file I/O.
    pub fn session_start(&self, machine_id: Uuid) -> Result<String, Error> {
        let mut st = self.lock()?;
        match st.machines.get(&machine_id) {
            None => return Err(Error::not_found("unknown machine")),
            Some(m) if m.row.revoked => return Err(Error::forbidden("machine revoked")),
            Some(m) => {
                self.owner_step(m.row.owner)?;
            }
        }
        let now = Instant::now();
        st.challenges
            .retain(|_, (_, t)| now.duration_since(*t) < CHALLENGE_TTL);
        if st.challenges.len() >= MAX_CHALLENGES {
            return Err(Error::unavailable("too many pending sessions"));
        }
        let (challenge, _) = new_token()?;
        st.challenges.insert(challenge.clone(), (machine_id, now));
        Ok(challenge)
    }

    /// Read this machine's outstanding erasures without replacing its session.
    /// A single-use challenge binds the proof to the enrolled key.
    pub fn erasure_status(
        &self,
        request: &calternal_machine::protocol::ErasureStatusRequest,
    ) -> Result<calternal_machine::protocol::ErasureStatus, Error> {
        let mut st = self.lock()?;
        let denied = || Error::unauthorized("bad machine status proof");
        match st.challenges.remove(&request.challenge) {
            Some((id, time)) if id == request.machine_id && time.elapsed() < CHALLENGE_TTL => (),
            _ => return Err(denied()),
        }
        let machine = st.machines.get(&request.machine_id).ok_or_else(denied)?;
        if machine.row.revoked {
            return Err(Error::forbidden("machine revoked"));
        }
        let _owner = self.owner_step(machine.row.owner)?;
        let signature = B64.decode(&request.signature).map_err(|_| denied())?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &machine.row.public_key)
            .verify(
                &calternal_machine::protocol::erasure_status_message(
                    request.machine_id,
                    &request.challenge,
                ),
                &signature,
            )
            .map_err(|_| denied())?;
        Ok(calternal_machine::protocol::ErasureStatus {
            pending_erasures: super::erasure_queue::count(&st.db, request.machine_id)?,
        })
    }

    /// Verify the signed challenge and open a session, replacing any earlier one.
    /// One Ed25519 verification plus bounded discard reads/state commits on a blocking worker.
    ///
    /// Re-queues `Assign` for every lease the machine holds and `Exec` for every
    /// unfinished exec. A new `boot_id` means the machine role restarted: execs it
    /// was running are reported lost.
    pub fn session_finish(&self, f: &SessionFinish) -> Result<SessionGranted, Error> {
        if f.protocol != PROTOCOL_VERSION {
            return Err(Error::invalid_input("unsupported machine protocol version"));
        }
        let mut st = self.lock()?;
        let st = &mut *st;
        let denied = || Error::unauthorized("bad session proof");
        match st.challenges.remove(&f.challenge) {
            Some((m, t)) if m == f.machine_id && t.elapsed() < CHALLENGE_TTL => {}
            _ => return Err(denied()),
        }
        let machine = st.machines.get_mut(&f.machine_id).ok_or_else(denied)?;
        if machine.row.revoked {
            return Err(Error::forbidden("machine revoked"));
        }
        let _step = self.owner_step(machine.row.owner)?;
        let sig = B64.decode(&f.signature).map_err(|_| denied())?;
        ring::signature::UnparsedPublicKey::new(&ring::signature::ED25519, &machine.row.public_key)
            .verify(&session_message(f.machine_id, &f.challenge), &sig)
            .map_err(|_| denied())?;
        if f.backend == "direct" && (!machine.row.direct || machine.row.owner == Owner::Pool) {
            return Err(Error::forbidden(
                "the owner must enable direct execution with calternal machine set",
            ));
        }
        if !matches!(f.backend.as_str(), "direct" | "podman" | "auto") {
            return Err(Error::invalid_input("unsupported machine backend"));
        }
        let backend = if f.backend == "auto" {
            if machine.row.direct && machine.row.owner != Owner::Pool {
                "direct"
            } else {
                "podman"
            }
        } else {
            f.backend.as_str()
        }
        .to_owned();
        let (token, hash) = new_token()?;
        if let Some(old) = machine.live.take() {
            st.sessions.remove(&old.session);
            old.notify.notify_one();
        }
        st.sessions.insert(hash, f.machine_id);
        let rebooted = machine.boot_id.is_some_and(|b| b != f.boot_id);
        machine.boot_id = Some(f.boot_id);
        machine.last_seen = Instant::now();
        machine.last_seen_ms = Some(super::db::now_ms());
        machine.live = Some(Live {
            session: hash,
            direct: backend == "direct",
            outbox: VecDeque::new(),
            next_seq: 1,
            polls: 0,
            capacity: Capacity::default(),
            draining: false,
            notify: Arc::new(Notify::new()),
        });
        let owner = machine.row.owner;
        st.readiness_changed(owner);
        st.replay_discards(f.machine_id)?;
        let held: Vec<Uuid> = st
            .leases
            .values()
            .filter(|l| {
                l.row.machine == Some(f.machine_id)
                    && matches!(l.state, LeaseState::Assigned | LeaseState::Running)
            })
            .map(|l| l.row.id)
            .collect();
        for lease in held {
            st.send_assign(lease, &self.settings);
            if rebooted {
                st.lose_execs(lease, "the machine restarted");
            }
            st.send_execs(lease);
        }
        Ok(SessionGranted {
            backend,
            token,
            lease_ttl_ms: self.settings.lease_ttl.as_millis() as u64,
            poll_hold_ms: self.settings.poll_hold.as_millis() as u64,
        })
    }

    /// Resolve a bearer token to its machine; 401 if unknown, 403 if revoked.
    /// One BLAKE3 hash and a map lookup.
    pub fn authenticate(&self, token: &str) -> Result<Uuid, Error> {
        let st = self.lock()?;
        let id = *st
            .sessions
            .get(&hash32(token))
            .ok_or_else(|| Error::unauthorized("unknown machine session"))?;
        let machine = st
            .machines
            .get(&id)
            .filter(|machine| !machine.row.revoked)
            .ok_or_else(|| Error::forbidden("machine revoked"))?;
        self.owner_step(machine.row.owner)?;
        Ok(id)
    }

    /// Long-poll: apply the machine's acknowledgement and report, place waiting
    /// work, then return queued messages or wait up to `poll_hold`.
    ///
    /// Each pass takes the lock on a blocking task; a quiet poll writes nothing
    /// (commits happen when leases change, work is placed, or explicit discard results arrive).
    pub async fn sync(
        self: &Arc<Self>,
        machine_id: Uuid,
        token: &str,
        req: SyncRequest,
    ) -> Result<Vec<Envelope>, Error> {
        let session = hash32(token);
        let reports = !req.discards.is_empty();
        let svc = self.clone();
        let notify = blocking(move || svc.sync_begin(machine_id, &session, &req)).await?;
        let _guard = PollGuard {
            svc: self.clone(),
            machine_id,
        };
        let deadline = tokio::time::Instant::now() + self.settings.poll_hold;
        loop {
            let svc = self.clone();
            let pending = blocking(move || svc.outbox(machine_id, &session)).await?;
            if reports || !pending.is_empty() {
                return Ok(pending);
            }
            if tokio::time::timeout_at(deadline, notify.notified())
                .await
                .is_err()
            {
                return Ok(Vec::new());
            }
        }
    }

    fn sync_begin(
        &self,
        machine_id: Uuid,
        session: &[u8; 32],
        req: &SyncRequest,
    ) -> Result<Arc<Notify>, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        let _step = st
            .machines
            .get(&machine_id)
            .map(|m| self.owner_step(m.row.owner))
            .transpose()?;
        live_mut(st.machines.get_mut(&machine_id), session)?;
        st.finish_discards(machine_id, &req.discards)?;
        let live = live_mut(st.machines.get_mut(&machine_id), session)?;
        live.outbox.retain(|e| e.seq > req.ack);
        let readiness_changed = live.capacity.max_sandboxes != req.capacity.max_sandboxes
            || live.capacity.running_sandboxes != req.capacity.running_sandboxes
            || live.draining != req.draining;
        live.capacity = req.capacity;
        live.draining = req.draining;
        live.polls += 1;
        let notify = live.notify.clone();
        if let Some(m) = st.machines.get_mut(&machine_id) {
            m.last_seen = Instant::now();
            m.last_seen_ms = Some(super::db::now_ms());
            let owner = m.row.owner;
            if readiness_changed {
                st.readiness_changed(owner);
            }
        }
        if st.finish_worker_stops(machine_id, session, req)? > 0 {
            self.worker_changes.notify_waiters();
        }
        st.reconcile(machine_id, &req.leases, &self.settings);
        st.place_waiting(&self.settings);
        Ok(notify)
    }

    fn outbox(&self, machine_id: Uuid, session: &[u8; 32]) -> Result<Vec<Envelope>, Error> {
        let mut st = self.lock()?;
        let owner = st
            .machines
            .get(&machine_id)
            .ok_or_else(|| Error::forbidden("machine revoked"))?
            .row
            .owner;
        let _owner = self.owner_step(owner)?;
        live_mut(st.machines.get_mut(&machine_id), session)?;
        st.deliver_erasures(machine_id)?;
        let live = live_mut(st.machines.get_mut(&machine_id), session)?;
        let queued = std::mem::take(&mut live.outbox);
        let mut deliver = Vec::with_capacity(queued.len());
        let mut keep = VecDeque::new();
        for envelope in queued {
            let lease = match &envelope.message {
                Down::Assign { lease_id, .. }
                | Down::Exec { lease_id, .. }
                | Down::Input { lease_id, .. } => Some(lease_id),
                _ => None,
            };
            let _steps = match lease {
                Some(id) => match st.leases.get(id).and_then(|l| st.lease_steps(&l.row).ok()) {
                    Some(steps) => steps,
                    None => continue,
                },
                None => None,
            };
            deliver.push(envelope.clone());
            keep.push_back(envelope);
        }
        live_mut(st.machines.get_mut(&machine_id), session)?.outbox = keep;
        Ok(deliver)
    }

    /// Accept exec output from the machine holding the lease at this fence.
    /// No file I/O.
    ///
    /// Accepts the longest in-order prefix that fits the exec's buffer and acks
    /// it; a full buffer answers `retry_after_ms`. Wrong machine, stale fence or
    /// ended lease: 409.
    pub fn output(&self, machine_id: Uuid, post: OutputPost) -> Result<OutputAck, Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        let fenced = || Error::conflict("lease fenced");
        let lease = st.leases.get(&post.lease_id).ok_or_else(fenced)?;
        if lease.row.machine != Some(machine_id)
            || lease.row.fence != post.fence
            || matches!(lease.state, LeaseState::Ended | LeaseState::Stopping)
        {
            return Err(fenced());
        }
        let _account = self.owner_step(Owner::Account {
            id: lease.row.account,
        })?;
        let _space = self
            .lifecycle
            .get()
            .map(|l| l.step(lease.row.space.as_uuid()))
            .transpose()?;
        let account = lease.row.account;
        let first = post.chunks.first().map_or(1, |c| c.seq);
        let exec = st.execs.entry(post.exec_id).or_insert_with(|| {
            // Unknown exec after a server restart: accept from wherever it is.
            let mut e = Exec::new(
                post.lease_id,
                account,
                Vec::new(),
                Vec::new(),
                ExecState::Sent,
            );
            e.last_seq = first.saturating_sub(1);
            e.lost_before = (first > 1).then_some(first);
            e
        });
        if exec.lease != post.lease_id {
            return Err(fenced());
        }
        let (mut full, mut accepted) = (false, false);
        for chunk in post.chunks {
            if exec.state == ExecState::Done || chunk.seq <= exec.last_seq {
                continue;
            }
            if chunk.seq != exec.last_seq + 1 {
                break;
            }
            let size = match &chunk.event {
                ChunkEvent::Data { data, .. } => data.len() / 4 * 3,
                ChunkEvent::Exit(_) => 0,
            };
            if exec.held > 0 && exec.held + size > self.settings.output_buffer_bytes {
                full = true;
                break;
            }
            if matches!(chunk.event, ChunkEvent::Exit(_)) {
                exec.state = ExecState::Done;
                exec.finished = Some(Instant::now());
            }
            exec.last_seq = chunk.seq;
            exec.held += size;
            exec.chunks.push_back((chunk, size));
            accepted = true;
        }
        if accepted {
            exec.notify.notify_one();
        }
        Ok(OutputAck {
            acked: exec.last_seq,
            retry_after_ms: full.then_some(250),
        })
    }
}

fn live_mut<'a>(m: Option<&'a mut Machine>, session: &[u8; 32]) -> Result<&'a mut Live, Error> {
    let m = m.ok_or_else(|| Error::unauthorized("unknown machine"))?;
    if m.row.revoked {
        return Err(Error::forbidden("machine revoked"));
    }
    match m.live.as_mut() {
        Some(l) if &l.session == session => Ok(l),
        _ => Err(Error::unauthorized("session replaced")),
    }
}

/// Counts a held poll so the sweeper keeps the machine online; releases the
/// count on a blocking task when the request ends or is cancelled.
struct PollGuard {
    svc: Arc<MachineService>,
    machine_id: Uuid,
}

impl Drop for PollGuard {
    fn drop(&mut self) {
        let (svc, id) = (self.svc.clone(), self.machine_id);
        tokio::task::spawn_blocking(move || {
            if let Ok(mut st) = svc.state.lock()
                && let Some(m) = st.machines.get_mut(&id)
            {
                m.last_seen = Instant::now();
                m.last_seen_ms = Some(super::db::now_ms());
                if let Some(live) = m.live.as_mut() {
                    live.polls = live.polls.saturating_sub(1);
                }
            }
        });
    }
}
