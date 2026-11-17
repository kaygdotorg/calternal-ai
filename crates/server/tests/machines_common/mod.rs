//! Harness for machine tests: the real `calternal` binary in both roles on
//! loopback, with short lease and poll times.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const BIN: &str = env!("CARGO_BIN_EXE_calternal");
/// Lease TTL used by these tests.
pub const LEASE_TTL_MS: u64 = 3_000;
/// Poll hold used by these tests.
pub const POLL_HOLD_MS: u64 = 1_000;

/// A temporary directory removed on drop.
pub struct TempDir(pub PathBuf);
impl TempDir {
    pub fn new(tag: &str) -> Self {
        let p =
            std::env::temp_dir().join(format!("calternal-machines-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    pub fn join(&self, p: &str) -> PathBuf {
        self.0.join(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A child process killed on drop.
pub struct Proc(pub Child);
impl Proc {
    pub fn kill(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
    pub fn signal(&self, sig: &str) {
        Command::new("kill")
            .args([sig, &self.0.id().to_string()])
            .status()
            .unwrap();
    }
    /// Wait for exit up to `limit`; the exit code.
    pub fn wait_exit(&mut self, limit: Duration) -> Option<i32> {
        let t0 = Instant::now();
        while t0.elapsed() < limit {
            if let Ok(Some(st)) = self.0.try_wait() {
                return st.code();
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        None
    }
}
impl Drop for Proc {
    fn drop(&mut self) {
        self.kill();
    }
}

/// The installation arguments of the server under `data`, as `calternal
/// machines …` takes them (the same roots and pinned ID as `Server::start`).
pub fn installation(data: &Path) -> Vec<std::ffi::OsString> {
    let mut out = Vec::new();
    for (flag, root) in [
        ("--appdata", "appdata"),
        ("--userdata-ssd", "userdata-ssd"),
        ("--userdata-hdd", "userdata-hdd"),
    ] {
        out.push(flag.into());
        out.push(data.join(root).into());
    }
    out.push("--installation-id".into());
    out.push("00000000-0000-0000-0000-000000000001".into());
    out
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A running development server with its fixture tokens.
pub struct Server {
    pub proc: Proc,
    pub port: u16,
    pub owner: String,
    pub guest: String,
}

impl Server {
    pub fn start(data: &Path, port: u16) -> Self {
        Self::start_with_plugins(data, port, Path::new("/nonexistent-test-plugins"))
    }

    pub fn start_with_plugins(data: &Path, port: u16, plugins: &Path) -> Self {
        Self::start_with_flags(
            data,
            port,
            plugins,
            &[
                "--machine-lease-ttl-ms",
                &LEASE_TTL_MS.to_string(),
                "--machine-poll-hold-ms",
                &POLL_HOLD_MS.to_string(),
            ],
        )
    }

    /// Start a real server using its normal machine timing for Podman cleanup.
    /// The sandbox stop and discard run on the machine's sync loop.
    pub(super) fn start_with_standard_timing(data: &Path, port: u16, plugins: &Path) -> Self {
        Self::start_with_flags(data, port, plugins, &[])
    }

    fn start_with_flags(data: &Path, port: u16, plugins: &Path, flags: &[&str]) -> Self {
        let startup = data.join("appdata/startup.json");
        let _ = std::fs::remove_file(&startup);
        let child = Command::new(BIN)
            .args([
                "serve",
                "--development",
                "--port",
                &port.to_string(),
                "--create",
                "--installation-id",
                "00000000-0000-0000-0000-000000000001",
            ])
            .arg("--appdata")
            .arg(data.join("appdata"))
            .arg("--userdata-ssd")
            .arg(data.join("userdata-ssd"))
            .arg("--userdata-hdd")
            .arg(data.join("userdata-hdd"))
            .arg("--plugin-root")
            .arg(plugins)
            .args(flags)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut proc = Proc(child);
        let t0 = Instant::now();
        loop {
            if let Ok(bytes) = std::fs::read(&startup)
                && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
                && v["pid"].as_u64() == Some(proc.0.id() as u64)
            {
                let read = |p: &str| {
                    std::fs::read_to_string(v["token_files"][p].as_str().unwrap()).unwrap()
                };
                return Self {
                    owner: read("owner"),
                    guest: read("guest"),
                    proc,
                    port,
                };
            }
            assert!(proc.0.try_wait().unwrap().is_none(), "server exited early");
            assert!(
                t0.elapsed() < Duration::from_secs(20),
                "server did not start"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Call the API as `token`; `body` None means GET.
    pub async fn call(&self, token: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let http = reqwest::Client::new();
        let url = format!("{}{path}", self.url());
        let req = match body {
            Some(b) => http.post(url).json(&b),
            None => http.get(url),
        };
        let resp = req.bearer_auth(token).send().await.unwrap();
        let status = resp.status().as_u16();
        (status, resp.json().await.unwrap_or(Value::Null))
    }

    /// The account id and space ids of a fixture token.
    pub async fn principal(&self, token: &str) -> (String, Vec<(String, String)>) {
        let (s, v) = self.call(token, "/api/v1/describe", None).await;
        assert_eq!(s, 200, "{v}");
        let spaces = v["principal"]["spaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| {
                (
                    g["id"].as_str().unwrap().to_owned(),
                    g["role"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        (v["principal"]["id"].as_str().unwrap().to_owned(), spaces)
    }

    /// A space where `token` is owner.
    pub async fn owned_space(&self, token: &str) -> String {
        let (_, spaces) = self.principal(token).await;
        spaces
            .into_iter()
            .find(|(_, r)| r == "owner")
            .expect("an owned space")
            .0
    }

    /// Create a space for `token` and return its id.
    pub async fn create_space(&self, token: &str) -> String {
        let body = json!({"request_id": uuid::Uuid::new_v4().to_string(), "operation": "spaces.create", "input": {"name": "Machines"}});
        let (s, v) = self.call(token, "/api/v1/operations", Some(body)).await;
        assert_eq!(s, 200, "{v}");
        v["data"]["space"]["id"].as_str().unwrap().to_owned()
    }

    /// A one-time code for the token's account.
    pub async fn code(&self, token: &str) -> String {
        let (s, v) = self
            .call(token, "/api/v1/machines/codes", Some(json!({})))
            .await;
        assert_eq!(s, 200, "{v}");
        v["code"].as_str().unwrap().to_owned()
    }

    pub async fn request_run(&self, token: &str, space: &str, thread: uuid::Uuid) -> Value {
        let (s, v) = self
            .call(
                token,
                "/api/v1/machines/runs",
                Some(json!({"space_id": space, "thread_id": thread})),
            )
            .await;
        assert_eq!(s, 200, "{v}");
        v
    }

    pub async fn run(&self, token: &str, lease: &str) -> Value {
        let (s, v) = self
            .call(token, &format!("/api/v1/machines/runs/{lease}"), None)
            .await;
        assert_eq!(s, 200, "{v}");
        v
    }

    /// Poll the run until `pred` holds or `limit` passes.
    pub async fn wait_run(
        &self,
        token: &str,
        lease: &str,
        limit: Duration,
        pred: impl Fn(&Value) -> bool,
    ) -> Value {
        let t0 = Instant::now();
        loop {
            let v = self.run(token, lease).await;
            if pred(&v) {
                return v;
            }
            assert!(t0.elapsed() < limit, "run never matched: {v}");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn exec(&self, token: &str, lease: &str, argv: &[&str]) -> String {
        let (s, v) = self
            .call(
                token,
                &format!("/api/v1/machines/runs/{lease}/execs"),
                Some(json!({"argv": argv})),
            )
            .await;
        assert_eq!(s, 200, "{v}");
        v["exec_id"].as_str().unwrap().to_owned()
    }

    /// Read an exec to its end: (stdout, stderr, exit).
    pub async fn output(
        &self,
        token: &str,
        exec: &str,
        limit: Duration,
    ) -> (String, String, Value) {
        use base64::Engine;
        let (mut out, mut err, mut after) = (Vec::new(), Vec::new(), 0u64);
        let t0 = Instant::now();
        loop {
            let (s, v) = self
                .call(
                    token,
                    &format!("/api/v1/machines/execs/{exec}/output?after={after}&wait_ms=1000"),
                    None,
                )
                .await;
            assert_eq!(s, 200, "{v}");
            for c in v["chunks"].as_array().unwrap() {
                if c["kind"] == "data" {
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(c["data"].as_str().unwrap())
                        .unwrap();
                    if c["stream"] == "stdout" {
                        out.extend(bytes)
                    } else {
                        err.extend(bytes)
                    }
                }
            }
            after = v["next"].as_u64().unwrap();
            if !v["exit"].is_null() {
                return (
                    String::from_utf8_lossy(&out).into(),
                    String::from_utf8_lossy(&err).into(),
                    v["exit"].clone(),
                );
            }
            assert!(
                t0.elapsed() < limit,
                "exec never ended; got {:?}",
                String::from_utf8_lossy(&out)
            );
        }
    }
}

/// Enrol a machine directory with `code`; the CLI's exit code.
pub fn enrol(server: &str, code: &str, dir: &Path) -> i32 {
    Command::new(BIN)
        .args([
            "machine", "enroll", "--server", server, "--name", "test", "--direct", "--dir",
        ])
        .arg(dir)
        .env("CALTERNAL_ENROLL_CODE", code)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .code()
        .unwrap_or(-1)
}

/// Run the machine role with the process backend.
pub fn machine(dir: &Path, max: u32) -> Proc {
    Proc(
        Command::new(BIN)
            .args([
                "machine",
                "run",
                "--backend",
                "direct",
                "--max-sandboxes",
                &max.to_string(),
                "--dir",
            ])
            .arg(dir)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

/// Enrolled and running machine for `token`.
pub async fn connected_machine(server: &Server, token: &str, dir: &Path, max: u32) -> Proc {
    let code = server.code(token).await;
    assert_eq!(enrol(&server.url(), &code, dir), 0);
    let m = machine(dir, max);
    wait_online(server, token, 1).await;
    m
}

/// Wait until `token` has `n` online machines.
pub async fn wait_online(server: &Server, token: &str, n: usize) {
    let t0 = Instant::now();
    loop {
        let (_, v) = server.call(token, "/api/v1/machines", None).await;
        let online = v["machines"]
            .as_array()
            .map_or(0, |a| a.iter().filter(|m| m["state"] == "online").count());
        if online >= n {
            return;
        }
        assert!(
            t0.elapsed() < Duration::from_secs(15),
            "machines not online: {v}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Wait for Podman to remove this exact test container, including stopped state.
pub async fn wait_container_removed(account: uuid::Uuid, thread: uuid::Uuid, timeout: Duration) {
    let name = calternal_machine::sandbox::podman::PodmanBackend::container_name(account, thread);
    let waited = tokio::time::timeout(timeout, async {
        loop {
            let status = tokio::process::Command::new("podman")
                .args(["container", "exists", &name])
                .status()
                .await
                .unwrap();
            match status.code() {
                Some(1) => return,
                Some(0) => tokio::time::sleep(Duration::from_millis(100)).await,
                _ => panic!("could not check test container: {status}"),
            }
        }
    })
    .await;
    assert!(waited.is_ok(), "container {name} was retained");
}
