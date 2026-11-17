//! `calternal machine …`: the machine role's commands.
//!
//! Exit codes: 0 success, 1 failure, 2 usage, 3 the machine was revoked (the
//! systemd unit written by `scripts/install-machine.sh` does not restart on 3).

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Subcommand, ValueEnum};

use crate::connection::{self, Config};
use crate::identity::Credentials;
use crate::protocol::Owner;
use crate::sandbox::Backend;
use crate::sandbox::podman::PodmanBackend;
use crate::sandbox::process::ProcessBackend;
use crate::{Error, enrol};

/// Exit code when the server revoked this machine.
pub const EXIT_REVOKED: i32 = 3;

/// Which sandbox backend to run threads in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackendKind {
    /// One rootless Podman container per thread.
    Podman,
    /// Plain processes; tests and development only, not isolation.
    Process,
}

/// Machine-role subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Enrol this computer with a server using a one-time code.
    Enrol {
        /// Server base URL, for example https://cal.example.org.
        #[arg(long)]
        server: String,
        /// One-time enrolment code from the server.
        #[arg(long, env = "CALTERNAL_ENROL_CODE")]
        code: String,
        /// Machine directory (credentials and sandboxes).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Label shown to the owner; defaults to the host name.
        #[arg(long)]
        name: Option<String>,
    },
    /// Connect to the server and run assigned threads until stopped.
    Run {
        /// Machine directory (credentials and sandboxes).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Sandbox backend.
        #[arg(long, value_enum, default_value_t = BackendKind::Podman)]
        backend: BackendKind,
        /// Default base image for Podman sandboxes.
        #[arg(long, default_value = "docker.io/library/alpine:3")]
        image: String,
        /// Podman executable.
        #[arg(long, default_value = "podman")]
        podman: PathBuf,
        /// Most threads run at once.
        #[arg(long, default_value_t = 10)]
        max_sandboxes: u32,
    },
    /// Show this machine's identity and server.
    Status {
        /// Machine directory.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// Run a machine-role command and return the process exit code.
pub fn main(cmd: Command) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("calternal machine: cannot start runtime: {e}");
            return 1;
        }
    };
    rt.block_on(run(cmd))
}

async fn run(cmd: Command) -> i32 {
    match cmd {
        Command::Enrol { server, code, dir, name } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let name = name.unwrap_or_else(enrol::host_name);
            match enrol::enrol(&server, &code, &dir, &name).await {
                Ok(c) => {
                    println!("enrolled machine {} ({})", c.machine_id, owner_text(c.owner));
                    println!("credentials: {}", Credentials::path(&dir).display());
                    0
                }
                Err(e) => {
                    eprintln!("calternal machine enrol: {e}");
                    1
                }
            }
        }
        Command::Status { dir } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            match Credentials::load(&dir) {
                Ok(c) => {
                    println!("machine_id: {}\nowner: {}\nserver: {}", c.machine_id, owner_text(c.owner), c.server);
                    0
                }
                Err(e) => {
                    eprintln!("calternal machine status: {e}");
                    1
                }
            }
        }
        Command::Run { dir, backend, image, podman, max_sandboxes } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let root = dir.join("sandboxes");
            let backend: Arc<dyn Backend> = match backend {
                BackendKind::Process => Arc::new(ProcessBackend::new(root)),
                BackendKind::Podman => {
                    if !PodmanBackend::available(&podman).await {
                        eprintln!("calternal machine run: podman is not available ({})", podman.display());
                        return 1;
                    }
                    Arc::new(PodmanBackend::new(podman, root, image))
                }
            };
            let config = Config { dir, backend, max_sandboxes };
            match connection::run(config, shutdown_signal()).await {
                Ok(()) => 0,
                Err(Error::Revoked) => {
                    eprintln!("calternal machine: {}", Error::Revoked);
                    EXIT_REVOKED
                }
                Err(e) => {
                    eprintln!("calternal machine run: {e}");
                    1
                }
            }
        }
    }
}

fn owner_text(owner: Owner) -> String {
    match owner {
        Owner::Account { id } => format!("owned by account {id}"),
        Owner::Pool => "pool machine".into(),
    }
}

async fn shutdown_signal() {
    let mut term = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
        Ok(t) => t,
        Err(_) => return std::future::pending().await,
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = term.recv() => {},
    }
}
