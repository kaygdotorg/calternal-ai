//! Local operator commands and the session-authenticated HTTP transport.
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Host, WorkerRuntime};
use calternal_types::Error;
use std::{path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [command, root, origin, rest @ ..] = args.as_slice() else {
        return Err("usage: calternal-host serve ROOT ORIGIN ADDRESS | install ROOT ORIGIN OWNER SPACE DIRECTORY [--official]".into());
    };
    let root = PathBuf::from(root);
    let auth_dir = root.join("auth");
    let config = AuthConfig::new(origin)?;
    let auth_config = config.clone();
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_dir, auth_config)).await??;
    let retention =
        std::env::var("CALTERNAL_RECEIPT_RETENTION_DAYS").map_or(Ok(7), |raw| raw.parse())?;
    let host = Host::new(root, auth, retention)?;
    let configured = [
        "CALTERNAL_WORKER_BWRAP",
        "CALTERNAL_WORKER_NODE",
        "CALTERNAL_WORKER_SDK",
    ]
    .map(std::env::var_os);
    let host = Arc::new(
        tokio::task::spawn_blocking(move || match configured {
            [None, None, None] => Ok(host),
            [Some(sandbox), Some(node), Some(sdk)] => {
                WorkerRuntime::new(sandbox.into(), node.into(), sdk.into())
                    .map(|runtime| host.with_worker_runtime(runtime))
            }
            _ => Err(Error::invalid_input(
                "Configure all three worker runtime paths.",
            )),
        })
        .await??,
    );
    match (command.as_str(), rest) {
        ("serve", [address]) => {
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
