#[path = "product_http.rs"]
mod product_http;

use super::*;
use calternal_machine::{
    link::Link,
    protocol::SandboxSpec,
    sandbox::{
        Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxResult, direct::DirectBackend,
    },
};
use calternal_plugin_host::space_channel::MAX_TICKET_RESULT_FRAME;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};
use uuid::Uuid;

// Keep production's direct-placement restriction intact. Only this fixture
// delegates an isolated test assignment to the direct backend.
struct FixtureBackend {
    direct: DirectBackend,
    node: String,
}

impl FixtureBackend {
    fn new(root: &Path, node: String) -> Self {
        Self {
            direct: DirectBackend::new(root),
            node,
        }
    }
}

impl Backend for FixtureBackend {
    fn name(&self) -> &'static str {
        "fixture"
    }

    fn discard(&self, account: Uuid, thread: Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.discard(account, thread)
    }

    fn erase(&self, account: Uuid, thread: Option<Uuid>) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.erase(account, thread)
    }

    fn supports_small_plugin_vm(&self) -> bool {
        true
    }

    fn start_space<'a>(
        &'a self,
        spec: &'a SandboxSpec,
        _checkpoint: Option<u64>,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.start(spec)
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            Ok(Arc::new(FixtureSandbox {
                direct: self.direct.start(spec).await?,
                node: self.node.clone(),
            }) as Arc<dyn Sandbox>)
        })
    }
}

// The worker clears PATH. Supply the shell's pinned absolute Node program,
// then let DirectSandbox start, stream and reap the process normally.
struct FixtureSandbox {
    direct: Arc<dyn Sandbox>,
    node: String,
}

impl Sandbox for FixtureSandbox {
    fn exec(&self, mut request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        assert_eq!(request.argv[0], "node");
        request.argv[0] = self.node.clone();
        self.direct.exec(request)
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.freeze()
    }
    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.resume()
    }
    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.stop()
    }
    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.direct.release()
    }
}

async fn worker_pid(work: &Path) -> String {
    let mut entries = tokio::fs::read_dir("/proc").await.unwrap();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        if tokio::fs::read_link(entry.path().join("cwd"))
            .await
            .as_deref()
            .ok()
            == Some(work)
        {
            return entry.file_name().into_string().unwrap();
        }
    }
    panic!("package must be running before removing its lease renewals");
}

async fn kill_firecracker(pid: &str) {
    assert!(
        tokio::process::Command::new("kill")
            .args(["-KILL", pid])
            .status()
            .await
            .unwrap()
            .success()
    );
}

async fn launched_cold(pid: &str) -> bool {
    tokio::fs::read(format!("/proc/{pid}/cmdline"))
        .await
        .unwrap()
        .split(|byte| *byte == 0)
        .any(|arg| arg == b"--config-file")
}

async fn firecracker_pid(run: &Path) -> String {
    let mut entries = tokio::fs::read_dir("/proc").await.unwrap();
    let run = run.as_os_str().as_encoded_bytes();
    while let Some(entry) = entries.next_entry().await.unwrap() {
        if let Ok(args) = tokio::fs::read(entry.path().join("cmdline")).await
            && args.windows(run.len()).any(|window| window == run)
            && args
                .split(|b| *b == 0)
                .next()
                .is_some_and(|cmd| cmd.ends_with(b"/firecracker"))
        {
            return entry.file_name().into_string().unwrap();
        }
    }
    panic!("live plugin VMM is absent");
}

// TLS termination stays outside the product router, as in deployment. Dropping
// bytes in both directions still leaves both real TCP sockets open.
const RELAY: &str = r#"
const fs = require('node:fs');
const net = require('node:net');
const tls = require('node:tls');
const port = Number(process.argv[1]);
let next = 0;
const server = tls.createServer({
  key: fs.readFileSync('server.key'), cert: fs.readFileSync('server.pem')
}, downstream => {
  const upstream = net.connect(port, '127.0.0.1');
  const first = next++ === 0;
  const blocked = () => first && fs.existsSync('stall');
  for (const [source, target] of [[upstream, downstream], [downstream, upstream]]) {
    source.on('data', bytes => { if (!blocked()) target.write(bytes); });
    source.on('end', () => { if (!blocked()) target.end(); });
    source.on('error', () => { if (!blocked()) target.destroy(); });
  }
});
server.listen(0, '127.0.0.1', () => console.log(server.address().port + ' ' + process.execPath));
"#;

struct TlsRelay {
    link: Link,
    port: u16,
    node: String,
    stall: PathBuf,
    _process: Child,
}

impl TlsRelay {
    async fn start(root: &Path, server: SocketAddr) -> Self {
        async fn openssl(root: &Path, args: &[&str]) {
            let output = Command::new("openssl")
                .current_dir(root)
                .args(args)
                .output()
                .await
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        openssl(
            root,
            &[
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-noenc",
                "-days",
                "1",
                "-subj",
                "/CN=Disposable carrier CA",
                "-keyout",
                "ca.key",
                "-out",
                "ca.pem",
            ],
        )
        .await;
        openssl(
            root,
            &[
                "req",
                "-new",
                "-newkey",
                "rsa:2048",
                "-noenc",
                "-subj",
                "/CN=127.0.0.1",
                "-keyout",
                "server.key",
                "-out",
                "server.csr",
            ],
        )
        .await;
        tokio::fs::write(
            root.join("extensions"),
            b"subjectAltName=IP:127.0.0.1\nextendedKeyUsage=serverAuth\n",
        )
        .await
        .unwrap();
        openssl(
            root,
            &[
                "x509",
                "-req",
                "-in",
                "server.csr",
                "-CA",
                "ca.pem",
                "-CAkey",
                "ca.key",
                "-CAcreateserial",
                "-days",
                "1",
                "-extfile",
                "extensions",
                "-out",
                "server.pem",
            ],
        )
        .await;
        let mut process = Command::new("node")
            .current_dir(root)
            .args(["-e", RELAY, &server.port().to_string()])
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut port = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut port)
            .await
            .unwrap();
        let (port, node) = port.trim().split_once(' ').unwrap();
        let port: u16 = port.parse().unwrap();
        let ca = tokio::fs::read(root.join("ca.pem")).await.unwrap();
        let link = Link::with_space_test_ca(
            &format!("https://127.0.0.1:{port}"),
            reqwest::Certificate::from_pem(&ca).unwrap(),
        )
        .unwrap();
        Self {
            link,
            port,
            node: node.to_owned(),
            stall: root.join("stall"),
            _process: process,
        }
    }
}

#[path = "product_tests.rs"]
mod tests;

#[path = "package_product_tests.rs"]
mod package_tests;
