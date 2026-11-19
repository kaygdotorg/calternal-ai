//! Attach the supervisor endpoint to a worker assignment from machine sync.
use super::*;
use crate::protocol::SpaceChannelAssignment;
use calternal_plugin_host::{
    Error as ProtocolError, MAX_FRAME_BYTES,
    space_channel::{
        MAX_ACTIVE_TICKETS, PackagePin, SpaceChannel, SpaceMessage,
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
        if slot.channel_started.swap(true, Ordering::AcqRel) {
            return;
        }
        self.supervise_channel(&binding, &slot).await;
        // This reservation belongs to a v2 placement and cannot keep its sandbox
        // after the authenticated channel lease ends.
        Self::close(&slot, false).await;
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
async fn relay_package(
    channel: &mut SpaceChannel,
    pin: &PackagePin,
    mut exec: Exec,
) -> Result<(), ProtocolError> {
    let input = exec.input.take().ok_or(ProtocolError::Invocation)?;
    let mut active = HashMap::<Uuid, Uuid>::new();
    let mut line = Vec::new();
    channel.ready(pin)?;
    let sender = channel.sender();
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
                    SpaceMessage::Invoke { operation, input, .. } if active.len() < MAX_ACTIVE_TICKETS && !active.contains_key(&ticket) => {
                        let alias = Uuid::new_v4();
                        active.insert(ticket, alias);
                        (alias, ChildMessage::Invoke { operation: operation.clone(), input: input.clone() })
                    }
                    SpaceMessage::Reply { callback_id, result } => {
                        let Some(&alias) = active.get(&ticket) else { continue };
                        (alias, ChildMessage::HostReply { call_id: *callback_id, result: result.clone() })
                    }
                    SpaceMessage::Cancel {} => {
                        let Some(alias) = active.remove(&ticket) else { continue };
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
                        let Some(ticket) = active.iter().find_map(|(ticket, alias)| (*alias == frame.invocation_id).then_some(*ticket)) else {
                            // Late output for a canceled local invocation carries no authority.
                            continue;
                        };
                        let message = match frame.message {
                            ChildMessage::HostCall { call_id, method, input } =>
                                SpaceMessage::Call { callback_id: call_id, method, input },
                            ChildMessage::Complete { data } => {
                                active.remove(&ticket);
                                SpaceMessage::Complete { result: Ok(data) }
                            }
                            ChildMessage::Failed {} => {
                                active.remove(&ticket);
                                SpaceMessage::Complete { result: Err(calternal_types::ErrorCode::Unavailable) }
                            }
                            _ => return Err(ProtocolError::Invalid),
                        };
                        // Stop draining stdout until this frame has credit.
                        // The channel task still receives credit and renewals.
                        match sender.send_wait(pin, ticket, message).await {
                            Err(ProtocolError::TooLarge) => {
                                active.remove(&ticket);
                                let cancel = ChildFrame::encode(frame.invocation_id, ChildMessage::Cancel {})?;
                                input.send(cancel).await.map_err(|_| ProtocolError::Invocation)?;
                                sender.send_wait(pin, ticket, SpaceMessage::Complete {
                                    result: Err(calternal_types::ErrorCode::Unavailable),
                                }).await?;
                            }
                            result => result?,
                        }
                    }
                }
                Some(OutputEvent::Data(crate::protocol::Stream::Stderr, _)) => {}
                Some(OutputEvent::Exit(_)) | None => return Err(ProtocolError::Invocation),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn oversized_child_call_cancels_its_alias_before_reporting_failure() {
        let pin = PackagePin {
            package: "fixture".into(),
            digest: "a".repeat(64),
        };
        let space = calternal_types::SpaceId::new();
        let assignment = Uuid::new_v4();
        let (host_io, runtime_io) = tokio::io::duplex(4096);
        let (host, runtime) = tokio::join!(
            SpaceChannel::host(host_io, space, assignment, vec![pin.clone()]),
            SpaceChannel::runtime(runtime_io, space, assignment),
        );
        let mut host = host.unwrap();
        let mut runtime = runtime.unwrap();
        let (input, mut child_input) = tokio::sync::mpsc::channel(2);
        let (output, events) = tokio::sync::mpsc::channel(2);
        let relay_pin = pin.clone();
        let relay = tokio::spawn(async move {
            relay_package(
                &mut runtime,
                &relay_pin,
                Exec {
                    input: Some(input),
                    events,
                },
            )
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            assert!(matches!(
                host.receive().await.unwrap().frame().message,
                SpaceMessage::Ready {}
            ));
            let ticket = Uuid::new_v4();
            host.sender()
                .send_checked_wait(
                    &pin,
                    ticket,
                    SpaceMessage::Invoke {
                        operation: "fixture.head".into(),
                        input: serde_json::json!({}),
                        deadline_ms: 2_000,
                    },
                    || true,
                )
                .await
                .unwrap();
            let invoke: ChildFrame =
                serde_json::from_slice(&child_input.recv().await.unwrap()).unwrap();
            let alias = invoke.invocation_id;
            let call_id = Uuid::new_v4();
            let call = |text: String| ChildMessage::HostCall {
                call_id,
                method: calternal_plugin_host::space_channel::SpaceMethod::ItemHead,
                input: serde_json::json!(text),
            };
            let overhead = ChildFrame::encode(alias, call(String::new()))
                .unwrap()
                .len();
            let at_pipe_limit =
                ChildFrame::encode(alias, call("x".repeat(MAX_FRAME_BYTES + 1 - overhead)))
                    .unwrap();
            output
                .send(OutputEvent::Data(
                    crate::protocol::Stream::Stdout,
                    at_pipe_limit,
                ))
                .await
                .unwrap();
            let cancel: ChildFrame =
                serde_json::from_slice(&child_input.recv().await.unwrap()).unwrap();
            assert_eq!(cancel.invocation_id, alias);
            assert!(matches!(cancel.message, ChildMessage::Cancel {}));
            let terminal = host.receive().await.unwrap();
            assert_eq!(terminal.frame().ticket, Some(ticket));
            assert!(matches!(
                terminal.frame().message,
                SpaceMessage::Complete {
                    result: Err(calternal_types::ErrorCode::Unavailable)
                }
            ));
            assert!(host.is_live());
        })
        .await
        .unwrap();
        host.fence();
        relay.await.unwrap().unwrap();
    }
}
