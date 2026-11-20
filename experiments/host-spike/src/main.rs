//! `host-spike`: step 1 of the primitives-only host, as a throwaway spike.
//!
//! ```text
//! host-spike init  --root DIR --space UUID --member UUID:NAME:ROLE ...
//! host-spike serve --root DIR [--addr 127.0.0.1:0]
//! ```
//!
//! One route: `POST /call` with `{name, input}`. The acting person, space,
//! plugin and (optionally) the client's attempt key come from headers
//! `x-person`, `x-space`, `x-plugin`, `x-attempt`. There is no sign-in: the
//! person header is trusted (stub; see the report).
mod call;
mod decl;
mod files;
mod fm;
mod host;
mod query;
mod records;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = arg(&args, "--root").map(PathBuf::from);
    let result = match (args.get(1).map(String::as_str), root) {
        (Some("init"), Some(root)) => init(&args, &root),
        (Some("serve"), Some(root)) => serve(root, arg(&args, "--addr").unwrap_or_else(|| "127.0.0.1:0".into())),
        _ => Err("usage: host-spike init --root DIR --space UUID --member UUID:NAME:ROLE ...\n       host-spike serve --root DIR [--addr HOST:PORT]".into()),
    };
    if let Err(e) = result {
        eprintln!("host-spike: {e}");
        std::process::exit(2);
    }
}

fn init(args: &[String], root: &std::path::Path) -> Result<(), String> {
    let space = SpaceId::parse(&arg(args, "--space").ok_or("--space is required")?)
        .map_err(|e| e.message)?;
    let mut members = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--member" {
            let spec = args.get(i + 1).ok_or("--member needs UUID:NAME:ROLE")?;
            let mut parts = spec.splitn(3, ':');
            let (Some(p), Some(n), Some(r)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(format!("--member {spec}: expected UUID:NAME:ROLE"));
            };
            if !matches!(r, "reader" | "editor" | "manager" | "owner") {
                return Err(format!(
                    "--member {spec}: role is reader, editor, manager or owner"
                ));
            }
            members.push((
                AccountId::parse(p).map_err(|e| e.message)?,
                n.to_owned(),
                r.to_owned(),
            ));
        }
    }
    host::Space::create(root, space, &members).map_err(|e| e.message)?;
    println!(
        "space    {space}\nmembers  {}\nnext     host-spike serve --root {}",
        members.len(),
        root.display()
    );
    Ok(())
}

fn serve(root: PathBuf, addr: String) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let host = Arc::new(host::Host::new(&root));
        let app = axum::Router::new()
            .route("/call", axum::routing::post(handle))
            .with_state(host);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| e.to_string())?;
        println!(
            "listening http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        axum::serve(listener, app).await.map_err(|e| e.to_string())
    })
}

/// The one route. Builds the call context from headers, then runs the call on
/// a blocking thread holding the space's lock (SQLite and file I/O never run on
/// an async thread).
async fn handle(
    State(host): State<Arc<host::Host>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    let result = tokio::task::spawn_blocking(move || -> Result<Value, Error> {
        let h = |k: &str| {
            headers
                .get(k)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
        };
        let person = AccountId::parse(
            &h("x-person").ok_or_else(|| Error::unauthorized("x-person is required"))?,
        )?;
        let space = SpaceId::parse(
            &h("x-space").ok_or_else(|| Error::invalid_input("x-space is required"))?,
        )?;
        let plugin = h("x-plugin").unwrap_or_else(|| "host".into());
        let name = body
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::input_field("/name", "an operation name"))?;
        let input = body.get("input").cloned().unwrap_or(json!({}));
        let request = call::request_id(
            space,
            person,
            &plugin,
            name,
            &input,
            h("x-attempt").as_deref(),
        );
        let ctx = call::Ctx {
            space,
            person,
            plugin,
            request,
            role: String::new(),
        };
        let space = host.space(space)?;
        let mut space = space
            .lock()
            .map_err(|_| Error::internal("space lock poisoned"))?;
        call::dispatch(ctx, &mut space, name, &input)
    })
    .await
    .unwrap_or_else(|_| Err(Error::internal("the call panicked")));
    match result {
        Ok(v) => (StatusCode::OK, Json(json!({ "ok": true, "result": v }))),
        Err(e) => (
            StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            Json(json!({ "ok": false, "error": e })),
        ),
    }
}
