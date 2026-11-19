//! Firecracker's local snapshot controls, used by the small offline placement.
use std::{path::Path, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

use super::{SandboxError, SandboxResult};

pub(super) async fn pause(socket: &Path) -> SandboxResult<()> {
    request(socket, "PATCH", "/vm", json!({"state":"Paused"})).await
}

pub(super) async fn resume(socket: &Path) -> SandboxResult<()> {
    request(socket, "PATCH", "/vm", json!({"state":"Resumed"})).await
}

pub(super) async fn create(
    socket: &Path,
    state: &Path,
    memory: &Path,
    dirty: bool,
) -> SandboxResult<()> {
    request(
        socket,
        "PUT",
        "/snapshot/create",
        json!({
            "snapshot_type": if dirty {"Diff"} else {"Full"},
            "snapshot_path":state,
            "mem_file_path":memory,
        }),
    )
    .await
}

pub(super) async fn load(
    socket: &Path,
    state: &Path,
    memory: &Path,
    vsock: &Path,
) -> SandboxResult<()> {
    request(
        socket,
        "PUT",
        "/snapshot/load",
        json!({
            "snapshot_path":state,
            "mem_backend":{"backend_type":"File","backend_path":memory},
            "track_dirty_pages":true,
            "resume_vm":false,
            "vsock_override":{"uds_path":vsock},
        }),
    )
    .await
}

async fn request(
    socket: &Path,
    method: &'static str,
    path: &'static str,
    body: Value,
) -> SandboxResult<()> {
    let body = serde_json::to_vec(&body).map_err(SandboxError::new)?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut io = UnixStream::connect(socket)
            .await
            .map_err(SandboxError::new)?;
        io.write_all(request.as_bytes())
            .await
            .map_err(SandboxError::new)?;
        io.write_all(&body).await.map_err(SandboxError::new)?;
        let mut io = BufReader::with_capacity(512, io);
        let reply = io.fill_buf().await.map_err(SandboxError::new)?;
        let mut line = [0_u8; 128];
        let mut len = 0;
        let mut part = reply;
        loop {
            let n = part
                .iter()
                .position(|&b| b == b'\n')
                .map_or(part.len(), |i| i + 1);
            if n == 0 || len + n > line.len() {
                return Err(SandboxError("Invalid Firecracker snapshot response".into()));
            }
            line[len..len + n].copy_from_slice(&part[..n]);
            len += n;
            if line[len - 1] == b'\n' {
                break;
            }
            io.consume(n);
            part = io.fill_buf().await.map_err(SandboxError::new)?;
        }
        if !line[..len].starts_with(b"HTTP/1.1 204 ") {
            return Err(SandboxError(format!("Firecracker rejected {path}")));
        }
        Ok(())
    })
    .await
    .map_err(|_| SandboxError(format!("Firecracker timed out on {path}")))?
}
