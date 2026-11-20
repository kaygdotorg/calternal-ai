//! Keep one bounded output stream per exact running entry across carrier loss.
use super::entry_tail::EntryTail;
use super::*;
use crate::sandbox::entry::{EntryOutput, RUNTIME_LIMIT};
use calternal_plugin_host::{
    entry::EntryProgram,
    space_channel::{PackagePin, SpaceChannel, SpaceDelivery, SpaceMessage},
};
use calternal_types::{ErrorCode, SpaceId};
use tokio::sync::{mpsc, watch};

// Proposed maximum absence, and upper bound for a completed tail awaiting
// its commit ACK. The sandbox's own runtime limit applies independently.
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
                    if let Ok((sandbox, _)) = started { let _ = sandbox.stop().await; }
                    return;
                }
                (started, false)
            }
            started = &mut launch => (started, true),
        };
        let Ok((sandbox, exec)) = started else {
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
        let mut output = EntryOutput::new(exec);
        let mut sandbox = Some(sandbox);
        let deadline = tokio::time::sleep(RUNTIME_LIMIT);
        tokio::pin!(deadline);
        drive_entry(
            package,
            ticket,
            &mut output,
            &mut sandbox,
            attached.then_some(link),
            replacements,
            deadline,
        )
        .await;
        if let Some(sandbox) = sandbox {
            let _ = sandbox.stop().await;
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

async fn drive_entry(
    package: &PackagePin,
    ticket: Uuid,
    output: &mut EntryOutput,
    sandbox: &mut Option<Arc<dyn Sandbox>>,
    mut link: Option<Attachment>,
    mut replacements: mpsc::Receiver<Attachment>,
    mut deadline: std::pin::Pin<&mut tokio::time::Sleep>,
) {
    let mut tail = EntryTail::default();
    let mut was_live = link.is_some();
    let expiry = tokio::time::sleep(OUTAGE_LIMIT);
    tokio::pin!(expiry);
    while sandbox.is_some() || !tail.is_empty() {
        if was_live && link.is_none() && sandbox.is_some() {
            expiry
                .as_mut()
                .reset(tokio::time::Instant::now() + OUTAGE_LIMIT);
        }
        was_live = link.is_some();
        let watch_expiry = !was_live || sandbox.is_none();
        let outbound = link.as_ref().and_then(|link| {
            tail.after(link.cursor)
                .map(|(seq, msg)| (seq, msg, link.channel.sender()))
        });
        let control = async {
            match link.as_mut() {
                Some(link) => link.receive().await,
                None => std::future::pending().await,
            }
        };
        let send = async {
            let Some((seq, msg, sender)) = outbound else {
                return std::future::pending().await;
            };
            sender.send_wait(package, ticket, msg).await.map(|()| seq)
        };
        tokio::select! {
            biased;
            _ = &mut expiry, if watch_expiry => return,
            _ = &mut deadline, if sandbox.is_some() => {
                let runtime = sandbox.take().expect("active entry");
                if runtime.stop().await.is_err() { return; }
                if link.is_some() { expiry.as_mut().reset(tokio::time::Instant::now() + OUTAGE_LIMIT); }
                tail.failed();
            }
            incoming = control => match incoming {
                Some(delivery) if delivery.frame().ticket != Some(ticket) => {}
                Some(delivery) => match &delivery.frame().message {
                    SpaceMessage::Cancel {} => {
                        if let Some(link) = link.take() { link.channel.fence(); }
                        return;
                    }
                    SpaceMessage::EntryAck { through } if link.as_ref().is_some_and(|link| *through <= link.cursor) => {
                        if tail.acknowledge(*through).is_err() { link = None; }
                    }
                    _ => { link = None; }
                },
                None => { link = None; }
            },
            Some(replacement) = replacements.recv() => {
                if tail.acknowledge(replacement.cursor).is_ok()
                    && replacement.channel.sender().send(package, ticket, SpaceMessage::EntryAttached {
                        origin: replacement.origin,
                        buffered_through: tail.produced(),
                    }).is_ok()
                {
                    link = Some(replacement);
                }
            }
            sent = send => match sent {
                Ok(seq) => {
                    tail.sent(seq);
                    link.as_mut().expect("sending carrier").cursor = seq;
                }
                Err(_) => { link = None; }
            },
            frame = output.next(), if sandbox.is_some() && tail.can_read() => {
                let mut frame = frame.ok().flatten();
                if frame.as_ref().is_some_and(|value| value["kind"] == "output") {
                    if tail.push(frame.take().expect("output value")).is_ok() { continue; }
                }
                let runtime = sandbox.take().expect("active entry");
                if runtime.stop().await.is_err() { return; }
                if link.is_some() { expiry.as_mut().reset(tokio::time::Instant::now() + OUTAGE_LIMIT); }
                if frame.is_none_or(|value| tail.push(value).is_err()) { tail.failed(); }
            }
        }
    }
}
