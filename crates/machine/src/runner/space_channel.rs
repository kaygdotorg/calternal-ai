//! Attach the supervisor endpoint to a worker assignment from machine sync.
use super::*;
use crate::protocol::SpaceChannelAssignment;
use calternal_plugin_host::{
    Error as ProtocolError, MAX_FRAME_BYTES,
    space_channel::{
        PackagePin, SpaceChannel, SpaceMessage,
        child::{ChildFrame, ChildMessage},
    },
};

impl Runner {
    pub(super) async fn open_space_channel(self: Arc<Self>, binding: SpaceChannelAssignment) {
        let Some(slot) = self.slot(binding.lease_id).filter(|slot| {
            !slot.removed.load(Ordering::Acquire)
                && slot.fence.load(Ordering::Acquire) == binding.fence
                && slot
                    .spec
                    .worker
                    .as_ref()
                    .is_some_and(|worker| worker.space_id == binding.space)
        }) else {
            return;
        };
        self.supervise_channel(&binding, &slot).await;
        // This reservation belongs to a v2 placement and cannot keep its sandbox
        // after the authenticated channel lease ends.
        if slot.fence.load(Ordering::Acquire) == binding.fence {
            Self::close(&slot, false).await;
        }
    }

    async fn supervise_channel(&self, binding: &SpaceChannelAssignment, slot: &Slot) {
        let Ok((io, mut session)) = self.link.upgrade_space(binding).await else {
            return;
        };
        let Ok(mut channel) = SpaceChannel::runtime(io, binding.space, binding.assignment).await
        else {
            return;
        };
        if slot.removed.load(Ordering::Acquire)
            || slot.fence.load(Ordering::Acquire) != binding.fence
        {
            return;
        }
        let mut life = channel.lifetime();
        let fence = channel.fence_handle();
        let run = async {
            let guard = slot.sandbox.read().await;
            let sandbox = self
                .sandbox(slot, &guard)
                .await
                .map_err(|_| ProtocolError::Invocation)?;
            let exec = sandbox
                .exec(ExecRequest {
                    argv: [
                        "node",
                        "--permission",
                        "--allow-fs-read=.",
                        "--disallow-code-generation-from-strings",
                        "--max-old-space-size=128",
                        "worker.js",
                    ]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                    interactive: true,
                    ..ExecRequest::default()
                })
                .await
                .map_err(|_| ProtocolError::Invocation)?;
            relay_package(&mut channel, &binding.package, exec).await
        };
        tokio::select! {
            _ = session.changed() => fence.close(),
            _ = life.wait_for(|live| !*live) => {}
            _ = run => fence.close(),
        }
    }
}

/// Relay the scoped package pipe over an already-authenticated space channel.
/// The caller owns sandbox teardown. A malformed or mismatched child frame
/// stops this relay so the caller can fence the package before reusing it.
/// Cost: one bounded stdout line buffer and a capability-free invocation ID.
pub async fn relay_package(
    channel: &mut SpaceChannel,
    pin: &PackagePin,
    mut exec: Exec,
) -> Result<(), ProtocolError> {
    let input = exec.input.take().ok_or(ProtocolError::Invocation)?;
    let mut active = None::<(Uuid, Uuid)>;
    let mut line = Vec::new();
    channel.ready(pin)?;
    loop {
        tokio::select! {
            item = channel.receive() => {
                let Some(item) = item else { return Ok(()) };
                let frame = item.frame();
                if frame.package.as_deref() != Some(pin.package.as_str())
                    || frame.digest.as_deref() != Some(pin.digest.as_str()) {
                    return Err(ProtocolError::Refused);
                }
                let ticket = frame.ticket.ok_or(ProtocolError::Invalid)?;
                let (alias, message) = match &frame.message {
                    SpaceMessage::Invoke { operation, input, .. } if active.is_none() => {
                        let alias = Uuid::new_v4();
                        active = Some((alias, ticket));
                        (alias, ChildMessage::Invoke { operation: operation.clone(), input: input.clone() })
                    }
                    SpaceMessage::Reply { callback_id, result } => {
                        let (alias, _) = active.filter(|(_, bound)| *bound == ticket).ok_or(ProtocolError::Refused)?;
                        (alias, ChildMessage::HostReply { call_id: *callback_id, result: result.clone() })
                    }
                    SpaceMessage::Cancel {} => {
                        let (alias, _) = active.take().filter(|(_, bound)| *bound == ticket).ok_or(ProtocolError::Refused)?;
                        (alias, ChildMessage::Cancel {})
                    }
                    _ => return Err(ProtocolError::Invalid),
                };
                input.send(ChildFrame::encode(alias, message)?).await.map_err(|_| ProtocolError::Invocation)?;
            }
            event = exec.events.recv() => match event {
                Some(OutputEvent::Data(crate::protocol::Stream::Stdout, chunk)) => {
                    for byte in chunk {
                        if line.len() == MAX_FRAME_BYTES + 1 {
                            return Err(ProtocolError::TooLarge);
                        }
                        line.push(byte);
                        if byte != b'\n' {
                            continue;
                        }
                        let frame = ChildFrame::decode(&line)?;
                        line.clear();
                        let (_, ticket) = active.filter(|(alias, _)| *alias == frame.invocation_id)
                            .ok_or(ProtocolError::Refused)?;
                        let message = match frame.message {
                            ChildMessage::HostCall { call_id, method, input } =>
                                SpaceMessage::Call { callback_id: call_id, method, input },
                            ChildMessage::Complete { data } => {
                                active = None;
                                SpaceMessage::Complete { result: Ok(data) }
                            }
                            ChildMessage::Failed {} => {
                                active = None;
                                SpaceMessage::Complete { result: Err(calternal_types::ErrorCode::Unavailable) }
                            }
                            _ => return Err(ProtocolError::Invalid),
                        };
                        channel.send(pin, ticket, message)?;
                    }
                }
                Some(OutputEvent::Data(crate::protocol::Stream::Stderr, _)) => {}
                Some(OutputEvent::Exit(_)) | None => return Err(ProtocolError::Invocation),
            }
        }
    }
}
