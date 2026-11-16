use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
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
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .max_blocking_threads(8)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(serve(data_dir, port, web_dir, plugin_root, node))
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
) -> i32 {
    if let Err(e) = std::fs::create_dir_all(&data_dir)
        .and_then(|_| std::fs::set_permissions(&data_dir, std::fs::Permissions::from_mode(0o700)))
    {
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
    let tokens = match TokenTable::load_or_create(&data_dir) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("calternal: dev tokens: {e}");
            return 1;
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
    let token_files = tokens.files.clone();
    let state = Arc::new(AppState {
        store: Mutex::new(store),
        tokens,
        registry,
        node,
        allowed_hosts: api::allowed_hosts(port),
    });
    let app = api::router(state, web_dir.clone());

    let base_url = format!("http://127.0.0.1:{port}");
    let startup = serde_json::json!({
        "profile": "development-preview",
        "base_url": base_url,
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

    let served = axum::serve(listener, app)
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
    use std::io::Write;
    let tmp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)
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
