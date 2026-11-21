//! Lifetime of one awake placement and demultiplexing of its bounded tickets.
use super::*;
use crate::machines::space_channel::{Invocation, Residency};
use calternal_auth::spaces::InvocationReader;
use calternal_plugin_host::space_channel::{SpaceDelivery, SpaceFence};
use std::{
    collections::HashMap,
    sync::{
        Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{mpsc, watch};

pub(super) type Inbox = mpsc::Receiver<SpaceDelivery>;

#[derive(Default)]
pub(super) struct Wake {
    pub(super) awake: tokio::sync::Mutex<Option<Arc<Awake>>>,
}

pub(super) struct Awake {
    pub(super) sender: SpaceSender,
    pub(super) fence: SpaceFence,
    pub(super) epoch: Uuid,
    admission: Arc<InvocationReader>,
    reader: Arc<ItemHeadReader>,
    residency: Arc<Residency>,
    inboxes: Mutex<HashMap<Uuid, mpsc::Sender<SpaceDelivery>>>,
}

impl Drop for Awake {
    fn drop(&mut self) {
        self.fence.close();
    }
}

impl Awake {
    pub(super) async fn attach(
        mut connected: Connected,
        authority: Spaces,
        reader: Arc<ItemHeadReader>,
        pin: &PackagePin,
        placement: Option<(Weak<Wake>, HeldPlacement)>,
    ) -> Result<Arc<Self>, Error> {
        if connected.grants.space() != connected.space {
            return Err(unavailable());
        }
        let ready = tokio::time::timeout(crate::plugins::worker::WORKER_START_TIMEOUT, async {
            let ready = connected.channel.receive().await?;
            connected
                .restore_consumed
                .wait_for(|consumed| *consumed)
                .await
                .ok()?;
            Some(ready)
        })
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
        let frame = ready.frame();
        if !matches!(&frame.message, SpaceMessage::Ready {})
            || frame.package.as_deref() != Some(pin.package.as_str())
            || frame.digest.as_deref() != Some(pin.digest.as_str())
        {
            return Err(unavailable());
        }
        drop(ready);

        let grants = connected.grants;
        let admission = crate::blocking::spawn(move || authority.invocation_reader(&grants))
            .await
            .map_err(|_| unavailable())?
            .map_err(|_| unavailable())?;
        connected.residency.start();
        let residency = connected.residency.clone();
        let awake = Arc::new(Self {
            sender: connected.channel.sender(),
            fence: connected.channel.fence_handle(),
            epoch: connected.channel.epoch(),
            admission: Arc::new(admission),
            reader,
            residency: residency.clone(),
            inboxes: Mutex::default(),
        });
        let owner = Arc::downgrade(&awake);
        let epoch = awake.epoch;
        tokio::spawn(async move {
            let mut drain = false;
            let mut stop = false;
            loop {
                let delivery = tokio::select! {
                    _ = residency.wait_for_sleep() => {
                        if placement.is_some() {
                            drain = connected.channel.request_remote_sleep().is_ok();
                        }
                        if !drain {
                            stop = connected.channel.request_remote_stop().is_ok();
                        }
                        break;
                    }
                    delivery = connected.channel.receive() => {
                        let Some(delivery) = delivery else { break };
                        delivery
                    }
                };
                let Some(awake) = owner.upgrade() else { break };
                let Ok(mut inboxes) = awake.inboxes.lock() else {
                    break;
                };
                let Some(id) = delivery.frame().ticket else {
                    break;
                };
                if let Some(inbox) = inboxes.get(&id)
                    && inbox.try_send(delivery).is_err()
                {
                    inboxes.remove(&id);
                }
            }
            if drain || stop {
                let mut life = connected.channel.lifetime();
                let _ =
                    tokio::time::timeout(Duration::from_millis(500), life.wait_for(|live| !*live))
                        .await;
            }
            // Close the socket while the placement guard is still held.
            connected.channel.fence();
            if let Some(awake) = owner.upgrade()
                && let Ok(mut inboxes) = awake.inboxes.lock()
            {
                inboxes.clear();
            }
            if let Some((wake, mut placement)) = placement {
                if drain {
                    // The next wake joins this space's drain until the machine
                    // confirms that the old VMM was reaped.
                    let _ = placement.suspend().await;
                }
                drop(placement);
                if let Some(wake) = wake.upgrade() {
                    let mut current = wake.awake.lock().await;
                    if current.as_ref().is_some_and(|awake| awake.epoch == epoch) {
                        current.take();
                    }
                }
            }
            residency.finish();
        });
        Ok(awake)
    }

    pub(super) fn drain_waiter(&self) -> Option<watch::Receiver<bool>> {
        self.residency.drain_waiter()
    }

    pub(super) fn admit(&self) -> Result<Option<Invocation>, Error> {
        self.residency.admit()
    }

    #[cfg(test)]
    pub(super) fn has_active_ticket(&self) -> bool {
        !self.inboxes.lock().unwrap().is_empty()
    }

    #[cfg(test)]
    pub(super) async fn invoke(&self, scope: Scope) -> Result<Value, Error> {
        let usage = self.admit()?.ok_or_else(unavailable)?;
        self.invoke_reserved(scope, usage).await
    }

    pub(super) async fn invoke_reserved(
        &self,
        scope: Scope,
        mut usage: Invocation,
    ) -> Result<Value, Error> {
        if scope.epoch != self.epoch || !self.sender.is_live() {
            return Err(unavailable());
        }
        let admission = self.admission.clone();
        let ticket = Arc::new(
            crate::blocking::spawn(move || Ticket::mint(&admission, scope))
                .await
                .map_err(|_| unavailable())??,
        );
        let (send, mut inbox) = mpsc::channel(2);
        {
            let mut inboxes = self.inboxes.lock().map_err(|_| unavailable())?;
            if !self.sender.is_live() {
                return Err(unavailable());
            }
            inboxes.insert(ticket.id(), send);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let mut active = Active {
            awake: self,
            ticket: &ticket,
            cancel: cancel.clone(),
            completed: false,
        };
        let deadline = tokio::time::Instant::from_std(ticket.deadline());
        usage.mark_used();
        let work = super::run(
            &self.sender,
            &mut inbox,
            self.reader.clone(),
            ticket.clone(),
            cancel,
        );
        let result = tokio::time::timeout_at(deadline, work)
            .await
            .map_err(|_| unavailable())?;
        active.completed = result.is_ok();
        result
    }
}

struct Active<'a> {
    awake: &'a Awake,
    ticket: &'a Ticket,
    cancel: Arc<AtomicBool>,
    completed: bool,
}

impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        if let Ok(mut inboxes) = self.awake.inboxes.lock() {
            inboxes.remove(&self.ticket.id());
        }
        if !self.completed {
            let _ = self.awake.sender.send(
                &self.ticket.scope.package,
                self.ticket.id(),
                SpaceMessage::Cancel {},
            );
        }
    }
}
