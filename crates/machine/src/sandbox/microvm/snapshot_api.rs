//! Firecracker's local snapshot controls, used by the small offline placement.
use std::{path::Path, time::Duration};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
};

use super::{SandboxError, SandboxResult};

/// A checkpoint can be discarded after an explicit refusal. Transport and
/// local resource failures preserve it for a retry.
#[derive(Debug)]
pub(super) enum AttemptError {
    /// An endpoint explicitly rejected the checkpoint or operation.
    Refused(SandboxError),
    /// The endpoint or local resources could not complete the attempt.
    Unavailable(SandboxError),
}

impl From<SandboxError> for AttemptError {
    fn from(error: SandboxError) -> Self {
        Self::Unavailable(error)
    }
}

impl From<AttemptError> for SandboxError {
    fn from(error: AttemptError) -> Self {
        match error {
            AttemptError::Refused(error) | AttemptError::Unavailable(error) => error,
        }
    }
}

/// Pause vCPUs through the local Firecracker socket before saving.
pub(super) async fn pause(socket: &Path) -> SandboxResult<()> {
    request(socket, "PATCH", "/vm", json!({"state":"Paused"}))
        .await
        .map_err(Into::into)
}

/// Resume a paused guest after a successful snapshot load.
pub(super) async fn resume(socket: &Path) -> SandboxResult<()> {
    request(socket, "PATCH", "/vm", json!({"state":"Resumed"}))
        .await
        .map_err(Into::into)
}

/// Write Firecracker state and full or dirty memory into private draft files.
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
    .map_err(Into::into)
}

/// Load state and mapped memory while keeping vCPUs paused and rebinding vsock.
pub(super) async fn load(
    socket: &Path,
    state: &Path,
    memory: &Path,
    vsock: &Path,
) -> Result<(), AttemptError> {
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
) -> Result<(), AttemptError> {
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
                return Err(SandboxError("Invalid Firecracker snapshot response".into()).into());
            }
            line[len..len + n].copy_from_slice(&part[..n]);
            len += n;
            io.consume(n);
            if line[len - 1] == b'\n' {
                break;
            }
            part = io.fill_buf().await.map_err(SandboxError::new)?;
        }
        if !line[..len].starts_with(b"HTTP/1.1 204 ") {
            let error = SandboxError(format!("Firecracker rejected {path}"));
            return Err(
                if path == "/snapshot/load"
                    && line[..len].starts_with(b"HTTP/1.1 400 ")
                    && load_refused(io).await.unwrap_or(false)
                {
                    AttemptError::Refused(error)
                } else {
                    error.into()
                },
            );
        }
        Ok(())
    })
    .await
    .map_err(|_| {
        AttemptError::Unavailable(SandboxError(format!("Firecracker timed out on {path}")))
    })?
}

// Firecracker exposes only a message in its 400 response. Preserve a
// checkpoint on a reported OS/resource error or an unreadable response.
async fn load_refused(io: BufReader<UnixStream>) -> std::io::Result<bool> {
    let mut bounded = io.take(8192);
    let mut line = Vec::new();
    let mut size = None;
    loop {
        line.clear();
        if bounded.read_until(b'\n', &mut line).await? == 0 {
            return Ok(false);
        }
        if line == b"\r\n" {
            break;
        }
        if let Ok(header) = std::str::from_utf8(&line)
            && let Some((name, value)) = header.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            size = value
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|size| *size <= 4096);
        }
    }
    let Some(size) = size else {
        return Ok(false);
    };
    let mut body = vec![0; size];
    bounded.read_exact(&mut body).await?;
    let Ok(fault) = serde_json::from_slice::<Value>(&body) else {
        return Ok(false);
    };
    let Some(message) = fault.get("fault_message").and_then(Value::as_str) else {
        return Ok(false);
    };
    let message = message.to_ascii_lowercase();
    Ok(![
        "os error ",
        "no space left",
        "input/output error",
        "out of memory",
        "resource temporarily unavailable",
    ]
    .iter()
    .any(|resource| message.contains(resource)))
}
