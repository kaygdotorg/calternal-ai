//! Plain-process sandboxes, for tests and development.
//!
//! Each thread gets `<root>/<thread-id>/work`; each exec runs there in its own
//! process group with an empty environment apart from `PATH`, `HOME` (the work
//! directory) and what the exec asks for. Freeze, resume and stop signal the
//! process groups (`SIGSTOP`, `SIGCONT`, `SIGKILL`).
//!
//! This is not isolation: processes share the machine user's file system, network
//! and resources, and resource limits are not enforced. If the machine role is
//! killed with `SIGKILL`, running execs are orphaned rather than stopped.

use std::collections::HashSet;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};
use uuid::Uuid;

use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::SandboxSpec;

/// Starts process sandboxes under one root directory.
pub struct ProcessBackend {
    root: PathBuf,
}

impl ProcessBackend {
    /// Sandboxes live under `root/<thread-id>`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl Backend for ProcessBackend {
    fn name(&self) -> &'static str {
        "process"
    }

    fn start<'a>(&'a self, spec: &'a SandboxSpec) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let dir = self.root.join(spec.thread_id.to_string());
            let work = dir.join("work");
            tokio::fs::create_dir_all(&work)
                .await
                .map_err(|e| SandboxError(format!("cannot create work directory: {}", e.kind())))?;
            Ok(Arc::new(ProcessSandbox {
                thread_id: spec.thread_id,
                dir,
                work,
                groups: Mutex::new(HashSet::new()),
            }) as Arc<dyn Sandbox>)
        })
    }
}

/// One thread's process sandbox.
pub struct ProcessSandbox {
    thread_id: Uuid,
    dir: PathBuf,
    work: PathBuf,
    /// Process groups started here; pruned lazily when they no longer exist.
    groups: Mutex<HashSet<i32>>,
}

impl ProcessSandbox {
    fn signal_all(&self, signal: Signal) {
        let groups = self.groups.lock().expect("groups");
        for &g in groups.iter() {
            if let Some(pid) = Pid::from_raw(g) {
                let _ = kill_process_group(pid, signal);
            }
        }
    }
}

impl Sandbox for ProcessSandbox {
    fn thread_id(&self) -> Uuid {
        self.thread_id
    }

    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            let Some((program, args)) = request.argv.split_first() else {
                return Err(SandboxError("empty command".into()));
            };
            let mut cmd = tokio::process::Command::new(program);
            cmd.args(args)
                .current_dir(&self.work)
                .env_clear()
                .env("PATH", std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()))
                .env("HOME", &self.work)
                .envs(request.env.iter().map(|(k, v)| (k, v)));
            cmd.as_std_mut().process_group(0);
            let (exec, pid) = super::spawn_streaming(cmd)?;
            let mut groups = self.groups.lock().expect("groups");
            groups.retain(|&g| Pid::from_raw(g).is_some_and(|p| test_kill_process_group(p).is_ok()));
            if let Some(pid) = pid {
                groups.insert(pid as i32);
            }
            Ok(exec)
        })
    }

    fn freeze(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            self.signal_all(Signal::STOP);
            Ok(())
        })
    }

    fn resume(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            self.signal_all(Signal::CONT);
            Ok(())
        })
    }

    fn stop(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            self.signal_all(Signal::KILL);
            match tokio::fs::remove_dir_all(&self.dir).await {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(SandboxError(format!("cannot remove sandbox: {}", e.kind()))),
            }
        })
    }

    fn release(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            // Stopped groups ignore SIGKILL until continued on some kernels' views
            // of job control; continue first so the kill is delivered at once.
            self.signal_all(Signal::CONT);
            self.signal_all(Signal::KILL);
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{Limits, Stream};
    use crate::sandbox::{OutputEvent, collect};

    fn spec() -> SandboxSpec {
        SandboxSpec {
            thread_id: Uuid::new_v4(),
            image: None,
            limits: Limits::default(),
        }
    }

    fn argv(a: &[&str]) -> ExecRequest {
        ExecRequest {
            argv: a.iter().map(|s| s.to_string()).collect(),
            env: vec![("ONLY".into(), "this".into())],
        }
    }

    #[tokio::test]
    async fn runs_in_own_directory_with_clean_environment() {
        let dir = tempfile::tempdir().unwrap();
        let backend = ProcessBackend::new(dir.path());
        let (a, b) = (spec(), spec());
        let sa = backend.start(&a).await.unwrap();
        let sb = backend.start(&b).await.unwrap();
        let (out, _, exit) = collect(sa.exec(argv(&["sh", "-c", "pwd; env | sort"])).await.unwrap()).await;
        assert_eq!(exit.code, Some(0));
        let out = String::from_utf8(out).unwrap();
        let wd = dir.path().join(a.thread_id.to_string()).join("work");
        assert!(out.starts_with(&format!("{}\n", wd.display())), "{out}");
        assert!(out.contains("ONLY=this"));
        assert!(!out.contains("CARGO"), "machine environment leaked: {out}");
        let (out_b, _, _) = collect(sb.exec(argv(&["pwd"])).await.unwrap()).await;
        assert_ne!(String::from_utf8(out_b).unwrap().trim(), wd.display().to_string());
    }

    #[tokio::test]
    async fn stop_kills_the_whole_group_and_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let backend = ProcessBackend::new(dir.path());
        let s = spec();
        let sb = backend.start(&s).await.unwrap();
        // A grandchild that would outlive its parent shell without group kill.
        let mut exec = sb
            .exec(argv(&["sh", "-c", "sleep 30 & echo started; wait"]))
            .await
            .unwrap();
        assert_eq!(exec.events.recv().await, Some(OutputEvent::Data(Stream::Stdout, b"started\n".to_vec())));
        sb.stop().await.unwrap();
        let exit = loop {
            match exec.events.recv().await.unwrap() {
                OutputEvent::Exit(e) => break e,
                OutputEvent::Data(..) => {}
            }
        };
        assert_eq!(exit.signal, Some(9));
        assert!(!dir.path().join(s.thread_id.to_string()).exists());
    }

    #[tokio::test]
    async fn freeze_pauses_and_resume_continues() {
        let dir = tempfile::tempdir().unwrap();
        let sb = ProcessBackend::new(dir.path()).start(&spec()).await.unwrap();
        let mut exec = sb
            .exec(argv(&["sh", "-c", "while true; do echo t; sleep 0.05; done"]))
            .await
            .unwrap();
        exec.events.recv().await.unwrap();
        sb.freeze().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        while exec.events.try_recv().is_ok() {}
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(exec.events.try_recv().is_err(), "frozen sandbox kept writing");
        sb.resume().await.unwrap();
        let next = tokio::time::timeout(std::time::Duration::from_secs(2), exec.events.recv()).await;
        assert!(matches!(next, Ok(Some(OutputEvent::Data(..)))));
        sb.stop().await.unwrap();
    }

    #[tokio::test]
    async fn slow_reader_blocks_the_writer() {
        let dir = tempfile::tempdir().unwrap();
        let sb = ProcessBackend::new(dir.path()).start(&spec()).await.unwrap();
        // Writes 64 MiB; with nobody reading it must block well before finishing.
        let exec = sb
            .exec(argv(&["sh", "-c", "head -c 67108864 /dev/zero; echo done >&2"]))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut exec = exec;
        let mut seen = 0usize;
        while let Ok(OutputEvent::Data(_, b)) = exec.events.try_recv() {
            seen += b.len();
        }
        assert!(seen <= super::super::OUTPUT_CHANNEL * super::super::CHUNK_BYTES, "{seen}");
        sb.stop().await.unwrap();
    }
}
