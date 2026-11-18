//! One request per vsock connection. Length prefixes bound allocations before parsing.

use calternal_types::SpaceId;
use ring::hmac;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::protocol::{ChunkEvent, SandboxSpec};
use crate::sandbox::ExecRequest;
use uuid::Uuid;

pub(super) const PORT: u32 = 5000;
const MAX_MESSAGE: usize = 8 * 1024 * 1024;

// A separate read-only boot drive supplies a fresh capability to each guest.
// Guest-local vsock clients can reach the listener on some kernel versions.
const CONTROL_DOMAIN: &[u8] = b"calternal-microvm-control-v1";

pub(super) async fn prove(
    writer: &mut (impl AsyncWrite + Unpin),
    key: &hmac::Key,
) -> std::io::Result<()> {
    writer
        .write_all(hmac::sign(key, CONTROL_DOMAIN).as_ref())
        .await
}

pub(super) async fn verify(
    reader: &mut (impl AsyncRead + Unpin),
    key: &hmac::Key,
) -> std::io::Result<()> {
    let mut proof = [0; 32];
    reader.read_exact(&mut proof).await?;
    hmac::verify(key, CONTROL_DOMAIN, &proof)
        .map_err(|_| std::io::ErrorKind::PermissionDenied.into())
}

#[derive(Serialize, Deserialize)]
pub(super) enum Request {
    Ready,
    Start(SpaceId, SandboxSpec),
    StartHarness(SpaceId, SandboxSpec, crate::protocol::HarnessSandbox),
    Discard(Uuid, Uuid),
    Exec(Uuid, ExecRequest),
    Freeze(Uuid),
    Resume(Uuid),
    Release(Uuid, bool),
}

#[derive(Serialize, Deserialize)]
pub(super) enum Response {
    Ready,
    Error,
    Output(ChunkEvent),
}

pub(super) async fn send(
    writer: &mut (impl AsyncWrite + Unpin),
    value: &impl Serialize,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    if bytes.len() > MAX_MESSAGE {
        return Err(std::io::Error::other("guest frame exceeds size limit"));
    }
    writer.write_u32(bytes.len() as u32).await?;
    writer.write_all(&bytes).await
}

pub(super) async fn receive<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> std::io::Result<T> {
    let len = reader.read_u32().await? as usize;
    if len > MAX_MESSAGE {
        return Err(std::io::Error::other("guest frame exceeds size limit"));
    }
    let mut bytes = vec![0; len];
    reader.read_exact(&mut bytes).await?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}
