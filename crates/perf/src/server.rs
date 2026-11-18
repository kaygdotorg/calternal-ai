//! Own only disposable child processes and three lane-owned storage roots.
use crate::http::{Http, ORIGIN, Result};
use std::{
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
pub(crate) struct Server {
    child: Child,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Server {
    pub(crate) fn start(
        bin: &Path,
        work: &Path,
        plugins: &Path,
        test_plugins: Option<&Path>,
        startup_timeout_seconds: u64,
    ) -> Result<(Self, Http, f64)> {
        std::fs::create_dir_all(work)?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        drop(listener);
        let _ = std::fs::remove_file(work.join("appdata/startup.json"));
        let log = std::fs::File::create(work.join("server.log"))?;
        let t = Instant::now();
        let mut command = Command::new(bin);
        command.arg("serve");
        if let Some(root) = test_plugins {
            command.arg("--test-plugin-root").arg(root);
        }
        let child = command
            .args([
                "--port",
                &port.to_string(),
                "--origin",
                ORIGIN,
                "--create",
                "--installation-id",
                "00000000-0000-0000-0000-000000000001",
            ])
            .arg("--appdata")
            .arg(work.join("appdata"))
            .arg("--userdata-ssd")
            .arg(work.join("userdata-ssd"))
            .arg("--userdata-hdd")
            .arg(work.join("userdata-hdd"))
            .arg("--plugin-root")
            .arg(plugins)
            .env("RUST_LOG", "warn")
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()?;
        let mut server = Self { child };
        let http = Http::new(port)?;
        loop {
            if http.request("/api/v1/auth/status", None).is_ok() {
                return Ok((server, http, t.elapsed().as_secs_f64() * 1000.));
            }
            // Observe slow startup without relaxing the independent 300 ms budget.
            if server.child.try_wait()?.is_some()
                || t.elapsed() > Duration::from_secs(startup_timeout_seconds)
            {
                return Err(
                    format!("startup failed; see {}", work.join("server.log").display()).into(),
                );
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    /// Child identity for the separate syscall diagnostic; one field read, no I/O.
    pub(crate) fn pid(&self) -> u32 {
        self.child.id()
    }
    pub(crate) fn writers(&self) -> Result<usize> {
        let mut writers = 0;
        for entry in std::fs::read_dir(format!("/proc/{}/task", self.child.id()))? {
            let path = entry?.path().join("comm");
            if std::fs::read_to_string(path)
                .unwrap_or_default()
                .starts_with("store-")
            {
                writers += 1;
            }
        }
        Ok(writers)
    }
    pub(crate) fn rss(&self) -> Result<f64> {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id()))?;
        let kb = status
            .lines()
            .find_map(|l| l.strip_prefix("VmRSS:"))
            .ok_or("missing VmRSS")?
            .split_whitespace()
            .next()
            .ok_or("missing RSS value")?
            .parse::<f64>()?;
        Ok(kb / 1024.)
    }
}
