use std::path::PathBuf;
use std::sync::Arc;

mod admin_cli;
mod startup_io;
use admin_cli::MachinesCmd;
use calternal_cli::output::{CliError, Output, Reply};
use calternal_server::api::{self, AppState};
use calternal_server::auth::StoredDirectory;
use calternal_server::machines::MachineService;
use calternal_server::plugins;
use clap::{Parser, Subcommand};
use startup_io::{DATA_HINT, write_private_atomic};

#[derive(Parser)]
#[command(
    name = "calternal",
    about = "Run a Calternal server or manage its enrolled machines",
    after_help = "Examples:\n  calternal serve --appdata ./data/appdata --userdata-ssd ./data/ssd --userdata-hdd ./data/hdd --development\n  calternal machines list --appdata /srv/calternal/appdata\n\nSee also: calternal serve --help, calternal machine --help, calternal machines --help."
)]
struct Cli {
    /// Print one JSON document instead of text (errors too, on stdout); see docs/cli.md.
    #[arg(long, global = true)]
    json: bool,
    /// Hide the short next-step line shown after successful terminal output.
    #[arg(
        long,
        global = true,
        help = "Hide terminal hints (also CALTERNAL_NO_HINTS=1)"
    )]
    no_hints: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run, watch and control official provider sessions in machine sandboxes.
    Agent {
        #[command(flatten)]
        args: calternal_cli::agent::Args,
    },
    /// Invoke a discovered operation with structured JSON input.
    Invoke {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
        operation: String,
        #[arg(long, default_value = "{}")]
        input: String,
    },
    /// Discover available operations using the current person or thread credential.
    Describe {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
    },
    /// Save, read and erase private account compositions.
    Drafts {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::account_input::Drafts,
    },
    /// Queue, edit, confirm, submit or cancel private pending input.
    Pending {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::account_input::Pending,
    },
    /// Follow a space journal; --json prints one compact JSON object per event.
    Watch {
        space: String,
        #[arg(long, env = "CALTERNAL_URL", default_value = "http://127.0.0.1:3210")]
        base_url: String,
        #[arg(long, env = "CALTERNAL_TOKEN_FILE")]
        token_file: PathBuf,
        #[arg(
            long,
            help = "Bookmark cache directory exclusive to this server and account"
        )]
        bookmarks: Option<PathBuf>,
    },
    /// Serve the development preview on 127.0.0.1.
    #[command(
        about = "Start the development-preview server",
        after_help = "Examples:\n  calternal serve --appdata ./data/appdata --userdata-ssd ./data/ssd --userdata-hdd ./data/hdd --development\n  calternal serve --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal --port 3210\n\nSee also: calternal machines --help, calternal machine --help."
    )]
    Serve {
        /// Protected databases and state (normally on SSD).
        #[arg(long)]
        appdata: PathBuf,
        /// Space trees of claim-validated text files (normally on SSD).
        #[arg(long)]
        userdata_ssd: PathBuf,
        /// Space trees of other original files (normally on HDD).
        #[arg(long)]
        userdata_hdd: PathBuf,
        /// First setup: pair three empty or missing roots. Without it every root must
        /// already belong to this installation; harmless on a paired installation.
        #[arg(long)]
        create: bool,
        /// This installation's UUID, kept in the server's configuration outside every
        /// root (ADR 0062). Each root's marker must carry it; required.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: Option<String>,
        /// Days each space keeps request receipts; a retry after that runs as a new
        /// request.
        #[arg(long, env = "CALTERNAL_RECEIPT_RETENTION_DAYS", default_value_t = 7)]
        receipt_retention_days: u32,
        /// Loopback port; 0 picks a free port (see startup.json).
        #[arg(long, default_value_t = calternal_core::DEFAULT_PORT)]
        port: u16,
        /// Built web assets served same-origin (any static directory).
        #[arg(long)]
        web_dir: Option<PathBuf>,
        /// Enable disposable development accounts and bearer token files (off by default).
        #[arg(long)]
        development: bool,
        /// Exact public WebAuthn origin; defaults to `http://localhost:<bound-port>`.
        #[arg(long)]
        origin: Option<String>,
        /// Comma-separated trusted proxy CIDRs; forwarded headers are ignored by default.
        #[arg(long, default_value = "")]
        trusted_proxies: String,
        /// Root of repository-shipped trusted plugins.
        #[arg(long, default_value = "plugins")]
        plugin_root: PathBuf,
        /// Absolute Node executable for plugin workers (default: node on PATH).
        #[arg(long)]
        node: Option<PathBuf>,
        /// Root of the test-only plugins (`tests/plugins`); refused unless the server
        /// was built with the `test-plugins` feature.
        #[arg(long)]
        test_plugin_root: Option<PathBuf>,
        /// How long a machine's leases survive without a sync, in milliseconds.
        #[arg(long, default_value_t = 90_000)]
        machine_lease_ttl_ms: u64,
        /// Longest hold of a machine's long-poll, in milliseconds.
        #[arg(long, default_value_t = 20_000)]
        machine_poll_hold_ms: u64,
    },
    /// Machine role: enroll this computer and run threads for a server.
    #[command(
        about = "Enroll this computer and run assigned threads",
        after_help = "Examples:\n  calternal machine enroll --help\n  calternal machine status\n\nSee also: calternal machines code, calternal machine enroll."
    )]
    Machine {
        #[command(subcommand)]
        cmd: calternal_machine::cli::Command,
    },
    /// Operator administration of execution machines in a data directory.
    #[command(
        about = "Manage machines enrolled with this server",
        after_help = "Examples:\n  calternal machines list --appdata /srv/calternal/appdata\n\nSee also: calternal machines code, calternal machine --help."
    )]
    Machines {
        #[command(subcommand)]
        cmd: MachinesCmd,
    },
    /// Print SHA-256 of trusted worker bundles for pinning in trusted-plugins.json.
    #[command(
        about = "Show worker bundle hashes and their trust status",
        after_help = "Examples:\n  calternal plugin-digest\n  calternal plugin-digest --plugin-root ./plugins\n\nSee also: calternal serve --help."
    )]
    PluginDigest {
        /// Directory containing the trusted plugin bundles.
        #[arg(long, default_value = "plugins")]
        plugin_root: PathBuf,
    },
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = match calternal_cli::output::parse_args::<Cli>() {
        Ok(cli) => cli,
        Err(code) => std::process::exit(code),
    };
    // Reads the local time zone, which is only safe before any thread starts.
    let out = Output::detect_with_options(cli.json, cli.no_hints);
    let code = match cli.cmd {
        Cmd::Agent { args } => calternal_cli::agent::main(args, &out),
        Cmd::Invoke {
            connection,
            operation,
            input,
        } => calternal_cli::agent::invoke(connection, Some(operation), input, &out),
        Cmd::Describe { connection } => {
            calternal_cli::agent::invoke(connection, None, "{}".into(), &out)
        }
        Cmd::Drafts { connection, cmd } => {
            let (op, input) = cmd.operation();
            out.finish(calternal_cli::account_input::run(
                connection, op, input, &out,
            ))
        }
        Cmd::Pending { connection, cmd } => out.finish(cmd.operation().and_then(|(op, input)| {
            calternal_cli::account_input::run(connection, op, input, &out)
        })),
        Cmd::Watch {
            space,
            base_url,
            token_file,
            bookmarks,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("runtime");
            out.finish(rt.block_on(calternal_cli::watch::run(
                &base_url,
                token_file,
                space,
                bookmarks,
                &out,
                calternal_client::CancellationToken::new(),
            )))
        }
        Cmd::PluginDigest { plugin_root } => out.success(&admin_cli::plugin_digest(&plugin_root)),
        Cmd::Machine { cmd } => calternal_machine::cli::main(cmd, &out),
        Cmd::Machines { cmd } => out.finish(admin_cli::machines(cmd, &out)),
        Cmd::Serve {
            appdata,
            userdata_ssd,
            userdata_hdd,
            create,
            installation_id,
            receipt_retention_days,
            port,
            web_dir,
            plugin_root,
            node,
            development,
            origin,
            trusted_proxies,
            machine_lease_ttl_ms,
            machine_poll_hold_ms,
            test_plugin_root,
        } => {
            let machines = calternal_server::machines::Settings {
                lease_ttl: std::time::Duration::from_millis(machine_lease_ttl_ms),
                poll_hold: std::time::Duration::from_millis(machine_poll_hold_ms),
                ..Default::default()
            };
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(8)
                .enable_all()
                .build()
                .expect("tokio runtime");
            let result = if test_plugin_root.is_some() && plugins::test_entries().is_empty() {
                Err(CliError::new(
                    "invalid_input",
                    "--test-plugin-root needs a build with the test-plugins feature",
                )
                .hint("Rebuild with `--features test-plugins`, or drop --test-plugin-root."))
            } else {
                rt.block_on(serve(
                    calternal_roots::Roots {
                        appdata,
                        userdata_ssd,
                        userdata_hdd,
                    },
                    installation_id,
                    create,
                    std::time::Duration::from_secs(
                        u64::from(receipt_retention_days.max(1)) * 86_400,
                    ),
                    port,
                    web_dir,
                    plugin_root,
                    node,
                    development,
                    origin,
                    trusted_proxies,
                    test_plugin_root,
                    machines,
                    &out,
                ))
            };
            match result {
                Ok(()) => 0,
                Err(e) => out.failure(&e),
            }
        }
    };
    std::process::exit(code);
}

