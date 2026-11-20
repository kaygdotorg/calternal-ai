//! Enrolled HTTP upgrade and live assignment bindings for plugin carriers.
use super::{MachineService, blocking, http, state::State};
use crate::api::{Shared, failure};
use crate::auth::Directory;
use axum::{
    extract::{Path, Request, State as HttpState},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use calternal_auth::db::GrantRevision;
use calternal_machine::protocol::{
    Down, PluginWorker, SPACE_CHANNEL_UPGRADE, SpaceChannelAssignment,
};
use calternal_plugin_host::space_channel::{PackagePin, SpaceChannel};
use calternal_types::{AccountId, Error, ErrorCode, SpaceId};
use std::{sync::Arc, time::Duration};
use tokio::sync::{oneshot, watch};
use uuid::Uuid;

mod residency;
pub(crate) use residency::{Invocation, Residency};

/// Durable terminal cause for recovering a pending checkpoint acknowledgement.
pub(super) const SMALL_VM_IDLE_REASON: &str = "small plugin runtime idle";

// Preview host limit on live streams and reservations, independent of the
// content cache. The scheduler also enforces each machine's actual capacity.
const MAX_CHANNELS: usize = 32;
const OPEN_LIMIT: Duration = Duration::from_secs(5);

pub(crate) struct Connected {
    pub(crate) space: SpaceId,
    pub(crate) channel: SpaceChannel,
    pub(crate) grants: GrantRevision,
    /// Readiness for first invocation after any checkpoint proof is spent.
    pub(crate) restore_consumed: watch::Receiver<bool>,
    /// Idle policy and admission state shared with the placement scheduler.
    pub(crate) residency: Arc<Residency>,
}

pub(super) struct Reservation {
    binding: SpaceChannelAssignment,
    machine: Uuid,
    account: AccountId,
    session: [u8; 32],
    closing: watch::Sender<bool>,
    restore_consumed: watch::Sender<bool>,
    residency: Arc<Residency>,
    reply: Option<oneshot::Sender<Connected>>,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.closing.send_replace(true);
    }
}

/// One admitted small-VM lease. Dropping a cancelled wake queues normal teardown.
pub(crate) struct HeldPlacement {
    service: Arc<MachineService>,
    account: AccountId,
    /// Server-assigned lease used to bind the authenticated carrier.
    pub(crate) lease: Uuid,
    runtime: tokio::runtime::Handle,
    settled: bool,
}

impl HeldPlacement {
    /// Schedule the current lease's checkpoint and await physical teardown.
    pub(crate) async fn suspend(&mut self) -> Result<(), Error> {
        let (service, account, lease) = (self.service.clone(), self.account, self.lease);
        blocking(move || service.sleep_space_run(account, lease)).await?;
        self.settled = true;
        self.service.clone().wait_worker_stopped(lease).await
    }
}

impl Drop for HeldPlacement {
    fn drop(&mut self) {
        if self.settled {
            return;
        }
        let (service, account, lease) = (self.service.clone(), self.account, self.lease);
        self.runtime.spawn(async move {
            let _ = crate::blocking::spawn(move || service.stop_run(account, lease)).await;
        });
    }
}

enum Eviction {
    Carrier(watch::Receiver<bool>),
    Stopping(Uuid),
}

impl State {
    fn require_channel_capacity(&mut self, space: SpaceId) -> Result<(), Error> {
        self.space_channels.retain(|_, reservation| {
            !*reservation.closing.borrow()
                && reservation
                    .reply
                    .as_ref()
                    .is_none_or(|reply| !reply.is_closed())
        });
        if self.space_channels.contains_key(&space) || self.space_channels.len() >= MAX_CHANNELS {
            return Err(Error::unavailable(
                "space already has a carrier or carrier capacity is full",
            ));
        }
        Ok(())
    }

