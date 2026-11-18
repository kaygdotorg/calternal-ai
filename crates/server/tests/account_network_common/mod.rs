//! HTTPS issuer and SMTP fixture processes with isolated local certificates.
#![allow(dead_code)]
use crate::live::wait_json;
use crate::machines::Proc;
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

pub struct Service {
    _child: Proc,
    dir: TempDir,
    pub port: u16,
}

impl Service {
    pub fn start(mode: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/account_network_common/service.py");
        let mut child = Proc(
            Command::new("python3")
                .arg(script)
                .arg(mode)
                .arg(dir.path())
                .spawn()
                .unwrap(),
        );
        let port = wait_json(&mut child, &dir.path().join("ready.json"))
            .as_u64()
            .unwrap()
            .try_into()
            .unwrap();
        Self {
            _child: child,
            dir,
            port,
        }
    }

    pub fn ca(&self) -> PathBuf {
        self.dir.path().join("ca.pem")
    }

    pub fn mail(&mut self, index: usize, to: &str, subject: &str) -> String {
        let path = self.dir.path().join(format!("mail-{index}.json"));
        let mail = wait_json(&mut self._child, &path);
        assert_eq!(mail["recipient"], format!("TO:<{to}>"));
        let body = mail["body"].as_str().unwrap();
        assert!(
            body.lines()
                .any(|line| line == format!("Subject: {subject}"))
        );
        body.to_owned()
    }
}
