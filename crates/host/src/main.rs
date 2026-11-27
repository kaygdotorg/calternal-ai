//! Local operator commands and the session-authenticated HTTP transport.
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Host, WorkerRuntime};
use calternal_machine::sandbox::{Backend, podman::PodmanBackend};
use calternal_types::Error;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[path = "main_dev.rs"]
mod dev;

type CliResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> CliResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let [command, root, web, chat, harness, port] = args.as_slice()
        && command == "preview-serve"
    {
        return dev::serve(
            Path::new(root),
            Path::new(web),
            Path::new(chat),
            Path::new(harness),
            Some(port.parse()?),
        )
        .await;
    }
    if let [command, root, web, chat, harness] = args.as_slice()
        && command == "dev-serve"
    {
        return dev::serve(
            Path::new(root),
            Path::new(web),
            Path::new(chat),
            Path::new(harness),
            None,
        )
        .await;
    }
    let [command, root, origin, rest @ ..] = args.as_slice() else {
        return Err("usage: calternal-host serve ROOT ORIGIN ADDRESS | install ROOT ORIGIN OWNER SPACE DIRECTORY [--official]".into());
    };
    let root = PathBuf::from(root);
    let mut config = AuthConfig::new(origin)?;
    if let Some(proxies) = std::env::var_os("CALTERNAL_TRUSTED_PROXIES") {
        config.set_trusted_proxies(proxies.to_str().ok_or("Invalid trusted proxy setting.")?)?;
    }
    let (host, _) = open_host(root.clone(), &config).await?;
    match (command.as_str(), rest) {
        ("serve", [address]) => {
            let (host, _engine) = attach_runs(host, &root).await?;
            let app = calternal_host::browser::router(host, config);
            axum::serve(
                tokio::net::TcpListener::bind(address).await?,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await?;
        }
        ("install", [owner, space, directory, flags @ ..])
            if flags.is_empty() || flags == ["--official"] =>
        {
            let (owner, space, directory, official) = (
                owner.parse()?,
                space.parse()?,
                PathBuf::from(directory),
                !flags.is_empty(),
            );
            tokio::task::spawn_blocking(move || {
                host.install_local(owner, space, &directory, official)
            })
            .await??;
            println!("Installed plugin bundle.");
        }
        _ => return Err("invalid command or arguments".into()),
    }
    Ok(())
}

async fn open_host(root: PathBuf, config: &AuthConfig) -> CliResult<(Host, Auth)> {
    let auth_dir = root.join("auth");
    let config = config.clone();
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_dir, config)).await??;
    let retention =
        std::env::var("CALTERNAL_RECEIPT_RETENTION_DAYS").map_or(Ok(7), |raw| raw.parse())?;
    let host = Host::new(root, auth.clone(), retention)?;
    let configured = [
        "CALTERNAL_WORKER_BWRAP",
        "CALTERNAL_WORKER_NODE",
        "CALTERNAL_WORKER_SDK",
    ]
    .map(std::env::var_os);
    let host = tokio::task::spawn_blocking(move || match configured {
        [None, None, None] => Ok(host),
        [Some(sandbox), Some(node), Some(sdk)] => {
            WorkerRuntime::new(sandbox.into(), node.into(), sdk.into())
                .map(|runtime| host.with_worker_runtime(runtime))
        }
        _ => Err(Error::invalid_input(
            "Configure all three worker runtime paths.",
        )),
    })
    .await??;
    Ok((host, auth))
}

struct RunningEngine {
    backend: Arc<PodmanBackend>,
    _lock: calternal_fsio::DataLock,
}

impl RunningEngine {
    async fn cleanup(self) -> CliResult<()> {
        self.backend.initialize().await?;
        Ok(())
    }
}

async fn attach_runs(host: Host, root: &Path) -> CliResult<(Arc<Host>, Option<RunningEngine>)> {
    let host = Arc::new(host);
    let sign_in = std::env::var_os("CALTERNAL_RUN_SIGN_IN_DIR").map(PathBuf::from);
    let real_image = std::env::var_os("CALTERNAL_REAL_RUN_IMAGE");
    let image = match (real_image, &sign_in) {
        (Some(image), Some(_)) => image
            .into_string()
            .map_err(|_| "Invalid real run image name.")?,
        (None, None) => match std::env::var("CALTERNAL_RUN_IMAGE") {
            Ok(image) => image,
            Err(std::env::VarError::NotPresent) => return Ok((host, None)),
            Err(error) => return Err(error.into()),
        },
        _ => return Err("Set both CALTERNAL_REAL_RUN_IMAGE and CALTERNAL_RUN_SIGN_IN_DIR.".into()),
    };
    let root = root.join("runs");
    let lock_root = root.clone();
    let lock = tokio::task::spawn_blocking(move || calternal_fsio::DataLock::acquire(&lock_root))
        .await??;
    let mut backend = PodmanBackend::new("podman", root, &image);
    if let Some(sign_in) = sign_in {
        backend = backend.with_run_sign_in(sign_in)?;
    }
    let backend = Arc::new(backend);
    backend.initialize().await?;
    let host = host.with_podman_runs(backend.clone(), image);
    Ok((
        host,
        Some(RunningEngine {
            backend,
            _lock: lock,
        }),
    ))
}
