//! Direct workspace processes on a machine its account owner marked direct.
//!
//! Each thread gets `<root>/accounts/<account-id>/threads/<thread-id>/work`; each exec runs there in its own
//! process group with its space-scoped `HOME`, machine login directory, and requested environment. Freeze, resume and stop signal the
//! process groups (`SIGSTOP`, `SIGCONT`, `SIGKILL`).
//!
//! Direct mode provides no isolation: processes share the machine user's file system, network
//! and resources, and resource limits are not enforced. If the machine role is
//! killed with `SIGKILL`, running execs are orphaned rather than stopped.

use std::collections::HashSet;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rustix::process::{Pid, Signal, kill_process_group, test_kill_process_group};

use super::{Backend, BoxFuture, Exec, ExecRequest, Sandbox, SandboxError, SandboxResult};
use crate::protocol::SandboxSpec;

/// Starts process sandboxes under one root directory.
pub struct DirectBackend {
    root: PathBuf,
    cli_dir: Option<PathBuf>,
}

impl DirectBackend {
    /// Workspaces and persistent homes live under `root/accounts/<account-id>`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            cli_dir: None,
        }
    }

    /// Supply the machine-owned directory containing the operation CLI alias.
    /// Cost: one path assignment. The machine role owns the directory lifetime.
    pub(crate) fn with_cli_dir(mut self, dir: Option<PathBuf>) -> Self {
        self.cli_dir = dir;
        self
    }
}

impl Backend for DirectBackend {
    fn initialize(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            match tokio::fs::read_dir(self.root.join("workers")).await {
                Ok(mut entries) => {
                    if entries
                        .next_entry()
                        .await
                        .map_err(SandboxError::new)?
                        .is_some()
                    {
                        Err(SandboxError("restore Podman to confirm cleanup of prior plugin workers before reconnecting".into()))
                    } else {
                        Ok(())
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(SandboxError::new(e)),
            }
        })
    }
    fn name(&self) -> &'static str {
        "direct"
    }

    fn discard(&self, account: uuid::Uuid, thread: uuid::Uuid) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            let dir = self
                .root
                .join("accounts")
                .join(account.to_string())
                .join("threads")
                .join(thread.to_string());
            for path in [dir, self.root.join("workers").join(thread.to_string())] {
                match tokio::fs::remove_dir_all(path).await {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(SandboxError::new(e)),
                }
            }
            Ok(())
        })
    }
    fn erase(
        &self,
        account: uuid::Uuid,
        thread: Option<uuid::Uuid>,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        let root = self.root.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                use calternal_fsio::beneath::Folder;
                let remove = || -> std::io::Result<()> {
                    let root = Folder::open(&root)?;
                    let accounts = root.child("accounts", false)?;
                    let account = account.to_string();
                    match thread {
                        Some(thread) => accounts
                            .child(&account, false)?
                            .child("threads", false)?
                            .remove_tree(&thread.to_string()),
                        None => accounts.remove_tree(&account),
                    }
                };
                match remove() {
                    Ok(()) => Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(SandboxError::new(error)),
                }
            })
            .await
            .map_err(SandboxError::new)?
        })
    }
    fn start<'a>(
        &'a self,
        spec: &'a SandboxSpec,
    ) -> BoxFuture<'a, SandboxResult<Arc<dyn Sandbox>>> {
        Box::pin(async move {
            let result = async {
                let account = self.root.join("accounts").join(spec.account_id.to_string());
                let dir = if spec.worker.is_some() {
                    self.root.join("workers").join(spec.thread_id.to_string())
                } else {
                    account.join("threads").join(spec.thread_id.to_string())
                };
                if spec.shared_provider_sponsor.is_some() {
                    return Err(SandboxError(
                        "a shared provider login requires sandbox isolation".into(),
                    ));
                }
                let (home, login) = super::provider_home::prepare(account, &dir, spec).await?;
                let work = dir.join("work");
                tokio::fs::create_dir_all(&work).await.map_err(|e| {
                    SandboxError(format!("cannot create work directory: {}", e.kind()))
                })?;
                if let Some(worker) = &spec.worker {
                    super::worker::prepare(&work, worker).await?;
                }
                Ok(Arc::new(DirectSandbox {
                    dir,
                    cli_dir: self.cli_dir.clone(),
                    work: spec.code_directory.clone().unwrap_or(work),
                    worker: spec.worker.is_some(),
                    home,
                    login,
                    provider_login: spec.provider_login,
                    groups: Mutex::new(HashSet::new()),
                }) as Arc<dyn Sandbox>)
            }
            .await;
            if result.is_err() && spec.worker.is_some() {
                let _ = tokio::fs::remove_dir_all(
                    self.root.join("workers").join(spec.thread_id.to_string()),
                )
                .await;
            }
            result
        })
    }
}

