//! Temporary, space-authorized reads through the existing machine output stream.
use axum::{
    Extension,
    body::Bytes,
    extract::{OriginalUri, Path, State as HttpState},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use calternal_machine::protocol::{
    ChunkEvent, Down, PREVIEW_BODY_BYTES, PreviewMetadata, PreviewRequest, Stream,
};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::{
    http::{Svc, authorize_run, body, reply, service},
    state::{Exec, ExecState, Lease, State},
    *,
};
use crate::{
    api::{Shared, assets::SealedFrame},
    auth::{Directory, Identity},
};

pub(super) const MAX_FETCHES: usize = 32;
const PER_PREVIEW: usize = 4;
const MAX_PUBLICATIONS: usize = 128;
const PER_LEASE: usize = 4;
const LIFETIME: Duration = Duration::from_secs(15 * 60);
const FETCH_TIMEOUT: Duration = Duration::from_secs(12);
const METADATA_LIMIT: usize = 1024;

pub(super) struct Published {
    lease: Uuid,
    fence: u64,
    port: u16,
    expires: Instant,
    permits: Arc<Semaphore>,
}

struct Fetch {
    svc: Arc<MachineService>,
    id: Uuid,
    owner: AccountId,
    machine: Uuid,
    permits: Option<(OwnedSemaphorePermit, OwnedSemaphorePermit)>,
}

impl Drop for Fetch {
    fn drop(&mut self) {
        let (svc, id, machine, permits) =
            (self.svc.clone(), self.id, self.machine, self.permits.take());
        tokio::task::spawn_blocking(move || {
            if let Ok(mut registry) = svc.lock() {
                registry.execs.remove(&id);
                if let Some(live) = registry
                    .machines
                    .get_mut(&machine)
                    .and_then(|m| m.live.as_mut())
                {
                    live.outbox.retain(
                        |e| !matches!(e.message, Down::Preview { exec_id, .. } if exec_id == id),
                    );
                }
            }
            drop(permits);
        });
    }
}

fn missing() -> Error {
    Error::not_found("preview not found")
}

fn target_failed() -> Error {
    Error::unavailable("sandbox preview did not return a complete HTTP response")
}

fn lease_target(st: &State, lease: Uuid) -> Option<(&Lease, Uuid)> {
    let l = st.leases.get(&lease)?;
    if l.provider_login
        || l.row.worker
        || l.row.background_title
        || l.row.shared_sponsor.is_some()
        || l.state != LeaseState::Running
    {
        return None;
    }
    let machine = l.assigned_machine()?;
    st.machines
        .get(&machine)?
        .live
        .as_ref()
        .filter(|live| live.preview)?;
    Some((l, machine))
}

fn current(st: &State, id: Uuid) -> Result<(&Published, &Lease, Uuid), Error> {
    let preview = st.previews.get(&id).ok_or_else(missing)?;
    let (l, machine) = lease_target(st, preview.lease).ok_or_else(missing)?;
    if preview.expires <= Instant::now() || l.row.fence != preview.fence {
        return Err(missing());
    }
    Ok((preview, l, machine))
}

impl MachineService {
    fn publish_preview(&self, account: AccountId, lease: Uuid, port: u16) -> Result<Uuid, Error> {
        let mut state = self.lock()?;
        let st = &mut *state;
        let (l, _) = lease_target(st, lease)
            .filter(|(l, _)| l.row.account == account)
            .ok_or_else(missing)?;
        if port == 0 {
            return Err(Error::invalid_input("preview port must be nonzero"));
        }
        let fence = l.row.fence;
        let _steps = st.lease_steps(&l.row)?;
        let now = Instant::now();
        st.previews.retain(|_, p| {
            p.expires > now
                && st.leases.get(&p.lease).is_some_and(|lease| {
                    lease.state == LeaseState::Running && lease.row.fence == p.fence
                })
        });
        if st.previews.len() >= MAX_PUBLICATIONS
            || st
                .previews
                .values()
                .filter(|p| p.lease == lease && p.fence == fence)
                .count()
                >= PER_LEASE
        {
            return Err(Error::unavailable("preview publication limit reached"));
        }
        let id = Uuid::new_v4();
        st.previews.insert(
            id,
            Published {
                lease,
                fence,
                port,
                expires: now + LIFETIME,
                permits: Arc::new(Semaphore::new(PER_PREVIEW)),
            },
        );
        Ok(id)
    }

    fn preview_space(&self, id: Uuid) -> Result<SpaceId, Error> {
        let st = self.lock()?;
        let (_, l, _) = current(&st, id)?;
        let _steps = st.lease_steps(&l.row).map_err(|_| missing())?;
        Ok(l.row.space)
    }

    fn open_preview(self: &Arc<Self>, id: Uuid, path: String) -> Result<Fetch, Error> {
        let mut st = self.lock()?;
        let (preview, l, machine) = current(&st, id)?;
        let request = PreviewRequest {
            port: preview.port,
            path,
        };
        if !request.valid() {
            return Err(Error::invalid_input("invalid preview path"));
        }
        let at_capacity = |_| Error::unavailable("preview fetch limit reached");
        let install_permit = self
            .preview_slots
            .clone()
            .try_acquire_owned()
            .map_err(at_capacity)?;
        let preview_permit = preview
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(at_capacity)?;
        let _steps = st.lease_steps(&l.row).map_err(|_| missing())?;
        let (lease_id, fence, owner) = (l.row.id, l.row.fence, l.row.account);
        let exec_id = Uuid::new_v4();
        let mut exec = Exec::new(lease_id, owner, vec![], vec![], ExecState::Sent);
        exec.preview = true;
        st.execs.insert(exec_id, exec);
        st.push(
            machine,
            Down::Preview {
                lease_id,
                fence,
                exec_id,
                request,
            },
        );
        Ok(Fetch {
            svc: self.clone(),
            id: exec_id,
            owner,
            machine,
            permits: Some((install_permit, preview_permit)),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publication {
    port: u16,
}

pub(super) async fn publish<D: Directory>(
    HttpState(state): HttpState<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(lease): Path<Uuid>,
    bytes: Bytes,
) -> Response {
    reply(
        async {
            let svc = service(svc)?;
            let account = authorize_run(&state, &svc, &who, lease)
                .await
                .map_err(|_| missing())?;
            let Publication { port } = body(&bytes)?;
            let id = blocking(move || svc.publish_preview(account, lease, port)).await?;
            Ok(json!({ "id": id, "path": format!("/api/v1/machine-previews/{id}/") }))
        }
        .await,
    )
}

async fn authorize<D: Directory>(
    state: &Shared<D>,
    svc: &Arc<MachineService>,
    account: AccountId,
    id: Uuid,
) -> Result<(), Error> {
    let (state, svc) = (state.clone(), svc.clone());
    blocking(move || {
        let space = svc.preview_space(id)?;
        state
            .directory
            .capabilities(account, space)
            .filter(|caps| caps.contains(calternal_types::Capability::SpaceRead))
            .map(|_| ())
            .ok_or_else(missing)
    })
    .await
}

#[derive(Deserialize)]
pub(super) struct PreviewPath {
    preview: Uuid,
}

pub(super) async fn view<D: Directory>(
    HttpState(state): HttpState<Shared<D>>,
    svc: Svc,
    Extension(who): Extension<Identity>,
    Path(PreviewPath { preview }): Path<PreviewPath>,
    OriginalUri(uri): OriginalUri,
) -> Response {
    async {
        let svc = service(svc)?;
        authorize(&state, &svc, who.id, preview).await?;
        let prefix = format!("/api/v1/machine-previews/{preview}");
        let path = uri
            .path_and_query()
            .and_then(|p| p.as_str().strip_prefix(&prefix))
            .ok_or_else(missing)?
            .to_owned();
        let registry = svc.clone();
        let fetch = blocking(move || registry.open_preview(preview, path)).await?;
        // A disconnected viewer does not free capacity while the machine still
        // runs its fetch. The task owns both permits until it completes.
        let bytes = tokio::spawn(async move {
            tokio::time::timeout(FETCH_TIMEOUT, fetch.collect())
                .await
                .map_err(|_| target_failed())?
        })
        .await
        .map_err(|_| target_failed())??;
        authorize(&state, &svc, who.id, preview).await?;
        response(bytes)
    }
    .await
    .unwrap_or_else(crate::api::error_response)
}

impl Fetch {
    async fn collect(&self) -> Result<Vec<u8>, Error> {
        let (mut after, mut bytes) = (0, Vec::new());
        loop {
            let batch = self
                .svc
                .read_output(self.owner, self.id, after, Duration::from_secs(1))
                .await?;
            if batch.lost_before.is_some() {
                return Err(target_failed());
            }
            for chunk in batch.chunks {
                if let ChunkEvent::Data {
                    stream: Stream::Stdout,
                    data,
                } = chunk.event
                {
                    let data = B64.decode(data).map_err(|_| target_failed())?;
                    if bytes.len().saturating_add(data.len()) > PREVIEW_BODY_BYTES + METADATA_LIMIT
                    {
                        return Err(target_failed());
                    }
                    bytes.extend_from_slice(&data);
                }
            }
            if let Some(exit) = batch.exit {
                return if exit.code == Some(0) {
                    Ok(bytes)
                } else {
                    Err(target_failed())
                };
            }
            after = batch.next;
        }
    }
}

fn response(mut bytes: Vec<u8>) -> Result<Response, Error> {
    let newline = bytes
        .iter()
        .take(METADATA_LIMIT)
        .position(|b| *b == b'\n')
        .ok_or_else(target_failed)?;
    if bytes.len() - newline - 1 > PREVIEW_BODY_BYTES {
        return Err(target_failed());
    }
    let meta: PreviewMetadata =
        serde_json::from_slice(&bytes[..newline]).map_err(|_| target_failed())?;
    let status = StatusCode::from_u16(meta.status)
        .ok()
        .filter(|s| s.as_u16() >= 200 && s.as_u16() < 600 && !s.is_redirection())
        .ok_or_else(target_failed)?;
    let content_type = match meta
        .content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "text/html" => "text/html; charset=utf-8",
        "application/json" => "application/json",
        "image/png" => "image/png",
        "image/jpeg" => "image/jpeg",
        "image/gif" => "image/gif",
        "image/webp" => "image/webp",
        _ => "text/plain; charset=utf-8",
    };
    let mut reply = (
        status,
        [(header::CONTENT_TYPE, content_type)],
        bytes.split_off(newline + 1),
    )
        .into_response();
    reply.extensions_mut().insert(SealedFrame(HeaderValue::from_static(
        "sandbox allow-scripts; default-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:"
    )));
    Ok(reply)
}
