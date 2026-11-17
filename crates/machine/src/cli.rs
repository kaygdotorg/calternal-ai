//! `calternal machine …`: the machine role's commands.
//!
//! Printing goes through [`calternal_cli::output`] (tables, `--json`, errors).
//! Exit codes: 0 success, 1 failure, 2 usage, 3 the machine was revoked (the
//! systemd unit written by `scripts/install-machine.sh` does not restart on 3).

use std::path::PathBuf;
use std::sync::Arc;

use calternal_cli::output::{CliError, Output, Reply};
use clap::{Subcommand, ValueEnum};
use serde_json::json;

use crate::connection::{self, Config};
use crate::identity::Credentials;
use crate::protocol::Owner;
use crate::sandbox::Backend;
use crate::sandbox::podman::PodmanBackend;
use crate::sandbox::process::ProcessBackend;
use crate::{Error, enroll};

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
    /// Enroll this computer with a server using a one-time code.
    Enroll {
        /// Server base URL, for example `https://cal.example.org`.
        #[arg(long)]
        server: String,
        /// One-time enrollment code from the server, or `-` to read it from stdin.
        /// Prefer the environment variable or `-`: an argument shows in the process list.
        #[arg(long, env = "CALTERNAL_ENROLL_CODE")]
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

/// Run a machine-role command, print its result through `out`, and return the
/// process exit code. `out` must be built before this starts any thread.
pub fn main(cmd: Command, out: &Output) -> i32 {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            return out.failure(
                &CliError::new("internal", format!("cannot start the async runtime: {e}"))
                    .hint("Check the process limits (ulimit -u) and try again."),
            );
        }
    };
    out.finish(rt.block_on(run(cmd)))
}

/// The error for a machine-role failure, with the next step chosen from its kind.
fn fail(command: &str, e: Error) -> CliError {
    let (code, hint) = match &e {
        Error::Http(_) => (
            "unavailable",
            "Check the --server URL and your network, then run the command again.",
        ),
        Error::Status {
            status: 401 | 403 | 404,
            ..
        } => (
            "unauthorized",
            "The code may be used, expired or mistyped: ask for a new one (`calternal machines code`) and enroll again.",
        ),
        Error::Status { .. } => (
            "unavailable",
            "The server refused the request; try again in a minute.",
        ),
        Error::Revoked => (
            "revoked",
            "Ask the owner for a new code, delete the credentials file, then run `calternal machine enroll` again.",
        ),
        Error::Io(_) => (
            "io",
            "Check that the machine directory exists and is writable (--dir).",
        ),
        Error::Protocol(_) => (
            "failed",
            "Run `calternal machine status` to see this machine's state.",
        ),
    };
    let exit = if matches!(e, Error::Revoked) {
        EXIT_REVOKED
    } else {
        1
    };
    CliError::new(code, format!("{command}: {e}"))
        .hint(hint)
        .exit(exit)
}

/// The identity block shared by `enroll` and `status`.
fn identity(c: &Credentials, dir: &std::path::Path) -> Reply {
    Reply::new(json!({
        "machine_id": c.machine_id,
        "owner": owner_label(c.owner),
        "server": c.server,
        "credentials_file": Credentials::path(dir),
    }))
    .fields([
        ("machine", c.machine_id.to_string()),
        ("owner", owner_label(c.owner)),
        ("server", c.server.clone()),
        ("credentials", Credentials::path(dir).display().to_string()),
    ])
}

