use std::path::{Path, PathBuf};
use std::sync::Arc;

mod admin_cli;
use admin_cli::MachinesCmd;
use calternal_cli::output::{CliError, Output, Reply};
use calternal_server::api::{self, AppState};
use calternal_server::auth::StoredDirectory;
use calternal_server::machines::MachineService;
use calternal_server::plugins::{self, Registry};
use calternal_server::store::Store;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "calternal",
    about = "Calternal-AI localhost development preview host (disposable data only)"
)]
struct Cli {
    /// Print one JSON document instead of text (errors too, on stdout); see docs/cli.md.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve the development preview on 127.0.0.1.
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
    Machine {
        #[command(subcommand)]
        cmd: calternal_machine::cli::Command,
    },
    /// Operator administration of execution machines in a data directory.
    Machines {
        #[command(subcommand)]
        cmd: MachinesCmd,
    },
    /// Print SHA-256 of trusted worker bundles for pinning in trusted-plugins.json.
    PluginDigest {
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
    let out = Output::detect(cli.json);
    let code = match cli.cmd {
        Cmd::PluginDigest { plugin_root } => out.success(&admin_cli::plugin_digest(&plugin_root)),
        Cmd::Machine { cmd } => calternal_machine::cli::main(cmd, &out),
        Cmd::Machines { cmd } => out.finish(admin_cli::machines(cmd, &out)),
        Cmd::Serve {
            appdata,
            userdata_ssd,
            userdata_hdd,
            create,
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
                    create,
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
    create: bool,
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
    const DATA_HINT: &str =
        "Check that --appdata exists (or can be created) and that you can write to it.";
    // The roots guard runs first: a missing, swapped or foreign root is refused
    // before any directory, lock or database is created.
    let store = Store::open(&roots, create).map_err(|e| {
        boot(
            1,
            "failed",
            e.message,
            "Pass the three roots this installation was set up with (--create only for \
             first setup), and make sure no other server is using them.",
        )
    })?;
    let data_dir = std::fs::canonicalize(&roots.appdata)
        .map_err(|e| boot(1, "io", format!("appdata: {e}"), DATA_HINT))?;
    // Every security change is written to this log in userdata-ssd before it is
    // acknowledged (crates/recovery), so auth.db can be rebuilt if appdata is lost.
    let recovery_dir = roots.userdata_ssd.join(".calternal/recovery");
    calternal_fsio::private::directory(&recovery_dir)
        .map_err(|e| boot(1, "io", format!("recovery log directory: {e}"), DATA_HINT))?;
    let recovery = calternal_recovery::Coordinator::open(&recovery_dir, store.installation())
        .map_err(|e| {
            boot(
                1,
                "failed",
                format!("recovery log: {e}"),
                "The recovery log in userdata-ssd is damaged or belongs to another installation.",
            )
        })?;
    let machines = {
        let dir = data_dir.clone();
        match tokio::task::spawn_blocking(move || MachineService::open(&dir, machine_settings))
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
    let plugin_root = std::fs::canonicalize(&plugin_root).unwrap_or(plugin_root);
    let test_root = test_plugin_root.map(|r| std::fs::canonicalize(&r).unwrap_or(r));
    let registry = match &test_root {
        Some(root) => Registry::load_with_tests(&plugin_root, Some(root)),
        None => Registry::load(&plugin_root),
    };
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
    let web_dir = web_dir.map(|d| std::fs::canonicalize(&d).unwrap_or(d));

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
    if config.set_trusted_proxies(&trusted_proxies).is_err() {
        return Err(boot(
            1,
            "invalid_input",
            "invalid --trusted-proxies",
            "Use comma-separated CIDR ranges, for example 10.0.0.0/8,fd00::/8.",
        ));
    }
    let directory = match StoredDirectory::open(&data_dir, config, development, Some(Arc::new(recovery))) {
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
    let setup_file = if let Some(service) = calternal_server::auth::Directory::http_auth(&directory) {
        match service.auth.first_owner_token().await {
            Ok(Some(token)) => {
                let path = data_dir.join("setup.token");
                write_private_atomic(&path, token.as_bytes())
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
    let jobs =
        plugins::Jobs::open(&data_dir).map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    let token_files = directory.files.clone();
    let state = Arc::new(AppState {
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
    write_private_atomic(&startup_path, &serde_json::to_vec_pretty(&startup).unwrap()).map_err(
        |e| {
            boot(
                1,
                "io",
                format!("cannot write startup.json: {e}"),
                DATA_HINT,
            )
        },
    )?;
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
    let _ = std::fs::remove_file(&startup_path);
    served.map_err(|e| {
        boot(
            1,
            "internal",
            format!("server error: {e}"),
            "Look at the server log above for the cause.",
        )
    })
}

fn write_private_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    calternal_fsio::write_atomic_mode(path, bytes, 0o600)
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
