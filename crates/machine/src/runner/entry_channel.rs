//! Keep one bounded output stream per exact running entry across carrier loss.
use super::entry_broker::BrokerRelay;
use super::entry_tail::EntryTail;
use super::*;
use crate::sandbox::{
    EntryLaunch, EntryLog,
    entry_log::{read_entry, record_entry},
};
use calternal_plugin_host::Error as ProtocolError;
use calternal_plugin_host::{
    entry::EntryProgram,
    space_channel::{PackagePin, SpaceChannel, SpaceDelivery, SpaceMessage},
};
use calternal_types::{ErrorCode, SpaceId};
use tokio::sync::{mpsc, watch};

// The owner allows 24 hours of continuous absence. Keep recorded output
// after stopping the process, until the host commits the tail.
pub(super) const OUTAGE_LIMIT: Duration = Duration::from_secs(24 * 60 * 60);

/// Reach the same entry actor on a replacement carrier while the machine lives.
pub(super) struct EntrySlot {
    origin: Uuid,
    space: SpaceId,
    package: PackagePin,
    attach: mpsc::Sender<Attachment>,
}

/// One authenticated carrier bound to an entry, with its relay cursor.
pub(super) struct Attachment {
    channel: SpaceChannel,
    session: watch::Receiver<Option<Arc<str>>>,
    life: watch::Receiver<bool>,
    cursor: u64,
    offset: u64,
    origin: Uuid,
}

impl Attachment {
    fn new(
        channel: SpaceChannel,
        session: watch::Receiver<Option<Arc<str>>>,
        origin: Uuid,
    ) -> Self {
        let life = channel.lifetime();
        Self {
            channel,
            session,
            life,
            cursor: 0,
            offset: 0,
            origin,
        }
    }

    async fn receive(&mut self) -> Option<SpaceDelivery> {
        tokio::select! {
            biased;
            _ = self.session.changed() => None,
            _ = self.life.wait_for(|live| !*live) => None,
            delivery = self.channel.receive() => delivery,
        }
    }
}

impl Runner {
    /// Bind a one-use carrier to a new launch or the same detached runtime.
    /// An exact origin is needed for attachment, which cannot launch code.
    pub(super) async fn open_entry_channel(
        &self,
        space: SpaceId,
        assignment: Uuid,
        package: PackagePin,
        ticket: Uuid,
    ) {
        if ticket.is_nil() {
            return;
        }
        let Ok((io, session)) = self.link.upgrade_space(space, assignment).await else {
            return;
        };
        let Ok(channel) = SpaceChannel::runtime(io, space, assignment).await else {
            return;
        };
        if channel.ready(&package).is_err() {
            return;
        }
        let mut link = Attachment::new(channel, session, assignment);
        let Some(delivery) = link.receive().await else {
            return;
        };
        let frame = delivery.frame();
        if frame.ticket != Some(ticket)
            || frame.package.as_deref() != Some(&package.package)
            || frame.digest.as_deref() != Some(&package.digest)
        {
            return;
        }
        match &frame.message {
            SpaceMessage::ResumeEntry { origin, after } => {
                let attach = self
                    .entries
                    .lock()
                    .expect("entries")
                    .get(&ticket)
                    .filter(|slot| {
                        slot.origin == *origin && slot.space == space && slot.package == package
                    })
                    .map(|slot| slot.attach.clone());
                if let Some(attach) = attach {
                    link.cursor = *after;
                    link.origin = *origin;
                    let _ = attach.try_send(link);
                }
            }
            SpaceMessage::StartEntry { program } => {
                let Ok(_capacity) = self.entry_capacity.try_acquire() else {
                    return;
                };
                let (attach, replacements) = mpsc::channel(1);
                let slot = EntrySlot {
                    origin: assignment,
                    space,
                    package: package.clone(),
                    attach,
                };
                {
                    let mut slots = self.entries.lock().expect("entries");
                    let std::collections::hash_map::Entry::Vacant(entry) = slots.entry(ticket)
                    else {
                        return;
                    };
                    entry.insert(slot);
                }
                self.run_entry(&package, ticket, program, link, replacements)
                    .await;
                self.entries.lock().expect("entries").remove(&ticket);
            }
            _ => {}
        }
    }

