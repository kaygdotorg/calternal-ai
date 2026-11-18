use std::path::PathBuf;
use std::sync::Arc;

mod admin_cli;
mod cli;
mod people_maintenance;
mod plugin_cli;
mod startup_io;
mod transport;
use calternal_cli::output::{CliError, Output, Reply};
use calternal_server::api::{self, AppState};
use calternal_server::auth::StoredDirectory;
use calternal_server::machines::MachineService;
use calternal_server::plugins;
use cli::{Cli, Cmd};
use startup_io::{DATA_HINT, write_private_atomic};

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
        Cmd::Threads { args } => calternal_cli::threads::main(args, &out),
        Cmd::Search { connection, cmd } => {
            let (op, input) = cmd.operation();
            let connection = calternal_cli::agent::Connection {
                base_url: connection.base_url,
                token_file: connection.token_file,
                request_id: connection.request_id,
            };
            calternal_cli::agent::invoke(connection, Some(op.id().into()), input.to_string(), &out)
        }
        Cmd::Notes { connection, cmd } => {
            let (op, input) = cmd.operation();
            calternal_cli::notes::run(connection, op, input, &out)
        }
        Cmd::Tags { connection, cmd } => {
            let (op, input) = cmd.operation();
            calternal_cli::notes::run(connection, op, input, &out)
        }
        Cmd::Trash { connection, cmd } => {
            let (op, input) = cmd.operation();
            calternal_cli::notes::run(connection, op, input, &out)
        }
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
        Cmd::People { connection, cmd } => {
            let (op, input) = cmd.operation();
            out.finish(calternal_cli::account_input::run(
                connection, op, input, &out,
            ))
        }
        Cmd::Groups { connection, cmd } => {
            let (op, input) = cmd.operation();
            out.finish(calternal_cli::account_input::run(
                connection, op, input, &out,
            ))
        }
        Cmd::Invites { connection, cmd } => out.finish(cmd.operation().and_then(|(op, input)| {
            calternal_cli::account_input::run(connection, op, input, &out)
        })),
        Cmd::Requests { connection, cmd } => out.finish(cmd.operation().and_then(|(op, input)| {
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
        Cmd::Plugin { command } => plugin_cli::main(command, &out),
        Cmd::PluginDigest { plugin_root } => out.success(&admin_cli::plugin_digest(&plugin_root)),
        Cmd::Machine { cmd } => calternal_machine::cli::main(cmd, &out),
        Cmd::Machines { cmd } => out.finish(admin_cli::machines(cmd, &out)),
        Cmd::SetupCode {
            appdata,
            installation_id,
        } => out.finish(admin_cli::setup_code(&appdata, &installation_id)),
        Cmd::Serve {
            appdata,
            userdata_ssd,
            userdata_hdd,
            create,
            installation_id,
            receipt_retention_days,
            invite_retention_days,
            bind_ip,
            port,
            web_dir,
            plugin_root,
            node,
            development,
            origin,
            trusted_proxies,
            machine_lease_ttl_ms,
            machine_poll_hold_ms,
            machine_sandbox_cpu_millis,
            test_plugin_root,
        } => {
            let mut machines = calternal_server::machines::Settings {
                lease_ttl: std::time::Duration::from_millis(machine_lease_ttl_ms),
                poll_hold: std::time::Duration::from_millis(machine_poll_hold_ms),
                ..Default::default()
            };
            machines.limits.cpu_millis = machine_sandbox_cpu_millis;
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(2)
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
                    std::time::Duration::from_secs(u64::from(invite_retention_days) * 86_400),
                    (bind_ip, port).into(),
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
    invite_retention: std::time::Duration,
    bind: std::net::SocketAddr,
    web_dir: Option<PathBuf>,
    plugin_root: Option<PathBuf>,
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
    let has_test_root = test_plugin_root.is_some();
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
    let local_test_workers = cfg!(feature = "test-plugins")
        && has_test_root
        && std::env::var_os("CALTERNAL_WORKER_TEST_BACKEND").is_some();
    if node.is_some() && !development && !local_test_workers {
        return Err(boot(
            1,
            "invalid_input",
            "--node requires --development or test-only plugins",
            "Use enrolled machines for plugin workers.",
        ));
    }
    let node = if local_test_workers || node.is_some() {
        let runtime = plugins::resolve_node(node.as_deref())
            .await
            .map_err(|why| {
                boot(
                    1,
                    "unavailable",
                    why,
                    "Install Node >= 22.13 for direct development workers.",
                )
            })?;
        tracing::info!(node = %runtime.path.display(), version = %runtime.version, "explicit development worker runtime");
        Some(runtime)
    } else {
        None
    };
    let web_dir = tokio::task::spawn_blocking(move || {
        web_dir.map(|d| std::fs::canonicalize(&d).unwrap_or(d))
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?;

    let listener = match tokio::net::TcpListener::bind(bind).await {
        Ok(l) => l,
        Err(e) => {
            return Err(boot(
                1,
                "unavailable",
                format!("cannot listen on {bind}: {e}"),
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
    config.invite_retention = invite_retention;
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
    let (store, pending_erasure) = {
        let spaces = directory.spaces.clone();
        let root = data_dir.clone();
        tokio::task::spawn_blocking(move || {
            let pending = calternal_server::erasure::restore_fence(&store, &spaces)?;
            calternal_server::secrets::configure_keys(&store.secrets(), spaces, &root)?;
            Ok::<_, calternal_types::Error>((store, pending))
        })
        .await
        .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?
    };
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
    machines.bind_lifecycle(store.content().lifecycle());
    store.bind_machines(&machines);
    let colocated = std::env::var_os("CALTERNAL_COLOCATED_HANDOFF")
        .map(|path| calternal_server::machines::colocated::start(machines.clone(), path.into()));
    let mut invite_tasks = tokio::task::JoinSet::new();
    if let Some(service) = calternal_server::auth::Directory::http_auth(&directory) {
        let auth = service.auth.clone();
        let backlog = auth
            .purge_invites()
            .await
            .map_err(|e| boot(1, "failed", format!("invite cleanup: {e}"), DATA_HINT))?;
        invite_tasks.spawn(people_maintenance::sweep_invites(auth, backlog));
    }
    let owner_setup = calternal_server::auth::Directory::http_auth(&directory).map(|service| {
        Arc::new(calternal_server::owner_setup::OwnerSetup::new(
            service.auth.clone(),
            &data_dir,
            &public_origin,
        ))
    });
    let setup_file = if let Some(setup) = &owner_setup {
        setup
            .issue(false)
            .await
            .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?
            .map(|_| setup.path().to_owned())
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
    let lifecycle = store.content().lifecycle();
    let jobs = tokio::task::spawn_blocking(move || {
        plugins::Jobs::open_with_lifecycle(&jobs_root, lifecycle)
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
    .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?
    .with_security(directory.spaces.agents())
    .with_receipt_retention(store.content().receipt_retention())
    .with_thread_events(store.shared_content());
    let token_files = directory.files.clone();
    let state = Arc::new(AppState {
        blocking: calternal_server::blocking::RequestWorkers::new()
            .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?,
        inputs: calternal_server::account_input::Inputs::new(
            &data_dir,
            calternal_server::account_input::Limits::from_env()
                .map_err(|e| CliError::new("invalid_input", e.message))?,
        )
        .with_content(store.shared_content()),
        store,
        directory,
        registry,
        workers: if let Some(node) = node {
            plugins::WorkerPool::new(Some(node))
        } else {
            plugins::WorkerPool::machines(machines.clone())
        },
        jobs,
        allowed_hosts: hosts,
    });
    drop(
        calternal_server::api::configure_search(&state)
            .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?,
    );
    let erasing = state.clone();
    tokio::task::spawn_blocking(move || {
        calternal_server::erasure::replay(
            &erasing.store,
            &erasing.jobs,
            &erasing.directory.spaces,
            pending_erasure,
        )
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
    .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    let _sweeper = machines.spawn_sweeper();
    // Operator commands reach this server here instead of writing behind it.
    let admin_root = data_dir.clone();
    let admin_machines = machines.clone();
    let _admin = tokio::task::spawn_blocking(move || {
        calternal_server::machines::admin::serve_socket(
            &admin_root,
            installation,
            admin_machines,
            owner_setup,
        )
    })
    .await
    .map_err(|e| boot(1, "failed", e.to_string(), DATA_HINT))?
    .map_err(|e| boot(1, "io", format!("admin socket: {e}"), DATA_HINT))?;
    let _plugin_admin = plugins::live_catalog::serve(data_dir.clone(), state.clone(), development)
        .map_err(|e| boot(1, "io", format!("plugin admin socket: {e}"), DATA_HINT))?;
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
    let _notes_retention = state.store.start_notes_retention().map_err(|e| {
        boot(
            2,
            "invalid_input",
            e.message,
            "Check the Notes Trash retention setting.",
        )
    })?;
    let wallpapers = api::wallpapers::Wallpapers::from_env(&data_dir)
        .await
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    let app = api::router(state.clone(), web_dir.clone())
        .layer(axum::Extension(machines.clone()))
        .layer(axum::Extension(wallpapers));

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
        transport::listener(listener),
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_signal().await;
        if let Some(task) = colocated {
            task.abort();
        }
        let interrupt = tokio::task::spawn_blocking(move || machines.interrupt_on_shutdown());
        state.workers.shutdown().await;
        if !matches!(interrupt.await, Ok(Ok(()))) {
            tracing::warn!("machine leases could not be fenced before shutdown");
        }
    })
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
