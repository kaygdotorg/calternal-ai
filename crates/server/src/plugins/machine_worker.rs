//! Server adapter for the machine role's sandbox and exec interfaces.
//! The server never spawns a worker or invokes a container engine.
use crate::machines::MachineService;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use calternal_machine::{
    protocol::{ChunkEvent, SandboxSpec},
    sandbox::{BoxFuture, Exec, ExecRequest, OutputEvent, Sandbox, SandboxError, SandboxResult},
};
use calternal_types::{AccountId, Error};
use std::sync::Arc;
use uuid::Uuid;

/// Admit an enrolled worker through the fenced scheduler. Preserves the
/// caller-safe readiness reason on refusal. Costs one blocking admission.
pub(super) async fn start(
    service: Arc<MachineService>,
    mut spec: SandboxSpec,
) -> Result<Arc<dyn Sandbox>, Error> {
    let worker = spec
        .worker
        .take()
        .ok_or_else(|| Error::internal("worker source missing"))?;
    let account = AccountId::from_uuid(spec.account_id)?;
    let thread = spec.thread_id;
    let scheduler = service.clone();
    let run = crate::blocking::spawn(move || scheduler.request_worker(account, thread, worker))
        .await
        .map_err(|_| Error::internal("worker admission failed"))??;
    Ok(Arc::new(MachineSandbox {
        service,
        account,
        lease: run.lease_id,
    }))
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
            let id =
                crate::blocking::spawn(move || service.worker_exec(account, lease, request.argv))
                    .await
                    .map_err(SandboxError::new)?
                    .map_err(|e| SandboxError(e.message))?;
            let (input, mut writes) = tokio::sync::mpsc::channel::<Vec<u8>>(16);
            let service = self.service.clone();
            tokio::spawn(async move {
                while let Some(data) = writes.recv().await {
                    let service = service.clone();
                    if !matches!(
                        crate::blocking::spawn(move || service.interactive_input(account, id, data))
                            .await,
                        Ok(Ok(()))
                    ) {
                        break;
                    }
                }
                let _ = crate::blocking::spawn(move || service.stop_run(account, lease)).await;
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
            crate::blocking::spawn(move || stop.stop_run(account, lease))
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