/// One thread's process sandbox.
pub struct DirectSandbox {
    dir: PathBuf,
    cli_dir: Option<PathBuf>,
    work: PathBuf,
    home: PathBuf,
    login: PathBuf,
    provider_login: bool,
    worker: bool,
    /// Process groups started here; pruned lazily when they no longer exist.
    groups: Mutex<HashSet<i32>>,
}

impl DirectSandbox {
    fn signal_all(&self, signal: Signal) {
        let groups = self.groups.lock().expect("groups");
        for &g in groups.iter() {
            if let Some(pid) = Pid::from_raw(g) {
                let _ = kill_process_group(pid, signal);
            }
        }
    }
}

impl Sandbox for DirectSandbox {
    fn exec(&self, request: ExecRequest) -> BoxFuture<'_, SandboxResult<Exec>> {
        Box::pin(async move {
            let Some((program, args)) = request.argv.split_first() else {
                return Err(SandboxError("empty command".into()));
            };
            let mut cmd = tokio::process::Command::new(program);
            request.validate_env_names()?;
            let mut paths = vec![self.home.join(".local/bin")];
            if let Some(dir) = &self.cli_dir {
                paths.push(dir.clone());
            }
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            ));
            let path = std::env::join_paths(paths).map_err(SandboxError::new)?;
            cmd.args(args)
                .current_dir(&self.work)
                .env_clear()
                .env("PATH", path)
                .env("HOME", &self.home)
                .envs(request.env.iter().map(|(k, v)| (k, v)))
                .env("CALTERNAL_PROVIDER_LOGIN_DIR", &self.login);
            if self.provider_login {
                cmd.env("CALTERNAL_PROVIDER_LOGIN", "1");
            }
            if self.worker {
                cmd.env_clear().env("LANG", "C.UTF-8");
            }
            cmd.as_std_mut().process_group(0);
            let (exec, pid) =
                super::spawn_streaming(cmd, request.stdin, request.interactive, self.worker)?;
            let mut groups = self.groups.lock().expect("groups");
            groups
                .retain(|&g| Pid::from_raw(g).is_some_and(|p| test_kill_process_group(p).is_ok()));
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
    use uuid::Uuid;

    fn spec() -> SandboxSpec {
        SandboxSpec {
            space_id: Some(calternal_types::SpaceId::new()),
            provider_login: false,
            shared_provider_sponsor: None,
            shared_tools: None,
            code_directory: None,
            worker: None,
            account_id: Uuid::new_v4(),
            thread_id: Uuid::new_v4(),
            image: None,
            limits: Limits::default(),
        }
    }

    fn argv(a: &[&str]) -> ExecRequest {
        ExecRequest {
            interactive: false,
            argv: a.iter().map(|s| s.to_string()).collect(),
            stdin: String::new(),
            env: vec![("ONLY".into(), "this".into())],
        }
    }

