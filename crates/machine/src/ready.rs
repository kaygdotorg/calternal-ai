//! Notify the co-located systemd unit once the machine has reached the server.

#[derive(Default)]
pub(crate) struct Ready {
    #[cfg(target_os = "linux")]
    socket: Option<std::ffi::OsString>,
    #[cfg(target_os = "linux")]
    sent: std::sync::atomic::AtomicBool,
}

impl Ready {
    pub(crate) fn from_env() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            socket: std::env::var_os("NOTIFY_SOCKET"),
            ..Self::default()
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn pending(&self) -> bool {
        self.socket.is_some() && !self.sent.load(std::sync::atomic::Ordering::Relaxed)
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn pending(&self) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn report(&self, status: &str) -> std::io::Result<()> {
        use std::os::linux::net::SocketAddrExt;
        use std::os::unix::{
            ffi::OsStrExt,
            net::{SocketAddr, UnixDatagram},
        };

        let Some(path) = self.socket.as_ref() else {
            return Ok(());
        };
        let socket = match path.as_bytes().strip_prefix(b"@") {
            Some(abstract_name) => SocketAddr::from_abstract_name(abstract_name)?,
            None => SocketAddr::from_pathname(path)?,
        };
        let notifier = UnixDatagram::unbound()?;
        notifier.set_nonblocking(true)?;
        notifier.send_to_addr(format!("READY=1\nSTATUS={status}").as_bytes(), &socket)?;
        self.sent.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn report(&self, _: &str) -> std::io::Result<()> {
        Ok(())
    }
}