async fn run(cmd: Command) -> Result<Reply, CliError> {
    match cmd {
        Command::Enroll {
            server,
            code,
            dir,
            name,
        } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let name = name.unwrap_or_else(enroll::host_name);
            let code = if code == "-" {
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).map_err(|e| {
                    CliError::new("io", format!("cannot read the code from stdin: {e}"))
                        .hint("Pipe the code in, or pass --code CODE.")
                })?;
                line.trim().to_owned()
            } else {
                code
            };
            let creds = enroll::enroll(&server, &code, &dir, &name)
                .await
                .map_err(|e| fail("machine enroll", e))?;
            Ok(identity(&creds, &dir))
        }
        Command::Status { dir } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let creds = Credentials::load(&dir).map_err(|e| match &e {
                Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => CliError::new(
                    "not_found",
                    format!("this computer is not enrolled (no credentials in {})", dir.display()),
                )
                .hint("Run `calternal machine enroll --server URL --code CODE`, or pass the right --dir."),
                _ => fail("machine status", e),
            })?;
            Ok(identity(&creds, &dir))
        }
        Command::Run {
            dir,
            backend,
            image,
            podman,
            max_sandboxes,
        } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let root = dir.join("sandboxes");
            let backend: Arc<dyn Backend> = match backend {
                BackendKind::Process => Arc::new(ProcessBackend::new(root)),
                BackendKind::Podman => {
                    if !PodmanBackend::available(&podman).await {
                        return Err(CliError::new(
                            "unavailable",
                            format!("podman is not available ({})", podman.display()),
                        )
                        .hint("Install rootless Podman, or pass --podman PATH; --backend process is for development only."));
                    }
                    Arc::new(PodmanBackend::new(podman, root, image))
                }
            };
            let config = Config {
                dir,
                backend,
                max_sandboxes,
            };
            connection::run(config, shutdown_signal())
                .await
                .map_err(|e| fail("machine run", e))?;
            Ok(Reply::new(json!({"stopped": true})).line("Machine stopped."))
        }
    }
}

/// Stable text for a machine's owner, `pool` or `account:<uuid>`; used in tables and JSON.
pub fn owner_label(owner: Owner) -> String {
    match owner {
        Owner::Pool => "pool".into(),
        Owner::Account { id } => format!("account:{id}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::MachineKey;
    use calternal_cli::testing::{assert_golden, assert_shape};
    use std::path::{Path, PathBuf};
    use time::UtcOffset;

    fn golden(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/golden")
            .join(name)
    }
    fn creds() -> Credentials {
        Credentials {
            server: "https://cal.example.org".into(),
            machine_id: uuid::Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").unwrap(),
            owner: Owner::Pool,
            key: MachineKey::generate().unwrap(),
        }
    }
    fn both(name: &str, shape: &str, result: Result<Reply, CliError>) {
        let human = Output::fixed(false, None, UtcOffset::UTC);
        let json = Output::fixed(true, None, UtcOffset::UTC);
        let (h, j) = match &result {
            Ok(r) => (human.render(r), json.render(r)),
            Err(e) => (human.render_error(e).1, json.render_error(e).0),
        };
        assert_golden(&golden(&format!("{name}.txt")), &h);
        assert_golden(&golden(&format!("{name}.json")), &j);
        assert_shape(shape, &serde_json::from_str(&j).unwrap());
    }
    #[test]
    fn enroll_and_status_show_identity_without_the_key() {
        let r = identity(
            &creds(),
            Path::new("/home/u/.local/share/calternal/machine"),
        );
        let text = Output::fixed(false, None, UtcOffset::UTC).render(&r);
        assert!(
            !text.to_lowercase().contains("key"),
            "no key material: {text}"
        );
        both("machine-enroll", "calternal machine enroll", Ok(r.clone()));
        both("machine-status", "calternal machine status", Ok(r));
    }
    #[test]
    fn run_and_revoked_outcomes() {
        both(
            "machine-run-stopped",
            "calternal machine run",
            Ok(Reply::new(json!({"stopped": true})).line("Machine stopped.")),
        );
        let e = fail("machine run", Error::Revoked);
        assert_eq!(e.exit, EXIT_REVOKED);
        both("machine-run-revoked", "error", Err(e));
    }
    #[test]
    fn refused_code_says_how_to_get_a_new_one() {
        let refused = Error::Status {
            status: 404,
            message: "unknown, used or expired enrollment code".into(),
        };
        both(
            "machine-enroll-refused",
            "error",
            Err(fail("machine enroll", refused)),
        );
    }
}
