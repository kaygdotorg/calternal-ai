//! Loopback-only operator entry point for the disposable Chat preview.
use super::{CliResult, attach_runs, open_host};
use calternal_auth::AuthConfig;
use std::{
    io::Write,
    net::SocketAddr,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

pub(super) async fn serve(root: &Path, web: &Path, chat: &Path, harness: &Path) -> CliResult<()> {
    let root = tokio::fs::canonicalize(root).await?;
    let web = tokio::fs::canonicalize(web).await?;
    if root.starts_with(&web)
        || web.starts_with(&root)
        || tokio::fs::read_dir(&root)
            .await?
            .next_entry()
            .await?
            .is_some()
    {
        return Err("Use an empty data directory separate from the built web client.".into());
    }
    // Keep the kernel-selected listener open from allocation through serving.
    let listener = loop {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        if listener.local_addr()?.port() != 4313 {
            break listener;
        }
    };
    let origin = format!("http://localhost:{}", listener.local_addr()?.port());
    let config = AuthConfig::new(&origin)?;
    let (host, auth) = open_host(root.clone(), &config).await?;
    let host = host.with_development_plugins([
        ("calternal-chat".to_owned(), chat.to_owned()),
        ("calternal-chat-claude-code".to_owned(), harness.to_owned()),
    ]);
    let (host, engine) = attach_runs(host, &root).await?;
    let setup_path = root.join("first-owner-code");
    let code = auth
        .first_owner_token()
        .await?
        .ok_or("The development root must be new.")?;
    let private = setup_path.clone();
    tokio::task::spawn_blocking(move || write_code(private, code)).await??;

    let app = calternal_host::browser::router(host, config)
        .route_service("/host", ServeFile::new(web.join("index.html")))
        .fallback_service(ServeDir::new(web));
    println!("Chat host {origin}/host");
    println!("First-owner code file {}", setup_path.display());
    let served = async {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    };
    tokio::select! {
        result = served => result?,
        () = shutdown() => {},
    }
    if let Some(engine) = engine {
        engine.cleanup().await?;
    }
    Ok(())
}

fn write_code(path: PathBuf, code: String) -> std::io::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(code.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()
}

async fn shutdown() {
    #[cfg(unix)]
    if let Ok(mut term) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = term.recv() => {},
        }
        return;
    }
    let _ = tokio::signal::ctrl_c().await;
}
