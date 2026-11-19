//! Bounded duplex space channels on byte streams supplied by enrolled transport.
/// Private broker messages exchanged with a single sandboxed package process.
pub mod child;
mod flow;
mod wire;

use crate::{Error, MAX_FRAME_BYTES};
use calternal_types::SpaceId;
use flow::{Credits, Pending, Shared};
pub use wire::{PackagePin, SpaceFrame, SpaceMessage, SpaceMethod};

use std::{
    collections::HashMap,
    future::poll_fn,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
    time::Duration,
};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::{mpsc, watch},
    time::{Instant, timeout},
};
use uuid::Uuid;
use wire::{Offer, Welcome, parse, read_line};

const V: u8 = 1;
const MAX_LINE: usize = MAX_FRAME_BYTES + 1;
const CONTROL: usize = 4096;
const DATA: usize = 2 * MAX_LINE;
const PER_PACKAGE: usize = 8;
const MAX_PACKAGES: usize = 8;
const LEASE_MS: u64 = 4_000;
const HANDSHAKE_LIMIT: Duration = Duration::from_secs(2);
const WRITE_LIMIT: Duration = Duration::from_millis(300);

/// One live connection belonging to a single space assignment and runtime epoch.
/// Dropping it fences the local endpoint and closes both halves of the stream.
pub struct SpaceChannel {
    shared: Arc<Shared>,
    received: mpsc::Receiver<SpaceDelivery>,
}

impl SpaceChannel {
    /// Agree an epoch and initial windows over an already authenticated stream.
    /// The host must check the machine session and assignment before calling.
    /// Cost: one bounded handshake exchange and one per-space I/O task.
    pub async fn host<IO>(
        io: IO,
        space: SpaceId,
        assignment: Uuid,
        packages: Vec<PackagePin>,
    ) -> Result<Self, Error>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(io);
        let mut reader = BufReader::new(reader);
        let welcome = Welcome {
            v: V,
            space,
            assignment,
            epoch: Uuid::new_v4(),
            lease_ms: LEASE_MS,
            data_credit: DATA,
            control_credit: CONTROL,
            packages,
        };
        let credits = validate_welcome(&welcome, space, assignment)?;
        timeout(HANDSHAKE_LIMIT, async {
            let mut line = Vec::new();
            read_line(&mut reader, &mut line, CONTROL).await?;
            let offer: Offer = parse(&line)?;
            if offer.v != V || offer.space != space || offer.assignment != assignment {
                return Err(Error::Refused);
            }
            write_handshake(&mut writer, &welcome).await
        })
        .await
        .map_err(|_| Error::Invocation)??;
        Ok(Self::start(reader, writer, welcome, credits, true))
    }

    /// Start the supervisor endpoint on an authenticated stream from the expected host.
    /// A major, space, assignment or window mismatch refuses the handshake.
    /// Cost: one bounded handshake exchange and one per-space I/O task.
    pub async fn runtime<IO>(io: IO, space: SpaceId, assignment: Uuid) -> Result<Self, Error>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, mut writer) = tokio::io::split(io);
        let mut reader = BufReader::new(reader);
        let welcome = timeout(HANDSHAKE_LIMIT, async {
            write_handshake(
                &mut writer,
                &Offer {
                    v: V,
                    space,
                    assignment,
                },
            )
            .await?;
            let mut line = Vec::new();
            read_line(&mut reader, &mut line, CONTROL).await?;
            parse::<Welcome>(&line)
        })
        .await
        .map_err(|_| Error::Invocation)??;
        let credits = validate_welcome(&welcome, space, assignment)?;
        Ok(Self::start(reader, writer, welcome, credits, false))
    }

    fn start(
        reader: impl AsyncBufRead + Unpin + Send + 'static,
        writer: impl AsyncWrite + Unpin + Send + 'static,
        welcome: Welcome,
        credits: HashMap<String, Credits>,
        host: bool,
    ) -> Self {
        let queue = PER_PACKAGE * credits.len();
        let (data, outbound) = mpsc::channel(queue);
        let (control, controls) = mpsc::channel(32);
        let (inbound, received) = mpsc::channel(queue + 32);
        let (live, mut life) = watch::channel(true);
        let (renew, expiry) = watch::channel(Instant::now());
        let shared = Arc::new(Shared {
            epoch: welcome.epoch,
            host,
            live,
            data,
            control,
            reserved: Arc::new(tokio::sync::Semaphore::new(CONTROL - 512)),
            returns: tokio::sync::Notify::new(),
            credits: Mutex::new(credits),
        });
        let connection = shared.clone();
        tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = life.wait_for(|live| !*live) => {}
                _ = read_loop(reader, &connection, inbound, renew) => {}
                _ = write_loop(writer, &connection, outbound, controls) => {}
                _ = maintain_lease(&connection, expiry) => {}
            }
            connection.live.send_replace(false);
        });
        Self { shared, received }
    }

    /// Epoch pinned to the authenticated connection. Cost: one field read.
    pub fn epoch(&self) -> Uuid {
        self.shared.epoch
    }

    /// Queue a direction-checked invocation frame without waiting on a stalled writer.
    /// Cost: bounded serialization, one package credit lookup and one nonblocking send.
    pub fn send(
        &self,
        package: &PackagePin,
        ticket: Uuid,
        message: SpaceMessage,
    ) -> Result<(), Error> {
        self.shared.submit(Some(package), Some(ticket), message)
    }

    /// Queue a frame with a local delivery guard. The writer checks the guard
    /// again before assigning an ordinal and before every socket write or flush poll.
    /// The guard must be nonblocking and may run after the submitting task ends.
    pub fn send_checked(
        &self,
        package: &PackagePin,
        ticket: Uuid,
        message: SpaceMessage,
        check: impl Fn() -> bool + Send + Sync + 'static,
    ) -> Result<(), Error> {
        let guard = Box::new(check);
        if !guard() || message.control() {
            return Err(Error::Refused);
        }
        self.shared
            .submit_checked(Some(package), Some(ticket), message, Some(guard))
    }

    /// Whether this channel has retained local authority. Cost: one space-local watch read.
    pub fn is_live(&self) -> bool {
        *self.shared.live.borrow()
    }

    /// Queue readiness for one pinned package from the supervisor.
    /// Cost: one reserved-control slot and a nonblocking send.
    pub fn ready(&self, package: &PackagePin) -> Result<(), Error> {
        self.shared
            .submit(Some(package), None, SpaceMessage::Ready {})
    }

    /// Receive the next permitted application frame or end when locally fenced.
    /// The frame retains package credit until it is dropped.
    pub async fn receive(&mut self) -> Option<SpaceDelivery> {
        let mut live = self.shared.live.subscribe();
        tokio::select! {
            biased;
            _ = live.wait_for(|value| !*value) => None,
            item = self.received.recv() => item,
        }
    }

    /// Ask the remote endpoint to stop and end this stream after the control frame.
    /// Delivery is opportunistic under back pressure. Use the independent local
    /// fence when authority ends before the socket can accept that message.
    pub fn request_remote_stop(&self) -> Result<(), Error> {
        self.shared.submit(None, None, SpaceMessage::Fence {})
    }

    /// Apply the local fence without waiting for a remote acknowledgement or socket I/O.
    /// Cost: one per-space notification. Dropping the carrier does the same.
    pub fn fence(&self) {
        self.shared.live.send_replace(false);
    }

    /// Retain a control-plane fence independent of the application consumer.
    /// Cost: one per-space watch reference. It does not retain the socket.
    pub fn fence_handle(&self) -> SpaceFence {
        SpaceFence(self.shared.live.clone())
    }

    /// Subscribe to the lifetime of this channel. False is terminal.
    /// Cost: one per-space watch reference.
    pub fn lifetime(&self) -> watch::Receiver<bool> {
        self.shared.live.subscribe()
    }
}

