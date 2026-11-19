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
use calternal_machine::protocol::{Down, SPACE_CHANNEL_UPGRADE, SpaceChannelAssignment};
use calternal_plugin_host::space_channel::{PackagePin, SpaceChannel};
use calternal_types::{AccountId, Error, SpaceId};
use std::{sync::Arc, time::Duration};
use tokio::sync::{oneshot, watch};
use uuid::Uuid;

const MAX_CHANNELS: usize = 1024;
const OPEN_LIMIT: Duration = Duration::from_secs(5);

pub(crate) struct Connected {
    pub(crate) space: SpaceId,
    pub(crate) channel: SpaceChannel,
    pub(crate) grants: GrantRevision,
}

pub(super) struct Reservation {
    binding: SpaceChannelAssignment,
    machine: Uuid,
    account: AccountId,
    session: [u8; 32],
    closing: watch::Sender<bool>,
    reply: Option<oneshot::Sender<Connected>>,
}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.closing.send_replace(true);
    }
}

impl State {
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
    /// Reserve the v2 data channel on an enrolled worker placement. Uses two
    /// control-plane memory lookups plus an outbox message. No frame uses this lock.
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
            };
            let session = st
                .machines
                .get(&machine)
                .and_then(|record| record.live.as_ref())
                .map(|live| live.session)
                .ok_or_else(no_assignment)?;
            st.space_channels.retain(|_, reservation| {
                !*reservation.closing.borrow()
                    && reservation
                        .reply
                        .as_ref()
                        .is_none_or(|reply| !reply.is_closed())
            });
            if st.space_channels.contains_key(&binding.space)
                || st.space_channels.len() >= MAX_CHANNELS
            {
                return Err(Error::unavailable(
                    "space already has a carrier or carrier capacity is full",
                ));
            }
            let (reply, receive) = oneshot::channel();
            st.space_channels.insert(
                binding.space,
                Reservation {
                    binding: binding.clone(),
                    machine,
                    account,
                    session,
                    closing: watch::channel(false).0,
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
            oneshot::Sender<Connected>,
        ),
        Error,
    > {
        let machine = self.authenticate(token)?;
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
        let token = http::bearer(request.headers())?;
        let service = http::service(svc)?;
        let authority = state
            .directory
            .protected_spaces()
            .cloned()
            .ok_or_else(no_assignment)?;
        let (binding, grants, closing, reply) = blocking(move || {
            let (machine, binding, closing, reply) =
                service.claim_channel(&token, space, assignment)?;
            let grants = match authority.grant_revision(space, machine) {
                Ok(grants) => grants,
                Err(error) => {
                    closing.send_replace(true);
                    return Err(super::authority_error(error));
                }
            };
            Ok((binding, grants, closing, reply))
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
