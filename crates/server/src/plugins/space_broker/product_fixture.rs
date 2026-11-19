use calternal_machine::{
    link::Link,
    protocol::SandboxSpec,
    sandbox::{Backend, BoxFuture, Sandbox, SandboxResult, direct::DirectBackend},
};
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
pub(super) struct FixtureBackend(DirectBackend);

impl FixtureBackend {
    pub(super) fn new(root: &Path) -> Self {
        Self(DirectBackend::new(root))
    }
}

impl Backend for FixtureBackend {
    fn name(&self) -> &'static str {
        "fixture"
    }

    fn discard(&self, account: Uuid, thread: Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        self.0.discard(account, thread)
    }

    fn erase(&self, account: Uuid, thread: Option<Uuid>) -> BoxFuture<'_, SandboxResult<()>> {
        self.0.erase(account, thread)
    }

    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        self.0.start(spec)
    }
}

pub(super) async fn worker_pid(work: &Path) -> String {
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

// TLS termination stays outside the product router, as in deployment. Dropping
// bytes in both directions still leaves both real TCP sockets open.
const RELAY: &str = r#"
const fs = require('node:fs');
const net = require('node:net');
const tls = require('node:tls');
const port = Number(process.argv[1]);
const server = tls.createServer({
  key: fs.readFileSync('server.key'), cert: fs.readFileSync('server.pem')
}, downstream => {
  const upstream = net.connect(port, '127.0.0.1');
  const blocked = () => fs.existsSync('stall');
  for (const [source, target] of [[upstream, downstream], [downstream, upstream]]) {
    source.on('data', bytes => { if (!blocked()) target.write(bytes); });
    source.on('end', () => { if (!blocked()) target.end(); });
    source.on('error', () => { if (!blocked()) target.destroy(); });
  }
});
server.listen(0, '127.0.0.1', () => console.log(server.address().port));
"#;

pub(super) struct TlsRelay {
    pub(super) link: Link,
    pub(super) stall: PathBuf,
    _process: Child,
}

impl TlsRelay {
    pub(super) async fn start(root: &Path, server: SocketAddr) -> Self {
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
        let port: u16 = port.trim().parse().unwrap();
        let ca = tokio::fs::read(root.join("ca.pem")).await.unwrap();
        let link = Link::with_space_test_ca(
            &format!("https://127.0.0.1:{port}"),
            reqwest::Certificate::from_pem(&ca).unwrap(),
        )
        .unwrap();
        Self {
            link,
            stall: root.join("stall"),
            _process: process,
        }
    }
}
