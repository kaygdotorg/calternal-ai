//! Local operator commands and the session-authenticated HTTP transport.
use axum::extract::{DefaultBodyLimit, Path, State, rejection::JsonRejection};
use axum::http::{HeaderMap, StatusCode};
use axum::{Json, Router, routing::post};
use calternal_auth::{Auth, AuthConfig};
use calternal_host::{Call, Host, WRITE_BYTES, WorkerRuntime};
use calternal_types::{Error, ErrorCode};
use serde_json::{Value, json};
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
    let auth = tokio::task::spawn_blocking(move || Auth::open(&auth_dir, config)).await??;
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
            let app = Router::new()
                .route("/v1/spaces/{space}/call", post(route))
                .layer(DefaultBodyLimit::max(WRITE_BYTES))
                .with_state(host);
            axum::serve(tokio::net::TcpListener::bind(address).await?, app).await?;
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

async fn route(
    State(host): State<Arc<Host>>,
    Path(space): Path<String>,
    headers: HeaderMap,
    body: Result<Json<Call>, JsonRejection>,
) -> (StatusCode, Json<Value>) {
    let result = async {
        let bearer = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Error::unauthorized("Sign in to use this space."))?;
        let Json(call) = body.map_err(|e| {
            if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
                Error::new(ErrorCode::TooLarge, "The call exceeds 16 MiB.")
            } else {
                Error::invalid_input("Expected a JSON call envelope.")
            }
        })?;
        host.session_call(bearer, space.parse()?, call).await
    }
    .await;
    match result {
        Ok(value) => (StatusCode::OK, Json(json!({"result":value}))),
        Err(error) => {
            let (code, next) = match error.code {
                ErrorCode::Unauthorized => ("needs-sign-in", "Sign in and try again."),
                ErrorCode::Forbidden | ErrorCode::PasskeyReverificationRequired => {
                    ("not-allowed", "Check your access.")
                }
                ErrorCode::NotFound => ("not-found", "Check the requested ID."),
                ErrorCode::Conflict => ("conflict", "Refresh before retrying."),
                ErrorCode::InvalidInput => ("invalid", "Check the call arguments."),
                ErrorCode::TooLarge => ("too-large", "Reduce the request size."),
                ErrorCode::EffectiveScopeLimitExceeded | ErrorCode::RetryAfter => {
                    ("limit", "Reduce the work or retry later.")
                }
                ErrorCode::Internal | ErrorCode::Unavailable => ("unreachable", "Try again later."),
            };
            let status =
                StatusCode::from_u16(error.code.http_status()).expect("shared HTTP status");
            let code = error
                .details
                .as_ref()
                .and_then(|details| details.get("operation_code"))
                .and_then(Value::as_str)
                .unwrap_or(code);
            let next = error
                .details
                .as_ref()
                .and_then(|details| details.get("next"))
                .and_then(Value::as_str)
                .unwrap_or(next);
            (
                status,
                Json(json!({"error":{"code":code,"message":error.message,"next":next}})),
            )
        }
    }
}
