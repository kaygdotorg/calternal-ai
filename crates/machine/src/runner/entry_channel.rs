//! Relay a single contribution on an authenticated attempt carrier.
use super::*;
use crate::sandbox::entry::EntryOutput;
use calternal_plugin_host::space_channel::{PackagePin, SpaceChannel, SpaceMessage};
use calternal_types::{ErrorCode, SpaceId};

impl Runner {
    /// Supervise one launch on its enrolled reservation. A changed session,
    /// wrong ticket or closed carrier prevents further output and starts
    /// verified container removal. One task per acquired sandbox permit.
    pub(super) async fn open_entry_channel(
        &self,
        space: SpaceId,
        assignment: Uuid,
        package: PackagePin,
        ticket: Uuid,
    ) {
        let Ok(_capacity) = self.entry_capacity.try_acquire() else {
            return;
        };
        if ticket.is_nil() {
            return;
        }
        let Ok((io, mut session)) = self.link.upgrade_space(space, assignment).await else {
            return;
        };
        let Ok(mut channel) = SpaceChannel::runtime(io, space, assignment).await else {
            return;
        };
        if channel.ready(&package).is_err() {
            return;
        }
        let mut life = channel.lifetime();
        let delivery = tokio::select! {
            biased;
            _ = session.changed() => return,
            _ = life.wait_for(|live| !*live) => return,
            delivery = channel.receive() => delivery,
        };
        let Some(delivery) = delivery else { return };
        let frame = delivery.frame();
        if frame.ticket != Some(ticket)
            || frame.package.as_deref() != Some(&package.package)
            || frame.digest.as_deref() != Some(&package.digest)
        {
            return;
        }
        let SpaceMessage::StartEntry { program } = &frame.message else {
            return;
        };
        let sender = channel.sender();
        let launch_fence = sender.clone();
        let live = Arc::new(move || launch_fence.is_live());
        let launch = self.backend.start_entry(ticket, program, live);
        tokio::pin!(launch);
        let started = tokio::select! {
            biased;
            _ = interrupted(&mut channel, &mut session, &mut life, ticket) => {
                channel.fence();
                // Podman may have allocated a container before cancellation.
                // Join launch and reap by its verified identity.
                if let Ok((sandbox, _)) = launch.await {
                    let _ = sandbox.stop().await;
                }
                return;
            }
            started = &mut launch => started,
        };
        let terminal = if let Ok((sandbox, exec)) = started {
            let output = async {
                let mut output = EntryOutput::new(exec);
                while let Some(frame) = output.next().await? {
                    if frame["kind"] == "terminal" {
                        return Ok(frame);
                    }
                    sender
                        .send_wait(&package, ticket, SpaceMessage::Chunk { data: frame })
                        .await?;
                }
                Err(calternal_plugin_host::Error::Invocation)
            };
            let result = tokio::select! {
                biased;
                _ = interrupted(&mut channel, &mut session, &mut life, ticket) => {
                    channel.fence();
                    None
                }
                result = output => Some(result.map_err(|_| ErrorCode::Unavailable)),
            };
            if sandbox.stop().await.is_err() {
                // Without verified teardown, do not send completion.
                return;
            }
            let Some(result) = result else { return };
            result
        } else {
            Err(ErrorCode::Unavailable)
        };
        if sender
            .send_wait(
                &package,
                ticket,
                SpaceMessage::Complete { result: terminal },
            )
            .await
            .is_ok()
        {
            // Sending queues a bounded frame. Give the host time to receive
            // it and close the carrier before dropping our transport handle.
            let _ = tokio::time::timeout(
                Duration::from_secs(5),
                interrupted(&mut channel, &mut session, &mut life, ticket),
            )
            .await;
        }
    }
}

/// Additional host data for this exact ticket ends admission. Fence apart
/// from stdout credit so a blocked receiver cannot delay Stop.
async fn interrupted(
    channel: &mut SpaceChannel,
    session: &mut tokio::sync::watch::Receiver<Option<Arc<str>>>,
    life: &mut tokio::sync::watch::Receiver<bool>,
    ticket: Uuid,
) {
    loop {
        tokio::select! {
            _ = session.changed() => return,
            _ = life.wait_for(|live| !*live) => return,
            delivery = channel.receive() => {
                if delivery.is_none_or(|delivery| delivery.frame().ticket == Some(ticket)) {
                    return;
                }
            }
        }
    }
}
