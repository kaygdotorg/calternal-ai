//! Server adapter for the machine role's sandbox and exec interfaces.
//! The server never spawns a worker or invokes a container engine.
use crate::machines::MachineService;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use calternal_machine::{
    protocol::{ChunkEvent, SandboxSpec},
    sandbox::{
        Backend, BoxFuture, Exec, ExecRequest, OutputEvent, Sandbox, SandboxError, SandboxResult,
    },
};
use calternal_types::AccountId;
use std::sync::Arc;
use uuid::Uuid;

pub(super) struct MachineBackend(pub Arc<MachineService>);

impl Backend for MachineBackend {
    fn name(&self) -> &'static str {
        "machine"
    }
    fn discard(&self, _: Uuid, _: Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError(
                "workers are discarded by ending their lease".into(),
            ))
        })
    }
    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let service = self.0.clone();
            let worker = spec
                .worker
                .clone()
                .ok_or_else(|| SandboxError("worker source missing".into()))?;
            let account = AccountId::from_uuid(spec.account_id).map_err(SandboxError::new)?;
            let thread = spec.thread_id;
            let run = tokio::task::spawn_blocking(move || {
                service.request_worker(account, thread, worker)
            })
            .await
            .map_err(SandboxError::new)?
            .map_err(|e| SandboxError(e.message))?;
            Ok(Arc::new(MachineSandbox {
                service: self.0.clone(),
                account,
                lease: run.lease_id,
            }) as Arc<dyn Sandbox>)
        })
    }
}

struct MachineSandbox {
    service: Arc<MachineService>,
    account: AccountId,
    lease: Uuid,
}
impl Sandbox for MachineSandbox {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            let (service, account, lease) = (self.service.clone(), self.account, self.lease);
            let id = tokio::task::spawn_blocking(move || {
                service.worker_exec(account, lease, request.argv)
            })
            .await
            .map_err(SandboxError::new)?
            .map_err(|e| SandboxError(e.message))?;
            let (input, mut writes) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
            let service = self.service.clone();
            tokio::spawn(async move {
                while let Some(data) = writes.recv().await {
                    let service = service.clone();
                    if !matches!(tokio::task::spawn_blocking(move || service.worker_input(account, id, data)).await, Ok(Ok(()))) { break; }
                }
                let _ = tokio::task::spawn_blocking(move || service.stop_run(account, lease)).await;
            });
            let (events, rx) = tokio::sync::mpsc::channel(16);
            let service = self.service.clone();
            tokio::spawn(async move {
                let mut after = 0;
                loop {
                    let output = match service
                        .read_output(account, id, after, std::time::Duration::from_secs(1))
                        .await
                    {
                        Ok(output) => output,
                        Err(_) => break,
                    };
                    if output.lost_before.is_some() {
                        break;
                    }
                    for chunk in output.chunks {
                        after = chunk.seq;
                        let event = match chunk.event {
                            ChunkEvent::Data { stream, data } => match B64.decode(data) {
                                Ok(bytes) => OutputEvent::Data(stream, bytes),
                                Err(_) => return,
                            },
                            ChunkEvent::Exit(info) => OutputEvent::Exit(info),
                        };
                        if events.send(event).await.is_err() {
                            return;
                        }
                    }
                    if output.exit.is_some() {
                        return;
                    }
                }
            });
            Ok(Exec {
                input: Some(input),
                events: rx,
            })
        })
    }
    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError(
                "worker leases freeze on machine disconnect".into(),
            ))
        })
    }
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError(
                "workers restart on the next invocation".into(),
            ))
        })
    }
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        let (service, account, lease) = (self.service.clone(), self.account, self.lease);
        Box::pin(async move {
            let stop = service.clone();
            tokio::task::spawn_blocking(move || stop.stop_run(account, lease))
                .await
                .map_err(SandboxError::new)?
                .map_err(|e| SandboxError(e.message))?;
            service
                .wait_worker_stopped(lease)
                .await
                .map_err(|e| SandboxError(e.message))
        })
    }
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.stop()
    }
}
