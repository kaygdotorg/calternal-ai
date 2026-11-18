//! Thread handles fenced by the guest socket for the VM generation that made them.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::AccountSlot;
use super::vmm::{self, Guest};
use super::wire::{self, Request, Response};
use crate::protocol::{ChunkEvent, ExitInfo, GUEST_LOST};
use crate::sandbox::{
    BoxFuture, CHUNK_BYTES, Exec, ExecRequest, OUTPUT_CHANNEL, OutputEvent, Sandbox, SandboxError,
    SandboxResult,
};

pub(super) struct Thread {
    slot: AccountSlot,
    socket: PathBuf,
    key: Arc<ring::hmac::Key>,
    id: Uuid,
    worker: bool,
}

impl Thread {
    pub(super) fn new(
        slot: AccountSlot,
        socket: PathBuf,
        key: Arc<ring::hmac::Key>,
        id: Uuid,
        worker: bool,
    ) -> Self {
        Self {
            slot,
            socket,
            key,
            id,
            worker,
        }
    }

    async fn control(&self, request: Request, may_be_absent: bool) -> SandboxResult<()> {
        // Serialize teardown with account deletion and replacement. Only this
        // account's operations can wait on the control request.
        let mut account = self.slot.lock().await;
        let vm = account.vm.as_mut().filter(|vm| vm.socket == self.socket);
        if let Some(vm) = vm
            && vm.alive()
        {
            vm.control(&request).await
        } else if may_be_absent {
            Ok(())
        } else {
            Err(SandboxError("microVM stopped".into()))
        }
    }
}

impl Sandbox for Thread {
    fn available(&self) -> bool {
        let Ok(mut account) = self.slot.try_lock() else {
            return true;
        };
        account
            .vm
            .as_mut()
            .is_some_and(|vm| vm.socket == self.socket && Guest::alive(vm))
    }

    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            if request.argv.is_empty() {
                return Err(SandboxError("empty command".into()));
            }
            request.validate_env_names()?;
            let interactive = request.interactive;
            let channel = tokio::time::timeout(Duration::from_secs(5), async {
                let mut channel = vmm::connect(&self.socket).await?;
                wire::prove(channel.get_mut(), &self.key)
                    .await
                    .map_err(SandboxError::new)?;
                wire::send(channel.get_mut(), &Request::Exec(self.id, request))
                    .await
                    .map_err(SandboxError::new)?;
                Ok::<_, SandboxError>(channel)
            })
            .await
            .map_err(|_| SandboxError("microVM exec handshake timed out".into()))??;
            // Split the buffered reader from the write half without losing
            // bytes already consumed from the Firecracker handshake.
            let (mut reader, mut writer) = tokio::io::split(channel);
            let (input, mut inputs) = mpsc::channel::<Vec<u8>>(OUTPUT_CHANNEL);
            let writer = tokio::spawn(async move {
                while let Some(data) = inputs.recv().await {
                    if data.len() > calternal_types::MAX_FRAME_BYTES + 1
                        || wire::send(&mut writer, &Some(data)).await.is_err()
                    {
                        return;
                    }
                }
                // An explicit EOF works over vsock even without half-closes.
                if interactive {
                    let _ = wire::send(&mut writer, &None::<Vec<u8>>).await;
                }
            });
            let (output, events) = mpsc::channel(OUTPUT_CHANNEL);
            let slot = self.slot.clone();
            let socket = self.socket.clone();
            tokio::spawn(async move {
                let exit = loop {
                    match wire::receive(&mut reader).await {
                        Ok(Response::Output(ChunkEvent::Data { stream, data })) => {
                            match STANDARD.decode(data) {
                                Ok(data) if data.len() <= CHUNK_BYTES => {
                                    if output.send(OutputEvent::Data(stream, data)).await.is_err() {
                                        break None;
                                    }
                                }
                                _ => break Some(failed("invalid microVM output")),
                            }
                        }
                        Ok(Response::Output(ChunkEvent::Exit(exit))) => break Some(exit),
                        Ok(Response::Error) => {
                            break Some(failed("microVM guest could not start exec"));
                        }
                        _ => {
                            // vsock can close just before waitpid sees the guest
                            // exit. Reread generation/pid state after that race.
                            tokio::time::sleep(Duration::from_millis(50)).await;
                            let mut account = slot.lock().await;
                            let alive = account
                                .vm
                                .as_mut()
                                .is_some_and(|vm| vm.socket == socket && vm.alive());
                            break Some(failed(if alive {
                                "microVM guest connection lost"
                            } else {
                                GUEST_LOST
                            }));
                        }
                    }
                };
                writer.abort();
                let mut account = slot.lock().await;
                if account.vm.as_ref().is_some_and(|vm| vm.socket == socket) {
                    account.last_used = Instant::now();
                }
                drop(account);
                if let Some(exit) = exit {
                    let _ = output.send(OutputEvent::Exit(exit)).await;
                }
            });
            Ok(Exec {
                input: interactive.then_some(input),
                events,
            })
        })
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(self.control(Request::Freeze(self.id), false))
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(self.control(Request::Resume(self.id), false))
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(self.control(Request::Release(self.id, false), self.worker))
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(self.control(Request::Release(self.id, true), true))
    }
}

fn failed(error: &str) -> ExitInfo {
    ExitInfo {
        error: Some(error.into()),
        ..ExitInfo::default()
    }
}