/// A startup failure: `message` says what failed, `hint` what to do, `exit` is the process status.
fn boot(exit: i32, code: &str, message: impl Into<String>, hint: &str) -> CliError {
    CliError::new(code, message).hint(hint).exit(exit)
}

async fn serve(
    roots: calternal_roots::Roots,
    installation_id: Option<String>,
    create: bool,
    receipt_retention: std::time::Duration,
    port: u16,
    web_dir: Option<PathBuf>,
    plugin_root: PathBuf,
    node: Option<PathBuf>,
    development: bool,
    origin: Option<String>,
    trusted_proxies: String,
    test_plugin_root: Option<PathBuf>,
    machine_settings: calternal_server::machines::Settings,
    out: &Output,
) -> Result<(), CliError> {
    // The roots guard runs first: a missing, swapped or foreign root is refused
    // before any directory, lock or database is created.
    const PIN_HINT: &str = "Put this installation's UUID in the server configuration as \
         CALTERNAL_INSTALLATION_ID (or pass --installation-id). For first setup, generate a \
         new UUID, keep it there, and start once with --create.";
    let installation = installation_id
        .ok_or_else(|| {
            boot(
                1,
                "invalid_input",
                "no installation ID is configured",
                PIN_HINT,
            )
        })
        .and_then(|id| {
            uuid::Uuid::parse_str(&id).map_err(|_| {
                boot(
                    1,
                    "invalid_input",
                    "the installation ID is not a UUID",
                    PIN_HINT,
                )
            })
        })?;
    let (store, data_dir, recovery, registry) = tokio::task::spawn_blocking(move || {
        startup_io::open_stores(
            roots,
            installation,
            create,
            receipt_retention,
            plugin_root,
            test_plugin_root,
        )
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))??;
    let node = match plugins::resolve_node(node.as_deref()).await {
        Ok(n) => {
            tracing::info!(node = %n.path.display(), version = %n.version, "plugin worker runtime");
            Some(n)
        }
        Err(why) => {
            tracing::warn!(reason = %why, "Node unavailable; plugin workers disabled");
            None
        }
    };
    let web_dir = tokio::task::spawn_blocking(move || {
        web_dir.map(|d| std::fs::canonicalize(&d).unwrap_or(d))
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?;

    let listener = match tokio::net::TcpListener::bind(("127.0.0.1", port)).await {
        Ok(l) => l,
        Err(e) => {
            return Err(boot(
                1,
                "unavailable",
                format!("cannot listen on 127.0.0.1:{port}: {e}"),
                "Pick another --port (0 picks a free one), or stop the program using it.",
            ));
        }
    };
    let port = listener.local_addr().expect("bound").port();
    let public_origin = origin.unwrap_or_else(|| format!("http://localhost:{port}"));
    let mut config = match calternal_auth::AuthConfig::new(&public_origin) {
        Ok(c) => c,
        Err(_) => {
            return Err(boot(
                1,
                "invalid_input",
                "invalid --origin",
                "Use the exact public origin, for example https://cal.example.org.",
            ));
        }
    };
    // Directory receipts follow the same retention as space receipts.
    config.receipt_retention = receipt_retention;
    if config.set_trusted_proxies(&trusted_proxies).is_err() {
        return Err(boot(
            1,
            "invalid_input",
            "invalid --trusted-proxies",
            "Use comma-separated CIDR ranges, for example 10.0.0.0/8,fd00::/8.",
        ));
    }
    let directory_root = data_dir.clone();
    let directory = tokio::task::spawn_blocking(move || {
        let data_dir = directory_root;
        let directory =
            match StoredDirectory::open(&data_dir, config, development, Some(Arc::new(recovery))) {
                Ok(t) => t,
                Err(e) => {
                    return Err(boot(
                        1,
                        "failed",
                        format!("account directory: {e}"),
                        DATA_HINT,
                    ));
                }
            };
        Ok::<_, CliError>(directory)
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))??;
    {
        let keys = store.secrets();
        let spaces = directory.spaces.clone();
        let root = data_dir.clone();
        tokio::task::spawn_blocking(move || {
            calternal_server::secrets::configure_keys(&keys, spaces, &root)
        })
        .await
        .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    }
    // Machine authority shares auth.db and its recovery barrier.
    let machines = {
        let dir = data_dir.clone();
        let authority = directory.spaces.machines();
        match tokio::task::spawn_blocking(move || {
            MachineService::open(&dir, authority, machine_settings)
        })
        .await
        {
            Ok(Ok(m)) => m,
            Ok(Err(e)) => {
                return Err(boot(
                    1,
                    "failed",
                    format!("machines: {}", e.message),
                    DATA_HINT,
                ));
            }
            Err(_) => {
                return Err(boot(
                    1,
                    "internal",
                    "the machines service did not start",
                    DATA_HINT,
                ));
            }
        }
    };
    let _sweeper = machines.spawn_sweeper();
    // Operator commands reach this server here instead of writing behind it.
    let admin_root = data_dir.clone();
    let admin_machines = machines.clone();
    let _admin = tokio::task::spawn_blocking(move || {
        calternal_server::machines::admin::serve_socket(&admin_root, installation, admin_machines)
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
    .map_err(|e| boot(1, "io", format!("admin socket: {e}"), DATA_HINT))?;
    let setup_file = if let Some(service) = calternal_server::auth::Directory::http_auth(&directory)
    {
        match service.auth.first_owner_token().await {
            Ok(Some(token)) => {
                let path = data_dir.join("setup.token");
                write_private_atomic(&path, token.as_bytes())
                    .await
                    .map_err(|e| boot(1, "io", format!("setup token file: {e}"), DATA_HINT))?;
                Some(path)
            }
            Ok(None) => None,
            Err(e) => return Err(boot(1, "failed", format!("setup: {e}"), DATA_HINT)),
        }
    } else {
        None
    };
    let mut hosts = api::allowed_hosts(port);
    if let Some(service) = calternal_server::auth::Directory::http_auth(&directory) {
        let authority = service
            .config
            .origin_header()
            .split("://")
            .nth(1)
            .unwrap_or("")
            .to_owned();
        if !hosts.contains(&authority) {
            hosts.push(authority);
        }
    }
    let jobs_root = data_dir.clone();
    let jobs = tokio::task::spawn_blocking(move || plugins::Jobs::open(&jobs_root))
        .await
        .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?
        .with_security(directory.spaces.agents());
    let token_files = directory.files.clone();
    let state = Arc::new(AppState {
        inputs: calternal_server::account_input::Inputs::new(
            &data_dir,
            calternal_server::account_input::Limits::from_env()
                .map_err(|e| CliError::new("invalid_input", e.message))?,
        )
        .with_content(store.shared_content()),
        store,
        directory,
        registry,
        workers: plugins::WorkerPool::new(node),
        jobs,
        allowed_hosts: hosts,
    });
    match plugins::runner::recover(&state).await {
        Ok(0) => {}
        Ok(n) => tracing::info!(jobs = n, "resuming plugin jobs"),
        Err(e) => {
            return Err(boot(
                1,
                "failed",
                format!("job recovery: {}", e.message),
                DATA_HINT,
            ));
        }
    }
    calternal_server::turns::retry::recover(&state, &machines)
        .await
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    let app = api::router(state, web_dir.clone()).layer(axum::Extension(machines));

    let base_url = if development {
        format!("http://127.0.0.1:{port}")
    } else {
        public_origin.clone()
    };
    let startup = serde_json::json!({
        "profile": "development-preview",
        "development_tokens": development,
        "setup_token_file": setup_file,
        "base_url": base_url,
        "passkey_origin": public_origin,
        "pid": std::process::id(),
        "appdata": data_dir,
        "token_files": token_files.iter().map(|(p, f)| (p.to_string(), serde_json::json!(f))).collect::<serde_json::Map<_, _>>(),
        "web_dir": web_dir,
    });
    let startup_path = data_dir.join("startup.json");
    write_private_atomic(&startup_path, &serde_json::to_vec_pretty(&startup).unwrap())
        .await
        .map_err(|e| {
            boot(
                1,
                "io",
                format!("cannot write startup.json: {e}"),
                DATA_HINT,
            )
        })?;
    let mut fields = vec![("url".to_owned(), base_url.clone())];
    fields.extend(
        token_files
            .iter()
            .map(|(p, f)| (format!("{p} token file"), f.display().to_string())),
    );
    fields.push(("startup info".into(), startup_path.display().to_string()));
    out.success(
        &Reply::new(serde_json::json!({
            "profile": "development-preview",
            "url": base_url,
            "token_files": token_files.iter().map(|(p, f)| (p.to_string(), serde_json::json!(f))).collect::<serde_json::Map<_, _>>(),
            "startup_file": startup_path,
        }))
        .line("Calternal-AI development preview (disposable data; not production)")
        .fields(fields),
    );

    let served = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    let _ = tokio::fs::remove_file(&startup_path).await;
    served.map_err(|e| {
        boot(
            1,
            "internal",
            format!("server error: {e}"),
            "Look at the server log above for the cause.",
        )
    })
}

async fn shutdown_signal() {
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("signal");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = term.recv() => {},
    }
    tracing::info!("shutting down");
}

#[cfg(test)]
mod cli_output_tests;