    async fn run_entry(
        &self,
        package: &PackagePin,
        ticket: Uuid,
        program: &EntryProgram,
        mut link: Attachment,
        replacements: mpsc::Receiver<Attachment>,
    ) {
        let fence = link.channel.sender();
        let launch_fence = fence.clone();
        let launch =
            self.backend
                .start_entry(ticket, program, Arc::new(move || launch_fence.is_live()));
        tokio::pin!(launch);
        let (started, attached) = tokio::select! {
            biased;
            stop = launch_interrupted(&mut link, ticket) => {
                link.channel.fence();
                // Join a create that may already have started the process.
                // Transport loss retains a started sandbox. Stop retires it.
                let started = launch.await;
                if stop {
                    if let Ok(started) = started {
                        let _ = started.sandbox.stop().await;
                        let _ = tokio::fs::remove_file(started.log.path).await;
                    }
                    return;
                }
                (started, false)
            }
            started = &mut launch => (started, true),
        };
        let Ok(EntryLaunch {
            sandbox,
            exec,
            log:
                EntryLog {
                    path,
                    reader,
                    writer,
                },
            broker,
            egress,
        }) = started
        else {
            let _ = fence
                .send_wait(
                    package,
                    ticket,
                    SpaceMessage::Complete {
                        result: Err(ErrorCode::Unavailable),
                    },
                )
                .await;
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                launch_interrupted(&mut link, ticket),
            )
            .await;
            return;
        };
        let (publish, published) = watch::channel(0);
        let mut run = Run {
            package,
            ticket,
            reader,
            producer: tokio::spawn(record_entry(exec, writer, publish)),
            published,
            sandbox: Some(sandbox),
            broker: broker.map(|listener| BrokerRelay::start(listener, program.methods.clone())),
        };
        let deadline = tokio::time::sleep(Duration::from_secs(program.run_limit_secs.into()));
        tokio::pin!(deadline);
        let preserve_log =
            drive_entry(&mut run, attached.then_some(link), replacements, deadline).await;
        // Fence callbacks and egress before teardown so no effect or reply
        // follows the end of authority.
        drop(run.broker.take());
        drop(egress);
        if let Some(sandbox) = run.sandbox.take() {
            let _ = sandbox.stop().await;
        }
        if !run.producer.is_finished() {
            run.producer.abort();
            let _ = (&mut run.producer).await;
        }
        if !preserve_log {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

/// One running entry's machine-side state, owned by the carrier driver.
pub(super) struct Run<'a> {
    pub(super) package: &'a PackagePin,
    pub(super) ticket: Uuid,
    pub(super) reader: Arc<std::fs::File>,
    pub(super) producer: tokio::task::JoinHandle<Result<(), ProtocolError>>,
    pub(super) published: watch::Receiver<u64>,
    pub(super) sandbox: Option<Arc<dyn Sandbox>>,
    pub(super) broker: Option<BrokerRelay>,
}

async fn launch_interrupted(link: &mut Attachment, ticket: Uuid) -> bool {
    while let Some(delivery) = link.receive().await {
        if delivery.frame().ticket == Some(ticket) {
            return matches!(delivery.frame().message, SpaceMessage::Cancel {});
        }
    }
    false
}

enum Outbound {
    Output { seq: u64, end: u64 },
    Call(Uuid),
}

// Keep the log if local I/O or teardown fails, or if Stop arrives after
// the watchdog has already ended the run. Broker calls go before output so
// a callback waiting on the host is never queued behind a long backlog.
pub(super) async fn drive_entry(
    run: &mut Run<'_>,
    mut link: Option<Attachment>,
    mut replacements: mpsc::Receiver<Attachment>,
    mut deadline: std::pin::Pin<&mut tokio::time::Sleep>,
) -> bool {
    let (package, ticket) = (run.package, run.ticket);
    let mut tail = EntryTail::default();
    let mut was_live = link.is_some();
    let mut producer_done = false;
    let mut outage_expired = false;
    let mut read = None;
    let expiry = tokio::time::sleep(OUTAGE_LIMIT);
    tokio::pin!(expiry);
    loop {
        let through = *run.published.borrow_and_update();
        if producer_done && tail.acked() == through {
            return false;
        }
        if was_live && link.is_none() {
            expiry
                .as_mut()
                .reset(tokio::time::Instant::now() + OUTAGE_LIMIT);
        }
        was_live = link.is_some();
        let watch_expiry = !was_live && !outage_expired;
        let call = link
            .as_ref()
            .and(run.broker.as_ref())
            .and_then(BrokerRelay::next_unsent);
        let outbound = if let Some((id, message)) = call {
            Some((
                Outbound::Call(id),
                message,
                link.as_ref().expect("live").channel.sender(),
            ))
        } else if let Some(link) = link.as_ref()
            && link.cursor < through
            && tail.can_send(link.cursor)
        {
            if read
                .as_ref()
                .is_none_or(|(offset, _, _)| *offset != link.offset)
            {
                let Ok((end, message)) = read_entry(run.reader.clone(), link.offset).await else {
                    return true;
                };
                read = Some((link.offset, end, message));
            }
            let (_, end, message) = read.as_ref().expect("read at relay cursor");
            if matches!(message, SpaceMessage::Complete { .. }) && !producer_done {
                None
            } else {
                Some((
                    Outbound::Output {
                        seq: link.cursor + 1,
                        end: *end,
                    },
                    match message {
                        SpaceMessage::Chunk { data } => SpaceMessage::Chunk { data: data.clone() },
                        SpaceMessage::Complete { result } => SpaceMessage::Complete {
                            result: result.clone(),
                        },
                        _ => unreachable!("entry record"),
                    },
                    link.channel.sender(),
                ))
            }
        } else {
            None
        };
        let control = async {
            match link.as_mut() {
                Some(link) => link.receive().await,
                None => std::future::pending().await,
            }
        };
        let send = async {
            let Some((what, msg, sender)) = outbound else {
                return std::future::pending().await;
            };
            sender.send_wait(package, ticket, msg).await.map(|()| what)
        };
        let entry_call = async {
            match run.broker.as_mut() {
                Some(broker) => broker.next_call().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            biased;
            _ = &mut expiry, if watch_expiry => {
                outage_expired = true;
                if let Some(runtime) = run.sandbox.take()
                    && runtime.stop().await.is_err() { return true; }
            }
            _ = &mut deadline, if run.sandbox.is_some() => {
                let runtime = run.sandbox.take().expect("active entry");
                if runtime.stop().await.is_err() { return true; }
            }
            result = &mut run.producer, if !producer_done => {
                if !matches!(result, Ok(Ok(()))) { return true; }
                if let Some(runtime) = run.sandbox.take()
                    && runtime.stop().await.is_err() { return true; }
                producer_done = true;
            }
            incoming = control => match incoming {
                Some(delivery) if delivery.frame().ticket != Some(ticket) => {}
                Some(delivery) => match &delivery.frame().message {
                    SpaceMessage::Cancel {} => {
                        if let Some(link) = link.take() { link.channel.fence(); }
                        return outage_expired;
                    }
                    SpaceMessage::EntryAck { through } if link.as_ref().is_some_and(|link| *through <= link.cursor) => {
                        if tail.acknowledge(*through).is_err() { link = None; }
                    }
                    SpaceMessage::Reply { callback_id, result } if run.broker.as_mut().is_some_and(|broker| broker.reply(*callback_id, result.clone())) => {}
                    _ => { link = None; }
                },
                None => { link = None; }
            },
            Some(mut replacement) = replacements.recv() => {
                if let Ok(offset) = tail.acknowledge(replacement.cursor)
                    && replacement.channel.sender().send(package, ticket, SpaceMessage::EntryAttached {
                        origin: replacement.origin,
                        buffered_through: *run.published.borrow(),
                    }).is_ok()
                {
                    replacement.offset = offset;
                    link = Some(replacement);
                    if let Some(broker) = run.broker.as_mut() { broker.reattached(); }
                }
            }
            sent = send => match sent {
                Ok(Outbound::Call(id)) => {
                    run.broker.as_mut().expect("relayed call").sent(id);
                }
                Ok(Outbound::Output { seq, end }) => {
                    tail.sent(seq, end);
                    let link = link.as_mut().expect("sending carrier");
                    link.cursor = seq;
                    link.offset = end;
                    read = None;
                }
                Err(_) => { link = None; }
            },
            call = entry_call => {
                run.broker.as_mut().expect("entry broker").queue(call);
            }
            changed = run.published.changed(), if !producer_done => {
                if changed.is_err() { tokio::task::yield_now().await; }
            }
        }
    }
}
