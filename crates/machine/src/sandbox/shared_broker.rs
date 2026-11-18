//! Machine-local provider gateway. Only a leased harness sees its socket.
use std::{
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};

use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
};
use calternal_fsio::beneath::Folder;
use calternal_types::SpaceId;
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::UnixListener,
    process::Command,
    sync::{Semaphore, watch},
};
use uuid::Uuid;

use super::{SandboxError, SandboxResult};
use crate::protocol::SharedToolsProvider;

const BODY_LIMIT: usize = 4 * 1024 * 1024;
const OUTPUT_LIMIT: u64 = 16 * 1024;
const TOOL_TIMEOUT: Duration = Duration::from_secs(25);

pub(super) struct Broker {
    listener: tokio::task::JoinHandle<()>,
    pub(super) directory: PathBuf,
    pub(super) access: String,
    closed: watch::Sender<bool>,
}

impl Drop for Broker {
    fn drop(&mut self) {
        self.close();
    }
}

struct Gateway {
    access: String,
    credential: HeaderValue,
    provider: SharedToolsProvider,
    client: reqwest::Client,
    tool_slot: Semaphore,
    tool: ToolSandbox,
    closed: watch::Receiver<bool>,
    #[cfg(test)]
    mock_base: Option<String>,
}

pub(super) struct ToolSandbox {
    pub(super) podman: PathBuf,
    pub(super) image: String,
    pub(super) root_label: String,
    pub(super) work: PathBuf,
    pub(super) writable: bool,
}

impl Broker {
    pub(super) fn close(&self) {
        self.closed.send_replace(true);
        self.listener.abort();
    }

    /// One disposable listener per sandbox start. Loads the machine-owned API
    /// credential once, never into a child environment or the sandbox's mounts.
    pub(super) async fn start(
        root: &Path,
        dir: &Path,
        space: SpaceId,
        provider: SharedToolsProvider,
        tool: ToolSandbox,
    ) -> SandboxResult<Self> {
        let root = root.to_owned();
        let directory = dir.join("broker");
        let setup = directory.clone();
        let credential = tokio::task::spawn_blocking(move || {
            calternal_fsio::private::directory(&setup).map_err(SandboxError::new)?;
            let socket = setup.join("gateway.sock");
            match std::fs::remove_file(&socket) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(SandboxError::new(e)),
            }
            std::fs::write(
                setup.join("client.mjs"),
                include_str!("shared_broker_client.mjs"),
            )
            .map_err(SandboxError::new)?;
            load_credential(&root, space, provider)
        })
        .await
        .map_err(SandboxError::new)??;
        let mut random = [0_u8; 32];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| SandboxError("gateway randomness unavailable".into()))?;
        let access: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let (closed, cancellation) = watch::channel(false);
        let gateway = Arc::new(Gateway {
            access: format!("Bearer {access}"),
            credential,
            provider,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .no_proxy()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(300))
                .build()
                .map_err(SandboxError::new)?,
            tool_slot: Semaphore::new(1),
            tool,
            closed: cancellation,
            #[cfg(test)]
            mock_base: None,
        });
        Self::bind(directory, access, closed, gateway)
    }

    fn bind(
        directory: PathBuf,
        access: String,
        closed: watch::Sender<bool>,
        gateway: Arc<Gateway>,
    ) -> SandboxResult<Self> {
        let listener =
            UnixListener::bind(directory.join("gateway.sock")).map_err(SandboxError::new)?;
        let router = Router::new().fallback(dispatch).with_state(gateway);
        let listener = tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self {
            listener,
            directory,
            access,
            closed,
        })
    }
}

fn load_credential(
    root: &Path,
    space: SpaceId,
    provider: SharedToolsProvider,
) -> SandboxResult<HeaderValue> {
    let bytes = (|| {
        let directory = Folder::open(root)?
            .child("broker-credentials", false)?
            .child(&space.to_string(), false)?;
        let file = directory.open_file(match provider {
            SharedToolsProvider::Claude => "anthropic.key",
            SharedToolsProvider::Codex => "openai.key",
        })?;
        if file.metadata()?.permissions().mode() & 0o077 != 0 {
            return Err(std::io::Error::other("credential permissions"));
        }
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        if bytes.len() > 4096 {
            return Err(std::io::Error::other("credential size"));
        }
        Ok(bytes)
    })()
    .map_err(|_: std::io::Error| {
        SandboxError("approved machine-local provider key unavailable".into())
    })?;
    let key = std::str::from_utf8(&bytes)
        .ok()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| SandboxError("invalid machine-local provider key".into()))?;
    let mut value = match provider {
        SharedToolsProvider::Claude => HeaderValue::from_str(key),
        SharedToolsProvider::Codex => HeaderValue::from_str(&format!("Bearer {key}")),
    }
    .map_err(|_| SandboxError("invalid machine-local provider key".into()))?;
    value.set_sensitive(true);
    Ok(value)
}