impl Drop for SpaceChannel {
    fn drop(&mut self) {
        self.fence();
    }
}

/// An out-of-band fence for a single channel epoch.
pub struct SpaceFence(watch::Sender<bool>);

impl SpaceFence {
    /// End local delivery and interrupt the stream task. No socket or storage I/O.
    pub fn close(&self) {
        self.0.send_replace(false);
    }
}

/// A permitted inbound frame and its credit reservation.
pub struct SpaceDelivery {
    frame: SpaceFrame,
    bytes: usize,
    shared: Arc<Shared>,
}

impl SpaceDelivery {
    /// Borrow an accepted frame without copying its payload.
    pub fn frame(&self) -> &SpaceFrame {
        &self.frame
    }
}

impl Drop for SpaceDelivery {
    fn drop(&mut self) {
        if self.bytes != 0 && self.shared.release(&self.frame, self.bytes).is_err() {
            self.shared.live.send_replace(false);
        }
    }
}

fn validate_welcome(
    welcome: &Welcome,
    space: SpaceId,
    assignment: Uuid,
) -> Result<HashMap<String, Credits>, Error> {
    if welcome.v != V
        || welcome.space != space
        || welcome.assignment != assignment
        || assignment.is_nil()
        || welcome.epoch.is_nil()
        || welcome.lease_ms != LEASE_MS
        || welcome.data_credit != DATA
        || welcome.control_credit != CONTROL
        || welcome.packages.is_empty()
        || welcome.packages.len() > MAX_PACKAGES
    {
        return Err(Error::Refused);
    }
    let mut credits = HashMap::new();
    for pin in &welcome.packages {
        if !crate::registry::identifier(&pin.package)
            || pin.digest.len() != 64
            || !pin
                .digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || credits
                .insert(pin.package.clone(), Credits::new(pin.digest.clone()))
                .is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(credits)
}

async fn write_handshake(
    writer: &mut (impl AsyncWrite + Unpin),
    value: &impl serde::Serialize,
) -> Result<(), Error> {
    let mut line = crate::bounded::json(value, CONTROL - 1)?;
    line.push(b'\n');
    write_bytes(writer, &line).await
}

async fn write_bytes(writer: &mut (impl AsyncWrite + Unpin), bytes: &[u8]) -> Result<(), Error> {
    writer
        .write_all(bytes)
        .await
        .map_err(|_| Error::Invocation)?;
    writer.flush().await.map_err(|_| Error::Invocation)
}

async fn write_guarded(
    writer: &mut (impl AsyncWrite + Unpin),
    mut bytes: &[u8],
    check: &(dyn Fn() -> bool + Send + Sync),
) -> Result<(), Error> {
    // An inactive grant after a partial frame ends the stream. Returning to
    // the queue would concatenate the next frame onto an unfinished line.
    while !bytes.is_empty() {
        let n = poll_fn(|context| {
            if !check() {
                return Poll::Ready(Err(Error::Refused));
            }
            Pin::new(&mut *writer)
                .poll_write(context, bytes)
                .map(|result| result.map_err(|_| Error::Invocation))
        })
        .await?;
        if n == 0 {
            return Err(Error::Invocation);
        }
        bytes = &bytes[n..];
    }
    poll_fn(|context| {
        if !check() {
            return Poll::Ready(Err(Error::Refused));
        }
        Pin::new(&mut *writer)
            .poll_flush(context)
            .map(|result| result.map_err(|_| Error::Invocation))
    })
    .await
}

async fn maintain_lease(
    shared: &Shared,
    mut expiry: watch::Receiver<Instant>,
) -> Result<(), Error> {
    if shared.host {
        let period = Duration::from_secs(1);
        let mut ticks = tokio::time::interval_at(Instant::now() + period, period);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticks.tick().await;
            shared.submit(None, None, SpaceMessage::Renew {})?;
        }
    }
    loop {
        let end = *expiry.borrow_and_update() + Duration::from_millis(LEASE_MS);
        timeout(
            end.saturating_duration_since(Instant::now()),
            expiry.changed(),
        )
        .await
        .map_err(|_| Error::Invocation)?
        .map_err(|_| Error::Invocation)?;
    }
}

