//! Loopback-only operator entry point for the development preview.
use super::{CliResult, attach_runs, open_host};
use calternal_auth::AuthConfig;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::net::TcpListener;
use tower_http::services::{ServeDir, ServeFile};

pub(super) async fn serve(
    root: &Path,
    web: &Path,
    bundles: &[String],
    port: Option<u16>,
) -> CliResult<()> {
    let root = tokio::fs::canonicalize(root).await?;
    let web = tokio::fs::canonicalize(web).await?;
    if root.starts_with(&web)
        || web.starts_with(&root)
        || (port.is_none()
            && tokio::fs::read_dir(&root)
                .await?
                .next_entry()
                .await?
                .is_some())
    {
        return Err(
            "Use a separate data directory. A disposable preview requires an empty directory."
                .into(),
        );
    }
    // Keep the kernel-selected listener open from allocation through serving.
    if matches!(port, Some(0 | 4313)) {
        return Err("Choose a fixed preview port other than 4313.".into());
    }
    let listener = loop {
        let listener = TcpListener::bind(("127.0.0.1", port.unwrap_or_default())).await?;
        if listener.local_addr()?.port() != 4313 {
            break listener;
        }
    };
    let origin = format!("http://localhost:{}", listener.local_addr()?.port());
    let config = AuthConfig::new(&origin)?;
    let (host, auth) = open_host(root.clone(), &config).await?;
    let seeds = bundles
        .iter()
        .map(|bundle| {
            let directory = PathBuf::from(bundle);
            let id = directory
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Name each preview bundle directory after its declared plugin ID.")?;
            Ok::<_, &str>((id.to_owned(), directory))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let host = host.with_development_plugins(seeds);
    let (host, engine) = attach_runs(host, &root).await?;
    let setup_path = root.join("first-owner-code");
    if let Some(code) = auth.first_owner_token().await? {
        let private = setup_path.clone();
        tokio::task::spawn_blocking(move || write_code(private, code)).await??;
    } else {
        match tokio::fs::remove_file(&setup_path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }

    let app = calternal_host::browser::router(host, config)
        .route_service("/host", ServeFile::new(web.join("index.html")))
        .fallback_service(ServeDir::new(web));
    println!("Preview host {origin}/host");
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
    calternal_fsio::write_atomic_mode(&path, code.as_bytes(), 0o600)
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
