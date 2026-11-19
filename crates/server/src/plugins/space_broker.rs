//! Host admission, authorization and item-head delivery over the v2 carrier.
use super::{
    Session,
    space_ticket::{Scope, Ticket},
};
use crate::{
    api::{Shared, authorization},
    auth::{Directory, Identity},
    machines::{MachineService, space_channel::Connected},
    store::sha256_hex,
};
use calternal_auth::spaces::Spaces;
use calternal_content::{ItemHead, ItemHeadReader};
use calternal_machine::{protocol::PluginWorker, sandbox::worker::MAX_SOURCE_BYTES};
use calternal_plugin_host::space_channel::{PackagePin, SpaceMessage, SpaceMethod, SpaceSender};
use calternal_types::{AccountId, Capability, Error, ErrorCode, SpaceId};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// An operator-approved package read operation for one space. Construct from a
/// verified installation declaration on a blocking setup thread. This narrow
/// profile admits one declared item kind and one read-only broker method.
pub struct ApprovedItemHead {
    pub(super) space: SpaceId,
    pub(super) operation: String,
    package: PackagePin,
    kind: String,
    worker_source: String,
    wake: Arc<Wake>,
    #[cfg(test)]
    container_fixture: bool,
}

impl std::fmt::Debug for ApprovedItemHead {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ApprovedItemHead")
            .field("space", &self.space)
            .field("operation", &self.operation)
            .field("package", &self.package)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl ApprovedItemHead {
    /// Bind a validated v2 declaration and its verified worker source. The
    /// caller owns the space's approval decision. This checks the bounded
    /// source and pinned digest once before the server may dispatch the op.
    pub fn new(
        space: SpaceId,
        pin: PackagePin,
        operation: String,
        kind: String,
        worker_source: String,
    ) -> Result<Self, Error> {
        let valid_id = |part: &str| {
            part.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
                && part.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_')
                })
        };
        if !valid_id(&pin.package)
            || pin.package.len() > 64
            || !valid_id(&kind)
            || kind.len() > 64
            || !operation.contains('.')
            || operation.len() > 128
            || !operation.split('.').all(valid_id)
            || worker_source.is_empty()
            || worker_source.len() > MAX_SOURCE_BYTES
            || sha256_hex(worker_source.as_bytes()) != pin.digest
        {
            return Err(Error::invalid_input(
                "invalid pinned item-head package declaration",
            ));
        }
        Ok(Self {
            space,
            operation,
            package: pin,
            kind,
            worker_source,
            wake: Arc::default(),
            #[cfg(test)]
            container_fixture: false,
        })
    }
}

mod awake;
use awake::{Awake, Wake};

