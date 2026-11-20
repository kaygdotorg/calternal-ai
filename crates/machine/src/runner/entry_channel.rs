//! Keep one bounded output stream per exact running entry across carrier loss.
use super::entry_tail::EntryTail;
use super::*;
use crate::sandbox::{
    EntryLog,
    entry::RUNTIME_LIMIT,
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
const OUTAGE_LIMIT: Duration = Duration::from_secs(24 * 60 * 60);

/// Reach the same entry actor on a replacement carrier while the machine lives.
pub(super) struct EntrySlot {
    origin: Uuid,
    space: SpaceId,
    package: PackagePin,
    attach: mpsc::Sender<Attachment>,
}

struct Attachment {
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
                    if let Ok((sandbox, _, log)) = started {
                        let _ = sandbox.stop().await;
                        let _ = tokio::fs::remove_file(log.path).await;
                    }
                    return;
                }
                (started, false)
            }
            started = &mut launch => (started, true),
        };
        let Ok((
            sandbox,
            exec,
            EntryLog {
                path,
                reader,
                writer,
            },
        )) = started
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
        let mut producer = tokio::spawn(record_entry(exec, writer, publish));
        let mut sandbox = Some(sandbox);
        let deadline = tokio::time::sleep(RUNTIME_LIMIT);
        tokio::pin!(deadline);
        let preserve_log = drive_entry(
            package,
            ticket,
            reader,
            &mut producer,
            published,
            &mut sandbox,
            attached.then_some(link),
            replacements,
            deadline,
        )
        .await;
        if let Some(sandbox) = sandbox {
            let _ = sandbox.stop().await;
        }
        if !producer.is_finished() {
            producer.abort();
            let _ = producer.await;
        }
        if !preserve_log {
            let _ = tokio::fs::remove_file(path).await;
        }
    }
}

async fn launch_interrupted(link: &mut Attachment, ticket: Uuid) -> bool {
    while let Some(delivery) = link.receive().await {
        if delivery.frame().ticket == Some(ticket) {
            return matches!(delivery.frame().message, SpaceMessage::Cancel {});
        }
    }
    false
}

// Keep the log if local I/O or teardown fails, or if Stop arrives after
// the watchdog has already ended the run.
async fn drive_entry(
    package: &PackagePin,
    ticket: Uuid,
    reader: Arc<std::fs::File>,
    producer: &mut tokio::task::JoinHandle<Result<(), ProtocolError>>,
    mut published: watch::Receiver<u64>,
    sandbox: &mut Option<Arc<dyn Sandbox>>,
    mut link: Option<Attachment>,
    mut replacements: mpsc::Receiver<Attachment>,
    mut deadline: std::pin::Pin<&mut tokio::time::Sleep>,
) -> bool {
    let mut tail = EntryTail::default();
    let mut was_live = link.is_some();
    let mut producer_done = false;
    let mut outage_expired = false;
    let mut read = None;
    let expiry = tokio::time::sleep(OUTAGE_LIMIT);
    tokio::pin!(expiry);
    loop {
        let through = *published.borrow_and_update();
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
        let outbound = if let Some(link) = link.as_ref()
            && link.cursor < through
            && tail.can_send(link.cursor)
        {
            if read
                .as_ref()
                .is_none_or(|(offset, _, _)| *offset != link.offset)
            {
                let Ok((end, message)) = read_entry(reader.clone(), link.offset).await else {
                    return true;
                };
                read = Some((link.offset, end, message));
            }
            let (_, end, message) = read.as_ref().expect("read at relay cursor");
            if matches!(message, SpaceMessage::Complete { .. }) && !producer_done {
                None
            } else {
                Some((
                    link.cursor + 1,
                    *end,
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
            let Some((seq, end, msg, sender)) = outbound else {
                return std::future::pending().await;
            };
            sender
                .send_wait(package, ticket, msg)
                .await
                .map(|()| (seq, end))
        };
        tokio::select! {
            biased;
            _ = &mut expiry, if watch_expiry => {
                outage_expired = true;
                if let Some(runtime) = sandbox.take()
                    && runtime.stop().await.is_err() { return true; }
            }
            _ = &mut deadline, if sandbox.is_some() => {
                let runtime = sandbox.take().expect("active entry");
                if runtime.stop().await.is_err() { return true; }
            }
            result = &mut *producer, if !producer_done => {
                if !matches!(result, Ok(Ok(()))) { return true; }
                if let Some(runtime) = sandbox.take()
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
                    _ => { link = None; }
                },
                None => { link = None; }
            },
            Some(mut replacement) = replacements.recv() => {
                if let Ok(offset) = tail.acknowledge(replacement.cursor)
                    && replacement.channel.sender().send(package, ticket, SpaceMessage::EntryAttached {
                        origin: replacement.origin,
                        buffered_through: *published.borrow(),
                    }).is_ok()
                {
                    replacement.offset = offset;
                    link = Some(replacement);
                }
            }
            sent = send => match sent {
                Ok((seq, end)) => {
                    tail.sent(seq, end);
                    let link = link.as_mut().expect("sending carrier");
                    link.cursor = seq;
                    link.offset = end;
                    read = None;
                }
                Err(_) => { link = None; }
            },
            changed = published.changed(), if !producer_done => {
                if changed.is_err() { tokio::task::yield_now().await; }
            }
        }
    }
}
