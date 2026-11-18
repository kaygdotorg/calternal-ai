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
use crate::sandbox::direct::DirectBackend;
use crate::sandbox::podman::PodmanBackend;
use crate::{Error, enroll};

/// Exit code when the server revoked this machine.
pub const EXIT_REVOKED: i32 = 3;

/// Which sandbox backend to run threads in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BackendKind {
    /// Follow the machine owner's server-stored execution setting (default).
    Auto,
    /// One rootless Podman container per thread.
    Podman,
    /// Direct processes as the machine user; owner consent required, no isolation.
    Direct,
}

/// Machine-role subcommands.
#[derive(Debug, Subcommand)]
#[command(
    after_help = "Examples:\n  calternal machine enroll --server https://cal.example.org --code CODE\n  calternal machine status\n\nSee also: calternal machines code, calternal machine --help."
)]
pub enum Command {
    /// Discard local thread files as this machine's owner; stop its role first.
    Discard {
        /// Account UUID owning the persistent home, which is kept.
        #[arg(long)]
        account_id: uuid::Uuid,
        /// Thread workspace UUID to discard.
        #[arg(long)]
        thread_id: uuid::Uuid,
        /// Machine directory containing credentials and sandboxes.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Backend whose artifacts to remove; auto removes Podman artifacts when available.
        #[arg(long,value_enum,default_value_t=BackendKind::Auto)]
        backend: BackendKind,
        /// Podman executable for container cleanup.
        #[arg(long, default_value = "podman")]
        podman: PathBuf,
    },
    /// Choose direct or sandboxed execution on a machine you own.
    Set {
        /// Machine UUID to configure.
        machine_id: String,
        /// Explicit direct execution, or false for sandboxes; never accepted for pool machines.
        #[arg(long, action=clap::ArgAction::Set)]
        direct: bool,
        /// Person session used to verify machine ownership.
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
    },
    /// Verify sandbox and provider readiness for an enrolled machine.
    Verify {
        /// Enrolled machine UUID.
        machine_id: String,
        /// Space UUID where the diagnostic can run.
        #[arg(long)]
        space_id: String,
        /// Provider plugin to check. Defaults to Claude Code.
        #[arg(long, default_value = "")]
        provider: String,
        /// Person credential used to check machine ownership.
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
    },
    /// Enroll this computer with a server using a one-time code.
    #[command(
        about = "Enroll this computer with a server",
        after_help = "Examples:\n  calternal machine enroll --server https://cal.example.org --code CODE\n  calternal machine enroll --server https://cal.example.org --code - < code.txt\n\nSee also: calternal machines code, calternal machine status."
    )]
    Enroll {
        /// Server base URL, for example `https://cal.example.org`.
        #[arg(long)]
        server: String,
        /// One-time enrollment code from the server, or `-` to read it from stdin.
        /// Prefer the environment variable or `-`: an argument shows in the process list.
        #[arg(long, env = "CALTERNAL_ENROLL_CODE")]
        code: String,
        /// Directory for credentials and thread sandboxes.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Label shown to the owner; defaults to the host name.
        #[arg(long)]
        name: Option<String>,
        /// Explicitly choose direct execution; accepted only with your own account code.
        #[arg(long)]
        direct: bool,
    },
    /// Connect to the server and run assigned threads until stopped.
    #[command(
        about = "Run threads assigned to this machine until stopped",
        after_help = "Examples:\n  calternal machine run\n  calternal machine run --backend podman --max-sandboxes 4\n\nSee also: calternal machine status, calternal machine enroll."
    )]
    Run {
        /// Directory containing credentials and thread sandboxes.
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Sandbox backend.
        #[arg(long, value_enum, default_value_t = BackendKind::Auto)]
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
    #[command(
        about = "Show this machine's enrollment details",
        after_help = "Examples:\n  calternal machine status\n  calternal machine status --dir /var/lib/calternal-machine\n\nSee also: calternal machine enroll, calternal machine run."
    )]
    Status {
        /// Directory containing this machine's credentials.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// Run a machine-role command, print its result through `out`, and return the
/// process exit code. `out` must be built before this starts any thread.
pub fn main(cmd: Command, out: &Output) -> i32 {
    if let Command::Set {
        machine_id,
        direct,
        connection,
    } = cmd
    {
        return calternal_cli::agent::invoke(
            connection,
            Some("machines.set".into()),
            json!({"machine_id":machine_id,"direct":direct}).to_string(),
            out,
        );
    }

    if let Command::Verify {
        machine_id,
        space_id,
        provider,
        connection,
    } = cmd
    {
        return calternal_cli::agent::invoke(
            connection,
            Some(calternal_cli::agent::MACHINES_VERIFY.into()),
            json!({"machine_id":machine_id,"space_id":space_id,"provider_id":provider}).to_string(),
            out,
        );
    }
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
    let human = match &e {
        Error::Http(why) => format!("Couldn't reach the server while trying to {command}: {why}"),
        Error::Status { status, message } => {
            format!("The server returned HTTP {status} while trying to {command}: {message}")
        }
        Error::Revoked => "The server revoked this machine.".to_owned(),
        Error::Io(why) => format!("Couldn't access the machine's files: {why}"),
        Error::Protocol(why) => format!("The server response could not be used: {why}"),
    };
    CliError::new(code, format!("{command}: {e}"))
        .hint(hint)
        .human_message(human)
        .exit(exit)
}

/// The identity block shared by `enroll` and `status`.
fn identity(
    c: &Credentials,
    dir: &std::path::Path,
    status: Option<(&str, Option<u64>, &str)>,
) -> Reply {
    let mut data = json!({
        "machine_id": c.machine_id,
        "owner": owner_label(c.owner),
        "server": c.server,
        "credentials_file": Credentials::path(dir),
    });
    if let Some((state, count, _)) = status {
        data["erasure_status"] = json!(state);
        if let Some(count) = count {
            data["pending_erasures"] = json!(count);
        }
    }
    let reply = Reply::new(data).fields([
        ("machine", c.machine_id.to_string()),
        ("owner", owner_label(c.owner)),
        ("server", c.server.clone()),
        ("credentials", Credentials::path(dir).display().to_string()),
    ]);
    if let Some((state, count, hint)) = status {
        reply
            .fields([(
                "pending erasures",
                count.map_or_else(|| state.replace('_', " "), |n| n.to_string()),
            )])
            .hint(hint)
    } else {
        reply.hint("Next: run `calternal machine run` to accept assigned threads.")
    }
}

async fn run(cmd: Command) -> Result<Reply, CliError> {
    match cmd {
        Command::Set { .. } | Command::Verify { .. } => unreachable!("handled before runtime"),
        Command::Enroll {
            server,
            code,
            dir,
            name,
            direct,
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
            let creds = enroll::enroll(&server, &code, &dir, &name, direct)
                .await
                .map_err(|e| fail("machine enroll", e))?;
            Ok(identity(&creds, &dir, None))
        }
        Command::Discard {
            account_id,
            thread_id,
            dir,
            backend,
            podman,
        } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let _guard = crate::ownership::lock(&dir)
                .await
                .map_err(|e| fail("machine discard", e))?;
            let root = dir.join("sandboxes");
            let backend: Arc<dyn Backend> = match backend {
                BackendKind::Direct => Arc::new(DirectBackend::new(root)),
                BackendKind::Podman => Arc::new(PodmanBackend::new(podman, root, "unused")),
                BackendKind::Auto => {
                    let configured = crate::sandbox::configured::ConfiguredBackend::new(
                        root,
                        podman,
                        "unused".into(),
                    );
                    configured
                        .configure("direct")
                        .map_err(|e| CliError::new("cleanup", e.to_string()))?;
                    Arc::new(configured)
                }
            };
            backend.discard(account_id,thread_id).await.map_err(|_|CliError::new("cleanup","Workspace deletion failed.").hint("Stop active work and check the selected backend, directory and engine; then retry."))?;
            Ok(Reply::new(json!({"discarded":true,"account_id":account_id,"thread_id":thread_id,"account_home":"retained"})).fields([("workspace","discarded".to_owned()),("account home","retained".to_owned()),("account",account_id.to_string()),("thread",thread_id.to_string())]).hint("Run `calternal machine run` to accept new work."))
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
            let status = match connection::erasure_status(&creds).await {
                Ok(status) if status.pending_erasures == 0 => (
                    "clear",
                    Some(0),
                    "Run calternal machine run to accept work.",
                ),
                Ok(status) => (
                    "pending",
                    Some(status.pending_erasures),
                    "Run calternal machine run to apply and acknowledge erasures.",
                ),
                Err(error)
                    if matches!(error, Error::Revoked)
                        || matches!(error.status(), Some(403 | 404)) =>
                {
                    (
                        "operator_wipe",
                        None,
                        "Ask the operator to check calternal machines list and wipe the machine-role data.",
                    )
                }
                Err(_) => (
                    "unavailable",
                    None,
                    "Reconnect to the server or ask the operator to check calternal machines list.",
                ),
            };
            Ok(identity(&creds, &dir, Some(status)))
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
                BackendKind::Auto => Arc::new(crate::sandbox::configured::ConfiguredBackend::new(
                    root, podman, image,
                )),
                BackendKind::Direct => Arc::new(DirectBackend::new(root)),
                BackendKind::Podman => {
                    if !PodmanBackend::available(&podman).await {
                        return Err(CliError::new(
                            "unavailable",
                            format!("podman is not available ({})", podman.display()),
                        )
                        .hint("Install rootless Podman, or pass --podman PATH; --backend direct requires the owner to enable direct execution."));
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
            Ok(Reply::new(json!({"stopped": true})).line("The machine stopped cleanly."))
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
            None,
        );
        let text = Output::fixed(false, None, UtcOffset::UTC).render(&r);
        assert!(
            !text.to_lowercase().contains("key"),
            "no key material: {text}"
        );
        both("machine-enroll", "calternal machine enroll", Ok(r));
        let status = identity(
            &creds(),
            Path::new("/home/u/.local/share/calternal/machine"),
            Some((
                "pending",
                Some(2),
                "Run calternal machine run to apply and acknowledge erasures.",
            )),
        );
        both("machine-status", "calternal machine status", Ok(status));
    }
    #[test]
    fn run_and_revoked_outcomes() {
        both(
            "machine-run-stopped",
            "calternal machine run",
            Ok(Reply::new(json!({"stopped": true})).line("The machine stopped cleanly.")),
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

    #[test]
    fn machine_command_help_has_examples_and_see_also() {
        use clap::Parser;

        #[derive(clap::Parser)]
        struct MachineCli {
            #[command(subcommand)]
            _command: Command,
        }

        let commands: &[&[&str]] = &[
            &["machine", "--help"],
            &["machine", "enroll", "--help"],
            &["machine", "run", "--help"],
            &["machine", "status", "--help"],
        ];
        for args in commands {
            let help = MachineCli::try_parse_from(args.iter().copied())
                .err()
                .expect("--help exits before running the command")
                .to_string();
            assert!(help.contains("Examples:"), "{args:?}: {help}");
            assert!(help.contains("See also:"), "{args:?}: {help}");
        }
    }

    #[test]
    fn identity_hint_has_terminal_and_no_hint_goldens() {
        let reply = identity(
            &creds(),
            Path::new("/home/u/.local/share/calternal/machine"),
            None,
        );
        let terminal = Output::fixed_terminal(false, 120, UtcOffset::UTC, true);
        let no_hints = Output::fixed_terminal(false, 120, UtcOffset::UTC, false);
        assert_golden(&golden("machine-status-hint.txt"), &terminal.render(&reply));
        assert_golden(
            &golden("machine-status-no-hint.txt"),
            &no_hints.render(&reply),
        );
    }
}