impl ApprovedItemHead {
    pub(super) fn share_wake(&mut self, first: &Self) -> Result<(), Error> {
        if self.package != first.package {
            return Err(Error::conflict(
                "the item-head profile admits one package per space",
            ));
        }
        self.wake = first.wake.clone();
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemHeadInput {
    pub(crate) space_id: SpaceId,
    pub(crate) item_id: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemTarget<'a> {
    kind: &'a str,
    item_id: Uuid,
}

fn item_target_matches(kind: &str, id: Uuid, input: &Value) -> bool {
    ItemTarget::deserialize(input).is_ok_and(|target| target.kind == kind && target.item_id == id)
}

struct HeldPlacement {
    service: Arc<MachineService>,
    account: AccountId,
    lease: Uuid,
    runtime: tokio::runtime::Handle,
}

impl Drop for HeldPlacement {
    fn drop(&mut self) {
        let (service, account, lease) = (self.service.clone(), self.account, self.lease);
        self.runtime.spawn(async move {
            let _ = crate::blocking::spawn(move || service.stop_run(account, lease)).await;
        });
    }
}

pub(crate) async fn invoke<D: Directory>(
    state: Shared<D>,
    actor: Identity,
    session: Session,
    approval: Arc<ApprovedItemHead>,
    input: ItemHeadInput,
    machines: Option<Arc<MachineService>>,
) -> Result<Value, Error> {
    if input.space_id != approval.space || input.item_id.is_nil() {
        return Err(missing());
    }
    let installation = match &session {
        Session::Static => None,
        Session::Installation {
            account,
            installation,
        } if *account == actor.id => Some(*installation),
        _ => return Err(Error::forbidden("item head requires a person session")),
    };
    // Only cold wake enters the authority worker for preflight or the machine
    // registry for placement. Warm invocations use the pinned space reader.
    let mut wake = approval.wake.awake.lock().await;
    let awake = match wake.as_ref().filter(|awake| awake.sender.is_live()) {
        Some(awake) => awake.clone(),
        None => {
            if !session.is_live(&state.directory).await
                || authorization::capabilities(&state, actor.id, approval.space)
                    .await?
                    .is_none_or(|rights| {
                        !rights.contains(Capability::PluginsUse)
                            || !rights.contains(Capability::SpaceRead)
                    })
            {
                return Err(missing());
            }
            let service = machines
                .or_else(|| state.store.machines())
                .ok_or_else(unavailable)?;
            let awake = wake_space(&state, &approval, actor.id, service).await?;
            *wake = Some(awake.clone());
            awake
        }
    };
    drop(wake);
    awake
        .invoke(Scope {
            actor: actor.id,
            installation,
            package: approval.package.clone(),
            epoch: awake.epoch,
            operation: approval.operation.clone(),
            kind: approval.kind.clone(),
            item: input.item_id,
        })
        .await
}

/// Cold control plane. The caller holds this space's wake gate until the
/// reservation and package are ready so concurrent calls share the placement.
/// The authority worker verifies enrollment once when binding the new carrier.
/// The shared scheduler is used for placement and teardown only.
async fn wake_space<D: Directory>(
    state: &Shared<D>,
    approval: &Arc<ApprovedItemHead>,
    account: AccountId,
    service: Arc<MachineService>,
) -> Result<Arc<Awake>, Error> {
    let scheduler = service.clone();
    let pinned = approval.clone();
    let runtime = tokio::runtime::Handle::current();
    let placement = crate::blocking::spawn(move || {
        scheduler.check_space_channel_available(pinned.space)?;
        let place = MachineService::request_space_vm;
        #[cfg(test)]
        let place = if pinned.container_fixture {
            MachineService::request_worker
        } else {
            place
        };
        let run = place(
            &scheduler,
            account,
            Uuid::new_v4(),
            PluginWorker {
                space_id: pinned.space,
                source: pinned.worker_source.clone(),
            },
        )?;
        Ok::<_, Error>(HeldPlacement {
            service: scheduler,
            account,
            lease: run.lease_id,
            runtime,
        })
    })
    .await
    .map_err(|_| unavailable())??;
    let connected = service
        .open_space_channel(account, placement.lease, approval.package.clone())
        .await?;
    let content = state.store.shared_content();
    let space = approval.space;
    let reader = crate::blocking::spawn(move || content.item_head_reader(space))
        .await
        .map_err(|_| unavailable())??;
    let authority = state
        .directory
        .protected_spaces()
        .cloned()
        .ok_or_else(unavailable)?;
    Awake::attach(
        connected,
        authority,
        Arc::new(reader),
        &approval.package,
        Some((Arc::downgrade(&approval.wake), placement)),
        awake::IDLE,
    )
    .await
}

#[cfg(test)]
async fn exchange(
    connected: Connected,
    authority: Spaces,
    reader: Arc<ItemHeadReader>,
    scope: Scope,
) -> Result<Value, Error> {
    let awake = Awake::attach(
        connected,
        authority,
        reader,
        &scope.package,
        None,
        awake::IDLE,
    )
    .await?;
    let result = awake.invoke(scope).await;
    awake.fence.close();
    result
}

async fn run(
    channel: &SpaceSender,
    inbox: &mut awake::Inbox,
    reader: Arc<ItemHeadReader>,
    ticket: Arc<Ticket>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Result<Value, Error> {
    let pin = &ticket.scope.package;
    let send_error = |_| {
        ticket
            .require_live(Instant::now())
            .err()
            .unwrap_or_else(unavailable)
    };
    let pending = ticket.clone();
    let canceled = cancel.clone();
    channel
        .send_checked_wait(
            pin,
            ticket.id(),
            SpaceMessage::Invoke {
                operation: ticket.scope.operation.clone(),
                input: json!({"kind":ticket.scope.kind,"item_id":ticket.scope.item}),
                deadline_ms: ticket
                    .deadline()
                    .saturating_duration_since(Instant::now())
                    .as_millis()
                    .clamp(1, 60_000) as u32,
            },
            move || {
                !canceled.load(std::sync::atomic::Ordering::Acquire)
                    && pending.require_live(Instant::now()).is_ok()
            },
        )
        .await
        .map_err(send_error)?;
    let mut answered: Option<Value> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let delivery = tokio::select! {
            _ = tick.tick() => {
                ticket.require_live(Instant::now())?;
                continue;
            }
            item = inbox.recv() => item.ok_or_else(unavailable)?,
        };
        let frame = delivery.frame();
        ticket.authorize_at(frame, Instant::now())?;
        match &frame.message {
            SpaceMessage::Call {
                callback_id,
                method: SpaceMethod::ItemHead,
                input,
            } if answered.is_none() => {
                if !item_target_matches(&ticket.scope.kind, ticket.scope.item, input) {
                    return Err(missing());
                }
                let (callback_id, read, scoped) = (*callback_id, reader.clone(), ticket.clone());
                drop(delivery);
                let head: ItemHead = tokio::task::spawn_blocking(move || {
                    scoped.require_live(Instant::now())?;
                    read.read(&scoped.scope.kind, scoped.scope.item)
                })
                .await
                .map_err(|_| unavailable())??;
                ticket.require_live(Instant::now())?;
                let result = serde_json::to_value(head).map_err(|_| unavailable())?;
                answered = Some(result.clone());
                let pending = ticket.clone();
                let canceled = cancel.clone();
                channel
                    .send_checked_wait(
                        pin,
                        ticket.id(),
                        SpaceMessage::Reply {
                            callback_id,
                            result: Ok(result),
                        },
                        move || {
                            !canceled.load(std::sync::atomic::Ordering::Acquire)
                                && pending.require_live(Instant::now()).is_ok()
                        },
                    )
                    .await
                    .map_err(send_error)?;
            }
            SpaceMessage::Complete { result: Ok(result) } if answered.as_ref() == Some(result) => {
                ticket.require_live(Instant::now())?;
                if !channel.is_live() {
                    return Err(unavailable());
                }
                let value = answered.take().ok_or_else(unavailable)?;
                return Ok(value);
            }
            SpaceMessage::Complete { result: Err(_) } => return Err(unavailable()),
            _ => {
                return Err(Error::new(
                    ErrorCode::Forbidden,
                    "plugin response is outside the admitted item-head operation",
                ));
            }
        }
    }
}

fn unavailable() -> Error {
    Error::unavailable("enrolled item-head worker unavailable")
}

fn missing() -> Error {
    Error::not_found("item not found")
}

#[cfg(test)]
mod tests;
