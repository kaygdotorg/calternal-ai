//! The shared harness for the end-to-end tests: it starts the real `host-spike` binary on a temporary directory.
//! Every call goes over HTTP to `POST /call`; the tests then read the files on
//! disk and the host state (`state/<space>/host.db`) directly.
#![allow(dead_code)] // each test file uses part of the harness
use calternal_types::{AccountId, SpaceId};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

const BIN: &str = env!("CARGO_BIN_EXE_host-spike");

/// A running `host-spike serve` on its own root, killed on drop.
pub struct Server {
    child: Child,
    addr: String,
    pub root: PathBuf,
    pub space: SpaceId,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Start `host-spike serve` and read the address it bound.
pub fn spawn(root: &Path) -> (Child, String) {
    let mut child = Command::new(BIN)
        .args(["serve", "--root"])
        .arg(root)
        .stdout(Stdio::piped())
        .spawn()
        .expect("start serve");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let addr = line
        .trim()
        .strip_prefix("listening http://")
        .expect("listening line")
        .to_owned();
    (child, addr)
}

impl Server {
    /// Stop the server and start it again on the same root.
    pub fn restart(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        (self.child, self.addr) = spawn(&self.root);
    }

    pub fn start(root: &Path, space: SpaceId, members: &[(AccountId, &str, &str)]) -> Self {
        let mut init = Command::new(BIN);
        init.args(["init", "--root"])
            .arg(root)
            .args(["--space", &space.to_string()]);
        for (p, n, r) in members {
            init.args(["--member", &format!("{p}:{n}:{r}")]);
        }
        let out = init.output().expect("run init");
        assert!(
            out.status.success(),
            "init: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let (child, addr) = spawn(root);
        Self {
            child,
            addr,
            root: root.to_owned(),
            space,
        }
    }

    /// One call: `(http status, body)`.
    pub fn call(
        &self,
        person: AccountId,
        plugin: &str,
        attempt: Option<&str>,
        name: &str,
        input: Value,
    ) -> (u16, Value) {
        let body = json!({ "name": name, "input": input }).to_string();
        let mut s = TcpStream::connect(&self.addr).unwrap();
        let attempt = attempt
            .map(|a| format!("x-attempt: {a}\r\n"))
            .unwrap_or_default();
        write!(
            s,
            "POST /call HTTP/1.1\r\nhost: spike\r\ncontent-type: application/json\r\nconnection: close\r\n\
             x-person: {person}\r\nx-space: {}\r\nx-plugin: {plugin}\r\n{attempt}content-length: {}\r\n\r\n{body}",
            self.space,
            body.len()
        )
        .unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).unwrap();
        let status: u16 = resp[9..12].parse().unwrap();
        let (_, json) = resp.split_once("\r\n\r\n").unwrap();
        (
            status,
            serde_json::from_str(json).unwrap_or_else(|_| panic!("not json: {json}")),
        )
    }

    pub fn ok(&self, person: AccountId, plugin: &str, name: &str, input: Value) -> Value {
        let (status, v) = self.call(person, plugin, None, name, input);
        assert_eq!(status, 200, "{name}: {v}");
        v["result"].clone()
    }

    pub fn files(&self) -> PathBuf {
        self.root.join("spaces").join(self.space.to_string())
    }

    pub fn ledger(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(
            self.root
                .join("state")
                .join(self.space.to_string())
                .join("host.db"),
        )
        .unwrap()
    }
}

pub fn declaration(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("plugins")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}
