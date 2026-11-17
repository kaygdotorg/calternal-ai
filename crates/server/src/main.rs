use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use calternal_server::api::{self, AppState};
use calternal_server::auth::TokenTable;
use calternal_server::plugins::{self, Registry};
use calternal_server::store::Store;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "calternal",
    about = "Calternal-AI localhost development preview host (disposable data only)"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve the development preview on 127.0.0.1.
    Serve {
        /// Disposable data directory (created if missing, mode 0700).
        #[arg(long)]
        data_dir: PathBuf,
        /// Loopback port; 0 picks a free port (see startup.json).
        #[arg(long, default_value_t = calternal_core::DEFAULT_PORT)]
        port: u16,
        /// Built web assets served same-origin (e.g. apps/web/dist).
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
    let cli = Cli::parse();
    let code = match cli.cmd {
        Cmd::PluginDigest { plugin_root } => digest(&plugin_root),
        Cmd::Serve {
            data_dir,
            port,
            web_dir,
            plugin_root,
            node,
            development,
            origin,
            trusted_proxies,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(8)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(serve(
                data_dir,
                port,
                web_dir,
                plugin_root,
                node,
                development,
                origin,
                trusted_proxies,
            ))
        }
    };
    std::process::exit(code);
}

fn digest(root: &Path) -> i32 {
    for e in plugins::registry_entries() {
        if let Some(w) = &e.worker {
            match plugins::file_sha256(&root.join(w)) {
                Ok(d) => {
                    let pinned = e.worker_sha256.as_deref() == Some(d.as_str());
                    println!(
                        "{}\t{}\t{}\t{}",
                        e.id,
                        w,
                        d,
                        if pinned { "pinned" } else { "NOT PINNED" }
                    );
                }
                Err(why) => println!("{}\t{}\t-\t{}", e.id, w, why),
            }
        }
    }
    0
}

async fn serve(
    data_dir: PathBuf,
    port: u16,
    web_dir: Option<PathBuf>,
    plugin_root: PathBuf,
    node: Option<PathBuf>,
    development: bool,
    origin: Option<String>,
    trusted_proxies: String,
) -> i32 {
    if let Err(e) = calternal_auth::private_fs::directory(&data_dir) {
        eprintln!("calternal: cannot prepare data dir: {e}");
        return 1;
    }
    let data_dir = match std::fs::canonicalize(&data_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("calternal: data dir: {e}");
            return 1;
        }
    };
    let store = match Store::open(&data_dir) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("calternal: {}", e.message);
            return 2;
        }
    };
    let plugin_root = std::fs::canonicalize(&plugin_root).unwrap_or(plugin_root);
    let registry = Registry::load(&plugin_root);
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
            eprintln!("calternal: bind 127.0.0.1:{port}: {e}");
            return 1;
        }
    };
    let port = listener.local_addr().expect("bound").port();
    let public_origin = origin.unwrap_or_else(|| format!("http://localhost:{port}"));
    let mut config = match calternal_auth::AuthConfig::new(&public_origin) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("calternal: invalid origin");
            return 1;
        }
    };
    if config.set_trusted_proxies(&trusted_proxies).is_err() {
        eprintln!("calternal: invalid trusted proxies");
        return 1;
    }
    let tokens = match TokenTable::open(&data_dir, config, development) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("calternal: account directory: {e}");
            return 1;
        }
    };
    let setup_file = if let Some(service) = calternal_server::auth::Directory::http_auth(&tokens) {
        match service.auth.first_owner_token().await {
            Ok(Some(token)) => {
                let path = data_dir.join("setup.token");
                if let Err(e) = write_private_atomic(&path, token.as_bytes()) {
                    eprintln!("calternal: setup token file: {e}");
                    return 1;
                }
                Some(path)
            }
            Ok(None) => None,
            Err(e) => {
                eprintln!("calternal: setup: {e}");
                return 1;
            }
        }
    } else {
        None
    };
    let mut hosts = api::allowed_hosts(port);
    if let Some(service) = calternal_server::auth::Directory::http_auth(&tokens) {
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
    let token_files = tokens.files.clone();
    let state = Arc::new(AppState {
        store: Mutex::new(store),
        tokens,
        registry,
        workers: plugins::WorkerPool::new(node),
        allowed_hosts: hosts,
    });
    let app = api::router(state, web_dir.clone());

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
        "data_dir": data_dir,
        "token_files": token_files.iter().map(|(p, f)| (p.to_string(), serde_json::json!(f))).collect::<serde_json::Map<_, _>>(),
        "web_dir": web_dir,
    });
    let startup_path = data_dir.join("startup.json");
    if let Err(e) =
        write_private_atomic(&startup_path, &serde_json::to_vec_pretty(&startup).unwrap())
    {
        eprintln!("calternal: write startup.json: {e}");
        return 1;
    }
    println!("Calternal-AI development preview (disposable data; not production)");
    println!("  url:          {base_url}");
    for (p, f) in &token_files {
        println!("  {p} token file: {}", f.display());
    }
    println!("  startup info: {}", startup_path.display());

    let served = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await;
    let _ = std::fs::remove_file(&startup_path);
    match served {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("calternal: server error: {e}");
            1
        }
    }
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
