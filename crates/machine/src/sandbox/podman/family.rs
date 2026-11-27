//! Linux process-family termination for a private Podman PID namespace.
use super::{SandboxError, SandboxResult};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
use std::{os::fd::OwnedFd, path::Path};
use tokio::io::{Interest, unix::AsyncFd};

pub(super) struct Family(AsyncFd<OwnedFd>);

impl Family {
    pub(super) async fn open(pid_file: &Path) -> SandboxResult<Self> {
        let raw = tokio::fs::read_to_string(pid_file)
            .await
            .map_err(SandboxError::new)?;
        let pid = raw
            .trim()
            .parse::<i32>()
            .ok()
            .and_then(Pid::from_raw)
            .filter(|pid| pid.as_raw_nonzero().get() > 1)
            .ok_or_else(|| {
                SandboxError("Podman did not report a valid container init PID".into())
            })?;
        let fd = pidfd_open(pid, PidfdFlags::empty()).map_err(SandboxError::new)?;
        // OwnedFd keeps the descriptor alive for the entire registration.
        #[allow(deprecated)]
        let registration = AsyncFd::with_interest(fd, Interest::READABLE);
        registration.map(Self).map_err(SandboxError::new)
    }

    pub(super) async fn terminate(&self) -> SandboxResult<()> {
        // Killing the init of this private PID namespace kills the entire
        // process family. The pidfd prevents signalling a reused PID.
        match pidfd_send_signal(self.0.get_ref(), Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => {}
            Err(error) => return Err(SandboxError::new(error)),
        }
        // Linux marks PID init exited only after the namespace is drained.
        let _ = self.0.readable().await.map_err(SandboxError::new)?;
        Ok(())
    }
}