    #[tokio::test]
    async fn runs_in_own_directory_with_clean_environment() {
        let dir = tempfile::tempdir().unwrap();
        let backend = DirectBackend::new(dir.path());
        let (a, b) = (spec(), spec());
        let sa = backend.start(&a).await.unwrap();
        let sb = backend.start(&b).await.unwrap();
        let (out, _, exit) = collect(
            sa.exec(argv(&["sh", "-c", "pwd; env | sort"]))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(exit.code, Some(0));
        let out = String::from_utf8(out).unwrap();
        let wd = dir
            .path()
            .join("accounts")
            .join(a.account_id.to_string())
            .join("threads")
            .join(a.thread_id.to_string())
            .join("work");
        assert!(out.starts_with(&format!("{}\n", wd.display())), "{out}");
        assert!(out.contains("ONLY=this"));
        assert!(!out.contains("CARGO"), "machine environment leaked: {out}");
        let (out_b, _, _) = collect(sb.exec(argv(&["pwd"])).await.unwrap()).await;
        assert_ne!(
            String::from_utf8(out_b).unwrap().trim(),
            wd.display().to_string()
        );
    }

    #[tokio::test]
    async fn stop_kills_the_whole_group_and_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let backend = DirectBackend::new(dir.path());
        let s = spec();
        let sb = backend.start(&s).await.unwrap();
        // A grandchild that would outlive its parent shell without group kill.
        let mut exec = sb
            .exec(argv(&["sh", "-c", "sleep 30 & echo started; wait"]))
            .await
            .unwrap();
        assert_eq!(
            exec.events.recv().await,
            Some(OutputEvent::Data(Stream::Stdout, b"started\n".to_vec()))
        );
        sb.stop().await.unwrap();
        let exit = loop {
            match exec.events.recv().await.unwrap() {
                OutputEvent::Exit(e) => break e,
                OutputEvent::Data(..) => {}
            }
        };
        assert_eq!(exit.signal, Some(9));
        assert!(
            !dir.path()
                .join("accounts")
                .join(s.account_id.to_string())
                .join("threads")
                .join(s.thread_id.to_string())
                .exists()
        );
    }

    #[tokio::test]
    async fn erasure_keeps_other_accounts_and_never_traverses_sandbox_symlinks() {
        let sandbox = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let target = spec();
        let other = spec();
        let backend = DirectBackend::new(sandbox.path());
        let path = |id: uuid::Uuid| sandbox.path().join("accounts").join(id.to_string());
        let workspace = path(target.account_id)
            .join("threads")
            .join(target.thread_id.to_string());
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::create_dir_all(path(target.account_id).join("home")).unwrap();
        std::fs::create_dir_all(path(other.account_id).join("home")).unwrap();
        let personal_file = outside.path().join("must-survive");
        std::fs::write(&personal_file, "person's own file").unwrap();
        std::os::unix::fs::symlink(outside.path(), workspace.join("link")).unwrap();
        backend
            .erase(target.account_id, Some(target.thread_id))
            .await
            .unwrap();
        assert!(!workspace.exists());
        assert!(path(target.account_id).join("home").exists());
        backend.erase(target.account_id, None).await.unwrap();
        backend.erase(target.account_id, None).await.unwrap();
        assert!(!path(target.account_id).exists());
        assert!(path(other.account_id).exists());
        assert!(personal_file.exists());
        std::os::unix::fs::symlink(outside.path(), path(target.account_id)).unwrap();
        assert!(
            backend
                .erase(target.account_id, Some(target.thread_id))
                .await
                .is_err()
        );
        assert!(personal_file.exists());
    }

    #[tokio::test]
    async fn freeze_pauses_and_resume_continues() {
        let dir = tempfile::tempdir().unwrap();
        let sb = DirectBackend::new(dir.path()).start(&spec()).await.unwrap();
        let mut exec = sb
            .exec(argv(&[
                "sh",
                "-c",
                "while true; do echo t; sleep 0.05; done",
            ]))
            .await
            .unwrap();
        exec.events.recv().await.unwrap();
        sb.freeze().await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        while exec.events.try_recv().is_ok() {}
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            exec.events.try_recv().is_err(),
            "frozen sandbox kept writing"
        );
        sb.resume().await.unwrap();
        let next =
            tokio::time::timeout(std::time::Duration::from_secs(2), exec.events.recv()).await;
        assert!(matches!(next, Ok(Some(OutputEvent::Data(..)))));
        sb.stop().await.unwrap();
    }

    #[tokio::test]
    async fn slow_reader_blocks_the_writer() {
        let dir = tempfile::tempdir().unwrap();
        let sb = DirectBackend::new(dir.path()).start(&spec()).await.unwrap();
        // Writes 64 MiB; with nobody reading it must block well before finishing.
        let exec = sb
            .exec(argv(&[
                "sh",
                "-c",
                "head -c 67108864 /dev/zero; echo done >&2",
            ]))
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let mut exec = exec;
        let mut seen = 0usize;
        while let Ok(OutputEvent::Data(_, b)) = exec.events.try_recv() {
            seen += b.len();
        }
        assert!(
            seen <= super::super::OUTPUT_CHANNEL * super::super::CHUNK_BYTES,
            "{seen}"
        );
        sb.stop().await.unwrap();
    }
}