    fn evict_small_vm(
        &self,
        account: AccountId,
        space: SpaceId,
        settings: &super::Settings,
    ) -> Result<Option<Eviction>, Error> {
        let (_, excluded) = super::checkpoint::placement(&self.db, space)?;
        let needs_slot = |id: Uuid| {
            self.machines.get(&id).is_some_and(|machine| {
                !excluded.contains(&id)
                    && self.may_use(machine, account)
                    && Self::worker_machine(machine, settings, true)
                    && self.small_vm_slots(machine) == 0
            })
        };
        if let Some(lease) = self.worker_stops.iter().find_map(|(id, stop)| {
            (needs_slot(stop.machine)
                && self.leases.get(id).is_some_and(|lease| lease.is_small_vm()))
            .then_some(*id)
        }) {
            return Ok(Some(Eviction::Stopping(lease)));
        }
        let eligible = self.space_channels.values().filter(|slot| {
            slot.binding.space != space && !*slot.closing.borrow() && needs_slot(slot.machine)
        });
        let mut idle = Vec::new();
        for reservation in eligible {
            if let Some(waiter) = reservation.residency.drain_waiter() {
                return Ok(Some(Eviction::Carrier(waiter)));
            }
            if let Some(last) = reservation.residency.last_idle() {
                idle.push((last, reservation));
            }
        }
        idle.sort_unstable_by_key(|(last, reservation)| (*last, reservation.binding.space));
        Ok(idle.into_iter().find_map(|(last, slot)| {
            if slot.residency.sleep(last) {
                slot.residency.drain_waiter().map(Eviction::Carrier)
            } else {
                None
            }
        }))
    }

    /// Persist consumption for the exact running carrier before admitting calls.
    /// The sync worker holds the scheduler lock across this one conditional write
    /// so a concurrent wake cannot reuse the proof before admission opens.
    pub(super) fn consume_restore(
        &self,
        space: SpaceId,
        machine: Uuid,
        lease: Uuid,
    ) -> Result<(), Error> {
        if let Some(reservation) = self.space_channels.get(&space)
            && reservation.binding.lease_id == lease
            && reservation.machine == machine
            && let Some(counter) = reservation.binding.checkpoint
        {
            if *reservation.restore_consumed.borrow() {
                return Ok(());
            }
            super::checkpoint::consume(&self.db, space, machine, counter)?;
            reservation.restore_consumed.send_replace(true);
        }
        Ok(())
    }

    /// Recheck machine inventory promptly while its restored guest awaits admission.
    pub(super) fn awaiting_restore_report(&self, machine: Uuid) -> bool {
        self.space_channels.values().any(|reservation| {
            reservation.machine == machine
                && !*reservation.closing.borrow()
                && !*reservation.restore_consumed.borrow()
        })
    }

    pub(super) fn close_space_lease(&mut self, lease: Uuid) {
        if let Some(space) = self.leases.get(&lease).map(|row| row.row.space)
            && self
                .space_channels
                .get(&space)
                .is_some_and(|r| r.binding.lease_id == lease)
        {
            self.space_channels.remove(&space);
        }
    }

    pub(super) fn close_account_channels(&mut self, account: AccountId) {
        self.space_channels.retain(|_, r| r.account != account);
    }

    pub(super) fn close_machine_channels(&mut self, machine: Uuid) {
        self.space_channels.retain(|_, r| r.machine != machine);
    }
}

impl MachineService {
    /// End an idle small placement and request a snapshot. Hold the
    /// scheduler lock through its durable terminal lease transition and
    /// queueing Stop; capacity stays reserved until machine acknowledgement.
    pub(crate) fn sleep_space_run(&self, account: AccountId, lease: Uuid) -> Result<(), Error> {
        let mut st = self.lock()?;
        let placement = st
            .leases
            .get(&lease)
            .filter(|l| l.row.account == account && l.is_small_vm())
            .ok_or_else(no_assignment)?;
        if matches!(
            placement.state,
            super::LeaseState::Stopping | super::LeaseState::Ended
        ) {
            return Ok(());
        }
        let machine = placement.assigned_machine().ok_or_else(no_assignment)?;
        let space = placement.row.space;
        st.advance_checkpoint(space, machine, lease)?;
        st.end_with_files(lease, SMALL_VM_IDLE_REASON, &self.settings, true);
        Ok(())
    }

