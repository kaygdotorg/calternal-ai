//! Disposable signing and HTTP fixtures for release verification and drain tests.
use super::*;
use std::sync::Arc;
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpListener,
};

pub(crate) struct Mock {
    pub(crate) origin: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Mock {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) async fn mock(
    response: impl Fn(&str, &[u8]) -> Option<Vec<u8>> + Send + Sync + 'static,
) -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let response = Arc::new(response);
    let task = tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let response = response.clone();
            tokio::spawn(async move {
                let mut stream = BufReader::new(stream);
                let mut first = String::new();
                stream.read_line(&mut first).await.unwrap();
                let Some(path) = first.split_whitespace().nth(1) else {
                    return;
                };
                let mut len = 0;
                loop {
                    let mut line = String::new();
                    if stream.read_line(&mut line).await.unwrap() == 0 || line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        len = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut body = vec![0; len];
                stream.read_exact(&mut body).await.unwrap();
                let reply = response(path, &body);
                let status = if reply.is_some() {
                    "200 OK"
                } else {
                    "404 Not Found"
                };
                let reply = reply.unwrap_or_default();
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    reply.len()
                );
                let stream = stream.get_mut();
                let _ = stream.write_all(header.as_bytes()).await;
                let _ = stream.write_all(&reply).await;
            });
        }
    });
    Mock { origin, task }
}

const VERSION: &str = "test-build-2";
const PLATFORM: &str = "linux-x86_64";
pub(crate) const OLD: &[u8] = b"old program";
pub(crate) const NEW: &[u8] = b"new program";

struct Fixture {
    key: String,
    manifest: Vec<u8>,
    signature: Vec<u8>,
}

fn signed(dir: &Path) -> Fixture {
    let private = dir.join("dev-only-release-key");
    let ssh_keygen = |args: &[&std::ffi::OsStr]| {
        assert!(
            std::process::Command::new("ssh-keygen")
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .unwrap()
                .success()
        );
    };
    ssh_keygen(&[
        "-q".as_ref(),
        "-t".as_ref(),
        "ed25519".as_ref(),
        "-N".as_ref(),
        "".as_ref(),
        "-f".as_ref(),
        private.as_os_str(),
    ]);
    let key: String = std::fs::read_to_string(private.with_extension("pub"))
        .unwrap()
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let digest: String = ring::digest::digest(&SHA256, NEW)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let manifest = format!(
        "calternal-machine-release-v1\n{VERSION}\n{PLATFORM}\n{}\n{digest}\n",
        NEW.len()
    );
    let file = dir.join("manifest");
    std::fs::write(&file, &manifest).unwrap();
    ssh_keygen(&[
        "-Y".as_ref(),
        "sign".as_ref(),
        "-n".as_ref(),
        "calternal-machine-release-v1".as_ref(),
        "-f".as_ref(),
        private.as_os_str(),
        file.as_os_str(),
    ]);
    Fixture {
        key,
        manifest: manifest.into_bytes(),
        signature: std::fs::read(file.with_extension("sig")).unwrap(),
    }
}

async fn release_mock(fixture: Fixture, executable: &[u8]) -> (Mock, String) {
    let prefix = format!("{}/{VERSION}/{PLATFORM}", release::PATH);
    let version = format!("{}/version", release::PATH);
    let executable = executable.to_owned();
    let server = mock(move |path, _| {
        if path == version {
            return Some(format!("{VERSION}\n").into_bytes());
        }
        match path.strip_prefix(&prefix) {
            Some("/manifest") => Some(fixture.manifest.clone()),
            Some("/manifest.sig") => Some(fixture.signature.clone()),
            Some("/calternal") => Some(executable.clone()),
            _ => None,
        }
    })
    .await;
    (server, fixture.key)
}

pub(crate) async fn staged(dir: &Path, target: &Path) -> Staged {
    let (server, key) = release_mock(signed(dir), NEW).await;
    check(
        &Link::new(&server.origin).unwrap(),
        "old-build",
        PLATFORM,
        &key,
        target,
    )
    .await
    .unwrap()
    .unwrap()
}

#[tokio::test]
async fn verifies_signature_platform_size_and_digest_before_publishing() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("calternal");
    std::fs::write(&target, OLD).unwrap();

    let fixture = signed(tmp.path());
    let manifest = fixture.manifest.clone();
    assert!(parse(&manifest, VERSION, "macos-aarch64").is_err());
    let mut changed = manifest.clone();
    changed[0] = b'x';
    assert!(verify(&fixture.key, &changed, &fixture.signature).is_err());

    let (server, key) = release_mock(fixture, b"BAD program").await;
    let link = Link::new(&server.origin).unwrap();
    assert!(
        check(&link, VERSION, PLATFORM, &key, &target)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        check(&link, "old-build", PLATFORM, &key, &target)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&target).unwrap(), OLD);

    let other = tempfile::tempdir().unwrap();
    let replacement = staged(other.path(), &target).await;
    assert_eq!(std::fs::read(&target).unwrap(), OLD);
    assert_eq!(replacement.publish().unwrap(), target);
    assert_eq!(std::fs::read(&target).unwrap(), NEW);
}
