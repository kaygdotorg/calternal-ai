//! Bounded per-package data credit and an independent control queue.
use super::*;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) struct Credits {
    digest: String,
    owed: usize,
    available: usize,
    outstanding: usize,
    sent_frames: usize,
    receive_available: usize,
    received_frames: usize,
    return_bytes: usize,
    return_frames: usize,
}

impl Credits {
    pub(super) fn new(digest: String) -> Self {
        Self {
            digest,
            owed: 0,
            available: DATA,
            outstanding: 0,
            sent_frames: 0,
            receive_available: DATA,
            received_frames: 0,
            return_bytes: 0,
            return_frames: 0,
        }
    }
}

type Guard = Box<dyn Fn() -> bool + Send + Sync>;

pub(super) struct Pending {
    pub frame: SpaceFrame,
    pub reserved: usize,
    pub control: Option<OwnedSemaphorePermit>,
    pub guard: Option<Guard>,
}

pub(super) struct Shared {
    pub epoch: Uuid,
    pub host: bool,
    pub live: watch::Sender<bool>,
    pub data: mpsc::Sender<Pending>,
    pub control: mpsc::Sender<Pending>,
    pub reserved: Arc<Semaphore>,
    pub returns: tokio::sync::Notify,
    pub credit_changed: tokio::sync::Notify,
    // One space only. The lock covers credit arithmetic and digest comparison.
    // Queueing, back-pressure waits and socket I/O run after releasing it.
    pub credits: Mutex<HashMap<String, Credits>>,
}

impl Shared {
    pub(super) fn submit(
        &self,
        package: Option<&PackagePin>,
        ticket: Option<Uuid>,
        message: SpaceMessage,
    ) -> Result<(), Error> {
        self.submit_checked(package, ticket, message, None)
    }

    pub(super) fn submit_checked(
        &self,
        package: Option<&PackagePin>,
        ticket: Option<Uuid>,
        message: SpaceMessage,
        guard: Option<Guard>,
    ) -> Result<(), Error> {
        let mut pending = Some(self.prepare(package, ticket, message, guard, false)?);
        if pending.as_ref().unwrap().control.is_some() {
            return self
                .control
                .try_send(pending.take().unwrap())
                .map_err(|_| Error::BackPressure);
        }
        self.queue_data(&mut pending)
    }

    pub(super) async fn submit_wait(
        &self,
        package: &PackagePin,
        ticket: Uuid,
        message: SpaceMessage,
        guard: Option<Guard>,
    ) -> Result<(), Error> {
        if message.control() {
            return Err(Error::Refused);
        }
        let guarded = guard.is_some();
        let mut life = self.live.subscribe();
        let mut pending = Some(self.prepare(Some(package), Some(ticket), message, guard, true)?);
        loop {
            let changed = self.credit_changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            match self.queue_data(&mut pending) {
                Err(Error::BackPressure) => {}
                result => return result,
            }
            tokio::select! {
                _ = life.wait_for(|live| !*live) => return Err(Error::Invocation),
                _ = &mut changed => {}
                _ = tokio::time::sleep(Duration::from_millis(100)), if guarded => {}
            }
        }
    }

    fn prepare(
        &self,
        package: Option<&PackagePin>,
        ticket: Option<Uuid>,
        message: SpaceMessage,
        guard: Option<Guard>,
        bounded_result: bool,
    ) -> Result<Pending, Error> {
        if !*self.live.borrow() {
            return Err(Error::Invocation);
        }
        if guard.as_ref().is_some_and(|check| !check()) {
            return Err(Error::Refused);
        }
        let limit = match &message {
            m if m.control() => CONTROL,
            SpaceMessage::Reply { .. } | SpaceMessage::Complete { .. } if bounded_result => {
                MAX_TICKET_RESULT_FRAME
            }
            _ => MAX_LINE,
        };
        let frame = SpaceFrame {
            v: V,
            epoch: self.epoch,
            // Reserve for the widest ordinal. The writer refunds the difference.
            seq: u64::MAX,
            package: package.map(|pin| pin.package.clone()),
            digest: package.map(|pin| pin.digest.clone()),
            ticket,
            message,
        };
        let route = frame.route(self.host)?;
        let reserved = frame.encode(limit)?.len();
        let control = if frame.message.control() {
            if route.is_some() {
                self.with_package(&frame, |_| Ok(()))?;
            }
            Some(
                self.reserved
                    .clone()
                    .try_acquire_many_owned(reserved as u32)
                    .map_err(|_| Error::BackPressure)?,
            )
        } else {
            None
        };
        Ok(Pending {
            frame,
            reserved,
            control,
            guard,
        })
    }