    /// Reserve a cold placement, waiting for an idle eviction only when an
    /// otherwise eligible machine has spent its small-VM reservations.
    /// The registry lock covers selection. Checkpoint I/O runs outside it.
    pub(crate) async fn place_space_vm(
        self: &Arc<Self>,
        account: AccountId,
        worker: PluginWorker,
    ) -> Result<HeldPlacement, Error> {
        let thread = Uuid::new_v4();
        let runtime = tokio::runtime::Handle::current();
        let result = async {
            loop {
                let service = self.clone();
                let worker = worker.clone();
                let runtime = runtime.clone();
                let (run, eviction) = blocking(move || {
                    let space = worker.space_id;
                    if service
                        .lock()?
                        .space_channels
                        .get(&space)
                        .is_some_and(|slot| !*slot.closing.borrow())
                    {
                        return Err(no_assignment());
                    }
                    match service.request_space_vm(account, thread, worker) {
                        Ok(run) => Ok((
                            Some(HeldPlacement {
                                service,
                                account,
                                lease: run.lease_id,
                                runtime,
                                settled: false,
                            }),
                            None,
                        )),
                        Err(error)
                            if error
                                .details
                                .as_ref()
                                .and_then(|value| value["reason"].as_str())
                                == Some("worker_capacity") =>
                        {
                            let eviction = service.lock()?.evict_small_vm(
                                account,
                                space,
                                &service.settings,
                            )?;
                            if eviction.is_none() {
                                return Err(error);
                            }
                            Ok((None, eviction))
                        }
                        Err(error) => Err(error),
                    }
                })
                .await?;
                if let Some(run) = run {
                    return Ok(run);
                }
                match eviction.expect("pressure requires a pending eviction") {
                    Eviction::Stopping(lease) => self.clone().wait_worker_stopped(lease).await?,
                    Eviction::Carrier(mut drained) => {
                        drained
                            .wait_for(|done| *done)
                            .await
                            .map_err(|_| no_assignment())?;
                    }
                }
            }
        };
        tokio::time::timeout(crate::plugins::worker::WORKER_START_TIMEOUT, result)
            .await
            .map_err(|_| Error::unavailable("small plugin VM is still sleeping; retry shortly"))?
    }

    /// Reserve the v2 carrier under the scheduler lock on the blocking worker.
    /// Read the restore proof before queueing the assignment so a concurrent
    /// erasure cannot authorize an older copy. No frame takes this lock.
    pub(crate) async fn open_space_channel(
        self: &Arc<Self>,
        account: AccountId,
        lease_id: Uuid,
        package: PackagePin,
    ) -> Result<Connected, Error> {
        let service = self.clone();
        let receive = blocking(move || {
            let mut st = service.lock()?;
            let lease = st
                .leases
                .get(&lease_id)
                .filter(|lease| lease.row.worker && lease.row.account == account)
                .ok_or_else(no_assignment)?;
            let machine = lease.assigned_machine().ok_or_else(no_assignment)?;
            let binding = SpaceChannelAssignment {
                space: lease.row.space,
                lease_id,
                fence: lease.row.fence,
                assignment: Uuid::new_v4(),
                package,
                checkpoint: super::checkpoint::proof(&st.db, lease.row.space, machine)?,
            };
            let (session, idle_seconds) = st
                .machines
                .get(&machine)
                .and_then(|record| record.live.as_ref())
                .and_then(|live| Some((live.session, live.capacity.small_vm?.idle_seconds)))
                .ok_or_else(no_assignment)?;
            st.require_channel_capacity(binding.space)?;
            let (reply, receive) = oneshot::channel();
            st.space_channels.insert(
                binding.space,
                Reservation {
                    binding: binding.clone(),
                    machine,
                    account,
                    session,
                    closing: watch::channel(false).0,
                    restore_consumed: watch::channel(binding.checkpoint.is_none()).0,
                    residency: Arc::new(Residency::new(Duration::from_secs(u64::from(
                        idle_seconds,
                    )))),
                    reply: Some(reply),
                },
            );
            st.push(machine, Down::OpenSpaceChannel { channel: binding });
            Ok(receive)
        })
        .await?;
        tokio::time::timeout(OPEN_LIMIT, receive)
            .await
            .map_err(|_| Error::unavailable("space carrier did not connect"))?
            .map_err(|_| no_assignment())
    }