impl Gateway {
    fn upstream(&self, path: &str) -> Option<(&'static str, &'static str)> {
        match (self.provider, path) {
            (SharedToolsProvider::Claude, "/v1/messages") => {
                Some(("https://api.anthropic.com/v1/messages", "x-api-key"))
            }
            (SharedToolsProvider::Claude, "/v1/messages/count_tokens") => Some((
                "https://api.anthropic.com/v1/messages/count_tokens",
                "x-api-key",
            )),
            (SharedToolsProvider::Codex, "/v1/responses") => {
                Some(("https://api.openai.com/v1/responses", "authorization"))
            }
            _ => None,
        }
    }
}

async fn dispatch(State(state): State<Arc<Gateway>>, request: Request) -> Response {
    if *state.closed.borrow() {
        return StatusCode::GONE.into_response();
    }
    if request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        != Some(state.access.as_str())
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if request.method() != Method::POST {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let (parts, body) = request.into_parts();
    let path = parts.uri.path();
    let Ok(body) = to_bytes(body, if path == "/tool" { 8192 } else { BODY_LIMIT }).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    if path == "/tool" {
        return run_tool(&state, &body).await;
    }
    let Some((url, auth)) = state.upstream(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(body) = isolate_request(&body, state.provider) else {
        return (StatusCode::BAD_REQUEST, "invalid harness request").into_response();
    };
    #[cfg(test)]
    let mock_url = state.mock_base.as_ref().map(|base| format!("{base}{path}"));
    #[cfg(test)]
    let url = mock_url.as_deref().unwrap_or(url);
    let mut call = state
        .client
        .post(url)
        .header(header::CONTENT_TYPE, "application/json")
        .header(auth, state.credential.clone());
    if state.provider == SharedToolsProvider::Claude {
        for name in ["anthropic-version", "anthropic-beta"] {
            if let Some(value) = parts.headers.get(name) {
                call = call.header(name, value);
            }
        }
    }
    let Ok(reply) = call.body(body).send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    if !reply.status().is_success() {
        // Vendor errors can echo headers or key fragments. Keep their status,
        // and give the harness a deliberately generic provider error.
        let status = if reply.status().is_redirection() {
            StatusCode::BAD_GATEWAY
        } else {
            reply.status()
        };
        return (status, "provider request failed").into_response();
    }
    let content_type = reply
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/json"));
    let streaming = content_type.as_bytes().starts_with(b"text/event-stream");
    let guard = if streaming {
        Body::from_stream(futures_util::stream::try_unfold(
            (reply, Vec::new()),
            |(mut response, mut buffer)| async move {
                loop {
                    if let Some(end) = event_end(&buffer) {
                        let remainder = buffer.split_off(end);
                        check_event(&buffer)?;
                        return Ok::<_, std::io::Error>(Some((
                            Bytes::from(buffer),
                            (response, remainder),
                        )));
                    }
                    let Some(chunk) = response.chunk().await.map_err(std::io::Error::other)? else {
                        return if buffer.is_empty() {
                            Ok(None)
                        } else {
                            Err(rejected_event())
                        };
                    };
                    if buffer.len().saturating_add(chunk.len()) > BODY_LIMIT {
                        return Err(rejected_event());
                    }
                    buffer.extend_from_slice(&chunk);
                }
            },
        ))
    } else {
        let mut reply = reply;
        let mut bytes = Vec::new();
        loop {
            match reply.chunk().await {
                Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= BODY_LIMIT => {
                    bytes.extend_from_slice(&chunk)
                }
                Ok(None) => break,
                _ => return StatusCode::BAD_GATEWAY.into_response(),
            }
        }
        if !valid_reply(&bytes) {
            return StatusCode::BAD_GATEWAY.into_response();
        }
        Body::from(bytes)
    };
    ([(header::CONTENT_TYPE, content_type)], guard).into_response()
}

const TOOL: &str = "mcp__workspace__workspace_command";
const CODEX_NAMESPACE: &str = "mcp__workspace";
const CODEX_METHOD: &str = "workspace_command";

fn isolate_request(body: &[u8], provider: SharedToolsProvider) -> Option<Vec<u8>> {
    let mut body: serde_json::Value = serde_json::from_slice(body).ok()?;
    let contents = body.as_object_mut()?;
    if let Some(tools) = contents.get_mut("tools") {
        tools.as_array_mut()?.retain_mut(|tool| match provider {
            SharedToolsProvider::Claude => {
                tool["name"] == TOOL && tool.get("type").is_none_or(|kind| kind == "custom")
            }
            SharedToolsProvider::Codex => {
                if tool["type"] == "function" && tool["name"] == TOOL {
                    return true;
                }
                if tool["type"] != "namespace" || tool["name"] != CODEX_NAMESPACE {
                    return false;
                }
                tool["tools"].as_array_mut().is_some_and(|functions| {
                    functions.retain(|function| {
                        function["type"] == "function" && function["name"] == CODEX_METHOD
                    });
                    !functions.is_empty()
                })
            }
        });
    }
    serde_json::to_vec(&body).ok()
}

fn event_end(buffer: &[u8]) -> Option<usize> {
    let lf = buffer
        .windows(2)
        .position(|bytes| bytes == b"\n\n")
        .map(|at| at + 2);
    let crlf = buffer
        .windows(4)
        .position(|bytes| bytes == b"\r\n\r\n")
        .map(|at| at + 4);
    lf.into_iter().chain(crlf).min()
}

fn check_event(frame: &[u8]) -> std::io::Result<()> {
    let mut data = Vec::new();
    for line in frame.split(|byte| *byte == b'\n') {
        if let Some(value) = line.strip_prefix(b"data:") {
            if !data.is_empty() {
                data.push(b'\n');
            }
            data.extend_from_slice(value.strip_prefix(b" ").unwrap_or(value));
        }
    }
    if data.is_empty() || data == b"[DONE]" || valid_reply(&data) {
        Ok(())
    } else {
        Err(rejected_event())
    }
}

fn rejected_event() -> std::io::Error {
    std::io::Error::other("unapproved or invalid provider response")
}

fn valid_reply(body: &[u8]) -> bool {
    fn allowed(node: &serde_json::Value) -> bool {
        match node {
            serde_json::Value::Object(fields) => {
                let kind = fields.get("type").and_then(|kind| kind.as_str());
                let name = fields.get("name").and_then(|name| name.as_str());
                let namespace = fields.get("namespace").and_then(|name| name.as_str());
                let allowed_call = match kind {
                    Some("tool_use") => name == Some(TOOL),
                    Some("function_call") => match namespace {
                        Some(CODEX_NAMESPACE) => name == Some(CODEX_METHOD),
                        None => name == Some(TOOL),
                        _ => false,
                    },
                    Some("custom_tool_call") => false,
                    _ => true,
                };
                if !allowed_call {
                    return false;
                }
                fields.values().all(allowed)
            }
            serde_json::Value::Array(items) => items.iter().all(allowed),
            _ => true,
        }
    }
    serde_json::from_slice(body)
        .ok()
        .is_some_and(|value| allowed(&value))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolInput {
    command: String,
}

async fn run_tool(state: &Gateway, body: &[u8]) -> Response {
    let Ok(ToolInput { command }) = serde_json::from_slice::<ToolInput>(body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if command.is_empty() || command.len() > 4096 || command.contains('\0') {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let Ok(_permit) = state.tool_slot.try_acquire() else {
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    };
    match state.tool.execute(command, state.closed.clone()).await {
        Ok(output) => output.into_response(),
        Err(_) => (StatusCode::BAD_GATEWAY, "isolated tool failed").into_response(),
    }
}

impl ToolSandbox {
    async fn execute(
        &self,
        command: String,
        mut closed: watch::Receiver<bool>,
    ) -> SandboxResult<String> {
        if *closed.borrow() {
            return Err(SandboxError("tool lease closed".into()));
        }
        let name = format!("calternal-shared-tool-{}", Uuid::new_v4());
        let mount = format!(
            "{}:/work:{}",
            self.work.display(),
            if self.writable { "rw" } else { "ro" }
        );
        let mut child = Command::new(&self.podman)
            .args([
                "run",
                "-i",
                "--rm",
                "--pull=never",
                "--network=none",
                "--pid=private",
                "--ipc=private",
                "--read-only",
                "--cap-drop=all",
                "--security-opt=no-new-privileges",
                "--userns=keep-id:uid=65534,gid=65534",
                "--user=65534:65534",
                "--http-proxy=false",
                "--log-driver=none",
                "--image-volume=ignore",
                "--memory=256m",
                "--pids-limit=32",
                "--timeout=20",
                "--name",
                &name,
                "--label",
                &self.root_label,
                "--volume",
                &mount,
                "--workdir",
                "/work",
                "--entrypoint=/usr/bin/env",
                &self.image,
                "-i",
                "LANG=C.UTF-8",
                "HOME=/tmp",
                "PATH=/usr/bin:/bin",
                "/bin/sh",
                "-s",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(SandboxError::new)?;
        let mut output = Vec::new();
        let result = tokio::select! {
            result = tokio::time::timeout(TOOL_TIMEOUT, async {
                let mut input = child.stdin.take().expect("tool input pipe");
                input.write_all(command.as_bytes()).await?;
                input.shutdown().await?;
                drop(input);
                child.stdout.take().expect("tool output pipe").take(OUTPUT_LIMIT + 1)
                    .read_to_end(&mut output).await?;
                if output.len() as u64 > OUTPUT_LIMIT {
                    return Err(std::io::Error::other("tool output limit"));
                }
                child.wait().await
            }) => result.map_err(SandboxError::new).and_then(|result| result.map_err(SandboxError::new)),
            _ = closed.changed() => Err(SandboxError("tool lease closed".into())),
        };
        if result.is_err() {
            let _ = child.start_kill();
            let _ = Command::new(&self.podman)
                .args(["rm", "-f", "--ignore", &name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        let status = result?;
        Ok(format!(
            "Exit {}\n{}",
            status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output)
        ))
    }
}

#[cfg(test)]
#[path = "shared_broker_tests.rs"]
mod tests;