    fn queue_data(&self, pending: &mut Option<Pending>) -> Result<(), Error> {
        if !*self.live.borrow() {
            return Err(Error::Invocation);
        }
        let item = pending.as_ref().expect("unsubmitted frame");
        if item.guard.as_ref().is_some_and(|guard| !guard()) {
            return Err(Error::Refused);
        }
        self.with_package(&item.frame, |credit| {
            if credit.outstanding == PER_PACKAGE || item.reserved > credit.available {
                return Err(Error::BackPressure);
            }
            credit.outstanding += 1;
            credit.available -= item.reserved;
            Ok(())
        })?;
        if let Err(error) = self.data.try_send(pending.take().expect("reserved frame")) {
            // The sum of the package windows bounds queue occupancy. A send
            // can still race channel shutdown, in which case refund the debit.
            self.discard(&error.into_inner())?;
            return Err(Error::Invocation);
        }
        Ok(())
    }

    pub(super) fn discard(&self, pending: &Pending) -> Result<(), Error> {
        self.with_package(&pending.frame, |credit| {
            credit.outstanding -= 1;
            credit.available += pending.reserved;
            Ok(())
        })?;
        self.credit_changed.notify_waiters();
        Ok(())
    }

    pub(super) fn written(&self, pending: &Pending, sent: usize) -> Result<(), Error> {
        self.with_package(&pending.frame, |credit| {
            credit.sent_frames += 1;
            credit.available += pending.reserved.checked_sub(sent).ok_or(Error::Invalid)?;
            credit.owed += sent;
            Ok(())
        })?;
        if pending.reserved != sent {
            self.credit_changed.notify_waiters();
        }
        Ok(())
    }

    pub(super) fn accept(&self, frame: &SpaceFrame, received: usize) -> Result<usize, Error> {
        let Some(_) = frame.route(!self.host)? else {
            return Ok(0);
        };
        let accepted = self.with_package(frame, |credit| match frame.message {
            SpaceMessage::Credit { bytes, frames } => {
                credit.owed = credit.owed.checked_sub(bytes).ok_or(Error::Invalid)?;
                credit.sent_frames = credit
                    .sent_frames
                    .checked_sub(frames)
                    .ok_or(Error::Invalid)?;
                credit.outstanding -= frames;
                credit.available += bytes;
                Ok(0)
            }
            _ if frame.message.control() => Ok(0),
            _ => {
                if credit.received_frames == PER_PACKAGE || received > credit.receive_available {
                    return Err(Error::BackPressure);
                }
                credit.received_frames += 1;
                credit.receive_available -= received;
                Ok(received)
            }
        })?;
        if matches!(frame.message, SpaceMessage::Credit { .. }) {
            self.credit_changed.notify_waiters();
        }
        Ok(accepted)
    }

    pub(super) fn release(&self, frame: &SpaceFrame, bytes: usize) -> Result<(), Error> {
        self.with_package(frame, |credit| {
            credit.return_frames += 1;
            credit.return_bytes += bytes;
            Ok(())
        })?;
        self.returns.notify_one();
        Ok(())
    }

    pub(super) fn take_return(&self) -> Result<Option<SpaceFrame>, Error> {
        let mut credits = self.credits.lock().map_err(|_| Error::Invocation)?;
        let Some((package, credit)) = credits
            .iter_mut()
            .find(|(_, credit)| credit.return_frames != 0)
        else {
            return Ok(None);
        };
        let bytes = std::mem::take(&mut credit.return_bytes);
        let frames = std::mem::take(&mut credit.return_frames);
        credit.received_frames -= frames;
        credit.receive_available += bytes;
        Ok(Some(SpaceFrame {
            v: V,
            epoch: self.epoch,
            seq: 0,
            package: Some(package.clone()),
            digest: Some(credit.digest.clone()),
            ticket: None,
            message: SpaceMessage::Credit { bytes, frames },
        }))
    }

    fn with_package<T>(
        &self,
        frame: &SpaceFrame,
        use_credit: impl FnOnce(&mut Credits) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut credits = self.credits.lock().map_err(|_| Error::Invocation)?;
        let credit = credits
            .get_mut(frame.package.as_deref().ok_or(Error::Invalid)?)
            .filter(|credit| frame.digest.as_ref() == Some(&credit.digest))
            .ok_or(Error::Refused)?;
        use_credit(credit)
    }
}
