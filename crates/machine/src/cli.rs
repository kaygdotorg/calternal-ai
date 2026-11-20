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
use crate::link::{ConnectAddress, Link};
use crate::protocol::Owner;
use crate::ready::Ready;
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
    /// One isolated Firecracker guest per account on a Linux pool machine.
    Microvm,
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
        /// Operator configuration file for the experimental microVM backend.
        #[arg(long)]
        microvm_config: Option<PathBuf>,
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
    /// Print a private, single-use installer for a machine you own.
    Install {
        /// Server and signed-in person credential.
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
    },
    /// Check whether the first owner's installation has an available machine.
    Onboarding {
        /// Server and person credential with server settings authority.
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
        /// Provider plugin to check. Uses the server default when omitted.
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
        /// Dial this private IP for the server's HOST:PORT while keeping the URL and TLS hostname.
        #[arg(long, value_name = "HOST:PORT=IP")]
        connect_address: Option<ConnectAddress>,
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
        /// Dial this private IP for the server's HOST:PORT while keeping the URL and TLS hostname.
        #[arg(long, value_name = "HOST:PORT=IP")]
        connect_address: Option<ConnectAddress>,
        /// Share this private directory with the co-located server for first-owner enrollment.
        #[arg(long, requires = "bootstrap_server")]
        bootstrap_handoff: Option<PathBuf>,
        /// Public server URL to record in the co-located machine's credentials.
        #[arg(long, requires = "bootstrap_handoff")]
        bootstrap_server: Option<String>,
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
        /// Operator configuration file for the experimental microVM backend.
        #[arg(long)]
        microvm_config: Option<PathBuf>,
        /// Operator configuration for the offline small plugin VM. Requires Auto.
        #[arg(long)]
        space_vm_config: Option<PathBuf>,
        /// Optional operator ceiling on concurrent sandboxes.
        #[arg(long, default_value_t = u32::MAX)]
        max_sandboxes: u32,
    },
    /// Show this machine's identity and server.
    #[command(
        about = "Show this machine's enrollment details",
        after_help = "Examples:\n  calternal machine status\n  calternal machine status --dir /var/lib/calternal-machine\n\nSee also: calternal machine enroll, calternal machine run."
    )]
    Status {
        /// Dial this private IP for the server's HOST:PORT while keeping the URL and TLS hostname.
        #[arg(long, value_name = "HOST:PORT=IP")]
        connect_address: Option<ConnectAddress>,
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

    if let Command::Install { connection } = cmd {
        return calternal_cli::agent::invoke(
            connection,
            Some(calternal_cli::agent::MACHINES_INSTALL.into()),
            "{}".into(),
            out,
        );
    }
    if let Command::Onboarding { connection } = cmd {
        return calternal_cli::agent::invoke(
            connection,
            Some(calternal_cli::agent::MACHINES_ONBOARDING.into()),
            "{}".into(),
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
    let machine_role = matches!(&cmd, Command::Run { .. });
    let result = rt.block_on(run(cmd));
    if machine_role {
        rt.shutdown_background();
    }
    out.finish(result)
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
    small_vm: Option<crate::protocol::SmallVmCapacity>,
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
    if let Some(capacity) = small_vm {
        data["small_vm"] = json!(capacity);
    }
    let mut reply = Reply::new(data).fields([
        ("machine", c.machine_id.to_string()),
        ("owner", owner_label(c.owner)),
        ("server", c.server.clone()),
        ("credentials", Credentials::path(dir).display().to_string()),
    ]);
    if let Some(capacity) = small_vm {
        reply = reply.fields([
            (
                "resident plugin VMs",
                format!("{} of {}", capacity.resident_count, capacity.max_residents),
            ),
            (
                "reserved plugin VM memory",
                format!(
                    "{} of {} MiB",
                    capacity.resident_memory_bytes >> 20,
                    capacity.memory_limit_bytes >> 20
                ),
            ),
            (
                "plugin VM idle window",
                format!("{} s", capacity.idle_seconds),
            ),
        ]);
    }
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
        Command::Set { .. }
        | Command::Verify { .. }
        | Command::Onboarding { .. }
        | Command::Install { .. } => unreachable!("handled before runtime"),
        Command::Enroll {
            connect_address,
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
            let creds = enroll::enroll_at(
                &server,
                &code,
                &dir,
                &name,
                direct,
                connect_address.as_ref(),
            )
            .await
            .map_err(|e| fail("machine enroll", e))?;
            Ok(identity(&creds, &dir, None, None))
        }
        Command::Discard {
            account_id,
            thread_id,
            dir,
            backend,
            microvm_config,
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
                BackendKind::Microvm => {
                    let backend = microvm(root, microvm_config).await?;
                    backend
                        .initialize()
                        .await
                        .map_err(|e| CliError::new("cleanup", e.to_string()))?;
                    backend
                }
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
            let discarded = backend.discard(account_id, thread_id).await;
            let stopped = backend.shutdown().await;
            discarded.and(stopped).map_err(|_|CliError::new("cleanup","Workspace deletion failed.").hint("Stop active work and check the selected backend, directory and engine; then retry."))?;
            Ok(Reply::new(json!({"discarded":true,"account_id":account_id,"thread_id":thread_id,"account_home":"retained"})).fields([("workspace","discarded".to_owned()),("account home","retained".to_owned()),("account",account_id.to_string()),("thread",thread_id.to_string())]).hint("Run `calternal machine run` to accept new work."))
        }
        Command::Status {
            dir,
            connect_address,
        } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let creds = Credentials::load(&dir).map_err(|e| match &e {
                Error::Io(io) if io.kind() == std::io::ErrorKind::NotFound => CliError::new(
                    "not_found",
                    format!("this computer is not enrolled (no credentials in {})", dir.display()),
                )
                .hint("Run `calternal machine enroll --server URL --code CODE`, or pass the right --dir."),
                _ => fail("machine status", e),
            })?;
            let report = connection::erasure_status_at(&creds, connect_address.as_ref()).await;
            let small_vm = report.as_ref().ok().and_then(|status| status.small_vm);
            let status = match report {
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
            Ok(identity(&creds, &dir, Some(status), small_vm))
        }
        Command::Run {
            connect_address,
            bootstrap_handoff,
            bootstrap_server,
            dir,
            backend,
            image,
            podman,
            microvm_config,
            space_vm_config,
            max_sandboxes,
        } => {
            let dir = dir.unwrap_or_else(connection::default_dir);
            let root = dir.join("sandboxes");
            let (cli_dir, _cli_alias) =
                if matches!(backend, BackendKind::Auto | BackendKind::Direct) {
                    let (path, alias) = tokio::task::spawn_blocking(direct_cli_dir).await.map_err(
                    |_| {
                        CliError::new(
                            "unavailable",
                            "Could not look up the running machine executable for Direct turns.",
                        )
                    },
                )??;
                    (Some(path), alias)
                } else {
                    (None, None)
                };
            if space_vm_config.is_some() && !matches!(backend, BackendKind::Auto) {
                return Err(CliError::new(
                    "usage",
                    "--space-vm-config requires --backend auto.",
                ));
            }
            let backend: Arc<dyn Backend> = match backend {
                BackendKind::Auto => {
                    let mut configured = crate::sandbox::configured::ConfiguredBackend::new(
                        root.clone(),
                        podman,
                        image,
                    )
                    .with_cli_dir(cli_dir);
                    if let Some(path) = space_vm_config {
                        configured = configured
                            .with_small_vm(root, &path)
                            .await
                            .map_err(|error| CliError::new("configuration", error.to_string()))?;
                    }
                    Arc::new(configured)
                }
                BackendKind::Direct => Arc::new(DirectBackend::new(root).with_cli_dir(cli_dir)),
                BackendKind::Microvm => {
                    #[cfg(target_os = "linux")]
                    {
                        match crate::sandbox::microvm::probe_kvm().await {
                            Ok(()) => microvm(root, microvm_config).await?,
                            Err(error) => {
                                tracing::warn!(%error, "using containers because KVM is unavailable");
                                if !PodmanBackend::available(&podman).await {
                                    return Err(CliError::new(
                                        "unavailable",
                                        "KVM and rootless Podman are unavailable.",
                                    )
                                    .hint("Enable nested KVM or install rootless Podman."));
                                }
                                Arc::new(PodmanBackend::new(podman, root, image).kvm_fallback())
                            }
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    microvm(root, microvm_config).await?
                }
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
            let ready = Ready::from_env();
            if let (Some(handoff), Some(server)) = (bootstrap_handoff, bootstrap_server) {
                let link = Link::with_connect_address(&server, connect_address.as_ref())
                    .map_err(|error| fail("machine run", error))?;
                tokio::select! {
                    result = crate::bootstrap::wait(&dir, &handoff, &link, &ready) => {
                        result.map_err(|e| fail("machine run", e))?;
                    }
                    _ = shutdown_signal() => {
                        return Ok(Reply::new(json!({"stopped": true})).line("The machine stopped cleanly."));
                    }
                }
            }
            let config = Config {
                dir,
                backend,
                max_sandboxes,
            };
            connection::run_at(config, connect_address.as_ref(), &ready, shutdown_signal())
                .await
                .map_err(|e| fail("machine run", e))?;
            Ok(Reply::new(json!({"stopped": true})).line("The machine stopped cleanly."))
        }
    }
}

/// Make the running executable available as `calternal` in Direct turns.
/// Cost: one executable lookup and stat, plus a private temporary directory
/// and symlink if renamed. Call from a blocking task.
fn direct_cli_dir() -> Result<(PathBuf, Option<tempfile::TempDir>), CliError> {
    let executable = std::env::current_exe().map_err(|error| {
        CliError::new(
            "unavailable",
            format!("Could not find the running machine executable for Direct turns: {error}."),
        )
    })?;
    let invalid = |detail| {
        CliError::new(
            "unavailable",
            format!(
                "Could not provide 'calternal' for Direct turns using {}: {detail}.",
                executable.display()
            ),
        )
    };
    if !std::fs::metadata(&executable)
        .map_err(|error| invalid(error.to_string()))?
        .is_file()
    {
        return Err(invalid("the executable path is not a file".into()));
    }
    if executable
        .file_name()
        .is_some_and(|name| name == "calternal")
    {
        let parent = executable
            .parent()
            .ok_or_else(|| invalid("the executable has no parent directory".into()))?;
        return Ok((parent.to_owned(), None));
    }
    let alias = tempfile::Builder::new()
        .prefix("calternal-cli-")
        .tempdir()
        .map_err(|error| invalid(error.to_string()))?;
    std::os::unix::fs::symlink(&executable, alias.path().join("calternal"))
        .map_err(|error| invalid(error.to_string()))?;
    Ok((alias.path().to_owned(), Some(alias)))
}

async fn microvm(root: PathBuf, config: Option<PathBuf>) -> Result<Arc<dyn Backend>, CliError> {
    #[cfg(target_os = "linux")]
    {
        use crate::sandbox::microvm::{Config, MicrovmBackend};
        let path = config.ok_or_else(|| {
            CliError::new(
                "usage",
                "The microVM backend requires an operator configuration.",
            )
            .hint("Pass --microvm-config PATH.")
        })?;
        let config = Config::load(&path).await.map_err(|e| {
            CliError::new(
                "configuration",
                format!("Cannot load microVM configuration: {e}"),
            )
            .hint("Check the configuration in the deployment guide.")
        })?;
        let root = tokio::fs::canonicalize(&root.parent().expect("sandbox root"))
            .await
            .map_err(|e| CliError::new("unavailable", e.to_string()))?
            .join("sandboxes");
        Ok(Arc::new(MicrovmBackend::new(root, config)))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (root, config);
        Err(
            CliError::new("unavailable", "The microVM backend requires Linux and KVM.")
                .hint("Select a backend supported by this machine."),
        )
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
            None,
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