async fn read_loop(
    mut reader: impl AsyncBufRead + Unpin,
    shared: &Arc<Shared>,
    inbound: mpsc::Sender<SpaceDelivery>,
    renew: watch::Sender<Instant>,
) -> Result<(), Error> {
    let mut tail = 0_u64;
    let mut line = Vec::new();
    loop {
        read_line(&mut reader, &mut line, MAX_LINE).await?;
        let frame: SpaceFrame = parse(&line)?;
        if frame.v != V
            || frame.epoch != shared.epoch
            || tail.checked_add(1) != Some(frame.seq)
            || (frame.message.control() && line.len() > CONTROL)
        {
            return Err(Error::Invalid);
        }
        let bytes = shared.accept(&frame, line.len())?;
        tail = frame.seq;
        match frame.message {
            SpaceMessage::Renew {} => {
                renew.send_replace(Instant::now());
            }
            SpaceMessage::Fence {} => return Ok(()),
            SpaceMessage::Credit { .. } => {}
            _ => inbound
                .try_send(SpaceDelivery {
                    frame,
                    bytes,
                    shared: shared.clone(),
                })
                .map_err(|_| Error::BackPressure)?,
        }
    }
}

async fn write_loop(
    mut writer: impl AsyncWrite + Unpin,
    shared: &Shared,
    mut data: mpsc::Receiver<Pending>,
    mut control: mpsc::Receiver<Pending>,
) -> Result<(), Error> {
    let mut seq = 0_u64;
    loop {
        let pending = if let Ok(control) = control.try_recv() {
            Some(control)
        } else if let Some(mut returned) = shared.take_return()? {
            seq = seq.checked_add(1).ok_or(Error::Invocation)?;
            returned.seq = seq;
            let bytes = returned.encode(512)?;
            timeout(WRITE_LIMIT, write_bytes(&mut writer, &bytes))
                .await
                .map_err(|_| Error::Invocation)??;
            continue;
        } else {
            tokio::select! {
                biased;
                item = control.recv() => item,
                _ = shared.returns.notified() => continue,
                item = data.recv() => item,
            }
        };
        let mut pending = pending.ok_or(Error::Invocation)?;
        let next = seq.checked_add(1).ok_or(Error::Invocation)?;
        pending.frame.seq = next;
        let bytes = pending.frame.encode(if pending.control.is_some() {
            CONTROL
        } else {
            MAX_LINE
        })?;
        if pending.guard.as_ref().is_some_and(|guard| !guard()) {
            shared.discard(&pending)?;
            continue;
        }
        seq = next;
        if pending.control.is_none() {
            shared.written(&pending, bytes.len())?;
        }
        timeout(WRITE_LIMIT, async {
            match pending.guard.as_deref() {
                Some(check) => write_guarded(&mut writer, &bytes, check).await,
                None => write_bytes(&mut writer, &bytes).await,
            }
        })
        .await
        .map_err(|_| Error::Invocation)??;
        if matches!(pending.frame.message, SpaceMessage::Fence {}) {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod fault_tests;