    /// Consume the one-time reservation on cold connect. The scheduler lock
    /// binds the authenticated machine to the current lease and assignment
    /// before taking the waiter. It does not cover network I/O or frame traffic.
    pub(super) fn claim_channel(
        &self,
        token: &str,
        space: SpaceId,
        assignment: Uuid,
    ) -> Result<
        (
            Uuid,
            SpaceChannelAssignment,
            watch::Sender<bool>,
            watch::Receiver<bool>,
            Arc<Residency>,
            oneshot::Sender<Connected>,
        ),
        Error,
    > {
        let machine = self.authenticate(token).map_err(|error| match error.code {
            ErrorCode::Unauthorized | ErrorCode::Forbidden | ErrorCode::NotFound => no_assignment(),
            _ => error,
        })?;
        let session = calternal_auth::token::hash32(token);
        let mut st = self.lock()?;
        let reservation = st
            .space_channels
            .get(&space)
            .filter(|slot| {
                slot.machine == machine
                    && slot.session == session
                    && slot.binding.assignment == assignment
                    && !*slot.closing.borrow()
            })
            .ok_or_else(no_assignment)?;
        let lease = st
            .leases
            .get(&reservation.binding.lease_id)
            .filter(|lease| {
                lease.assigned_machine() == Some(machine)
                    && lease.row.fence == reservation.binding.fence
                    && lease.row.space == space
            })
            .ok_or_else(no_assignment)?;
        let _steps = st.lease_steps(&lease.row)?;
        let reservation = st
            .space_channels
            .get_mut(&space)
            .ok_or_else(no_assignment)?;
        let reply = reservation
            .reply
            .take()
            .filter(|reply| !reply.is_closed())
            .ok_or_else(no_assignment)?;
        Ok((
            machine,
            reservation.binding.clone(),
            reservation.closing.clone(),
            reservation.restore_consumed.subscribe(),
            reservation.residency.clone(),
            reply,
        ))
    }
}

fn no_assignment() -> Error {
    Error::not_found("space carrier assignment unavailable")
}

pub(super) async fn upgrade<D: Directory>(
    svc: http::Svc,
    HttpState(state): HttpState<Shared<D>>,
    Path((space, assignment)): Path<(SpaceId, Uuid)>,
    mut request: Request,
) -> Response {
    let accepted = async {
        if request.version() != axum::http::Version::HTTP_11
            || request
                .headers()
                .get(header::UPGRADE)
                .is_none_or(|value| value != SPACE_CHANNEL_UPGRADE)
            || !request
                .headers()
                .get_all(header::CONNECTION)
                .iter()
                .any(|line| {
                    line.to_str().is_ok_and(|value| {
                        value
                            .split(',')
                            .any(|value| value.trim().eq_ignore_ascii_case("upgrade"))
                    })
                })
        {
            return Err(Error::invalid_input(
                "space carrier requires an HTTP upgrade",
            ));
        }
        let token = http::bearer(request.headers()).map_err(|_| no_assignment())?;
        let service = http::service(svc)?;
        let authority = state
            .directory
            .protected_spaces()
            .cloned()
            .ok_or_else(no_assignment)?;
        let (binding, grants, closing, restore_consumed, residency, reply) = blocking(move || {
            let (machine, binding, closing, restore_consumed, residency, reply) =
                service.claim_channel(&token, space, assignment)?;
            let grants = match authority.grant_revision(space, machine) {
                Ok(grants) => grants,
                Err(error) => {
                    closing.send_replace(true);
                    return Err(super::authority_error(error));
                }
            };
            Ok((binding, grants, closing, restore_consumed, residency, reply))
        })
        .await?;
        let upgraded = hyper::upgrade::on(&mut request);
        tokio::spawn(async move {
            let result = async {
                let mut stopped = closing.subscribe();
                let upgraded = tokio::time::timeout(OPEN_LIMIT, upgraded)
                    .await
                    .ok()?
                    .ok()?;
                let host = SpaceChannel::host(
                    hyper_util::rt::TokioIo::new(upgraded),
                    space,
                    assignment,
                    vec![binding.package],
                );
                let channel = tokio::select! {
                    biased;
                    _ = stopped.wait_for(|closed| *closed) => return None,
                    channel = host => channel.ok()?,
                };
                let mut life = channel.lifetime();
                let fence = channel.fence_handle();
                reply
                    .send(Connected {
                        space,
                        channel,
                        grants,
                        restore_consumed,
                        residency,
                    })
                    .ok()?;
                tokio::select! {
                    _ = stopped.wait_for(|closed| *closed) => fence.close(),
                    _ = life.wait_for(|live| !*live) => {}
                }
                Some(())
            }
            .await;
            let _ = result;
            closing.send_replace(true);
        });
        Ok::<_, Error>(
            (
                StatusCode::SWITCHING_PROTOCOLS,
                [
                    (header::CONNECTION, "Upgrade"),
                    (header::UPGRADE, SPACE_CHANNEL_UPGRADE),
                ],
            )
                .into_response(),
        )
    }
    .await;
    accepted.unwrap_or_else(|error| failure(None, error))
}
