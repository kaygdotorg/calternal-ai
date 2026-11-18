//! Read-only bearer transport. Each request resolves the link independently.

use super::*;
use axum::Extension;
use axum::extract::Path;
use axum::response::Html;
use calternal_auth::limit::{Limiter, TargetLimiter};
use tokio::sync::Semaphore;

const SOURCE_PER_MINUTE: u32 = 60;
const LINK_PER_MINUTE: u32 = 120;
const IN_FLIGHT: usize = 16;

struct GuestLimits {
    source: Limiter,
    link: TargetLimiter,
    work: Semaphore,
}

pub(in crate::api) fn routes<D: Directory>() -> Router<Shared<D>> {
    Router::new()
        .route("/s/{secret}", get(open::<D>))
        .route("/s/{secret}/notes/{note}", get(open::<D>))
        .layer(Extension(Arc::new(GuestLimits {
            source: Limiter::new(SOURCE_PER_MINUTE),
            link: TargetLimiter::new(LINK_PER_MINUTE),
            work: Semaphore::new(IN_FLIGHT),
        })))
}

async fn open<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(limits): Extension<Arc<GuestLimits>>,
    Path(path): Path<std::collections::HashMap<String, String>>,
    request: Request,
) -> Response {
    let source = state
        .directory
        .http_auth()
        .map(|http| crate::account_http::source_key(http, &request));
    let json = request
        .headers()
        .get(header::ACCEPT)
        .is_some_and(|v| v == "application/json");
    let headers = request.headers().clone();
    let offset = request
        .uri()
        .query()
        .and_then(|query| query.strip_prefix("after="))
        .and_then(|offset| offset.parse::<usize>().ok())
        .filter(|offset| *offset <= 10_000)
        .unwrap_or(0);
    drop(request);
    let result = load(&state, &limits, &path, source, offset, &headers).await;
    let mut response = match result {
        Ok(value) if json => axum::Json(value).into_response(),
        Ok(value) => Html(document(&value)).into_response(),
        Err(GuestFailure::Other(error)) if error.code == ErrorCode::NotFound => {
            (StatusCode::NOT_FOUND, "Shared item not found").into_response()
        }
        Err(GuestFailure::Limited) => {
            let mut response =
                (StatusCode::TOO_MANY_REQUESTS, "Try again in a minute").into_response();
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("60"));
            response
        }
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Shared item temporarily unavailable",
        )
            .into_response(),
    };
    for (name, value) in [
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
        (
            header::HeaderName::from_static("x-robots-tag"),
            "noindex, nofollow",
        ),
        (
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'",
        ),
        (header::VARY, "Accept"),
    ] {
        response
            .headers_mut()
            .insert(name, HeaderValue::from_static(value));
    }
    response
}

enum GuestFailure {
    Limited,
    Other(Error),
}

impl From<Error> for GuestFailure {
    fn from(error: Error) -> Self {
        Self::Other(error)
    }
}

async fn load<D: Directory>(
    state: &Shared<D>,
    limits: &GuestLimits,
    path: &std::collections::HashMap<String, String>,
    source: Option<String>,
    offset: usize,
    headers: &HeaderMap,
) -> Result<Value, GuestFailure> {
    let http = state.directory.http_auth().ok_or_else(missing)?;
    if !limits.source.attempt(&source.ok_or_else(missing)?) {
        return Err(GuestFailure::Limited);
    }
    let _permit = limits
        .work
        .try_acquire()
        .map_err(|_| GuestFailure::Limited)?;
    let secret = path.get("secret").ok_or_else(missing)?;
    let note = path
        .get("note")
        .map(|s| s.parse::<usize>().map_err(|_| missing()))
        .transpose()?;
    let link = http
        .auth
        .resolve_item_link(secret)
        .await
        .map_err(auth_error)?
        .ok_or_else(missing)?;
    if !limits.link.attempt(&link.grant.to_string()) {
        return Err(GuestFailure::Limited);
    }
    let (kind, id, grant) = (link.kind, link.item, link.grant);
    let root = format!("/s/{secret}");
    let (mut value, space, item_kind, item_id) = blocking(state, move |s| {
        let content = s.content();
        let space = content.locate(&kind, id)?.ok_or_else(missing)?;
        match kind.as_str() {
            "notes" if note.is_none() => {
                let note = s.get(space, &id.to_string())?;
                Ok((json!({"title":note.title,"markdown":note.markdown}),space,kind,id))
            }
            "collections" => {
                let collection = content.live_metadata(space, "collections", id)?;
                if !eligible("collections", &collection) {
                    return Err(missing());
                }
                let members = collection["members"].as_array().ok_or_else(missing)?;
                if let Some(position) = note {
                    let id = members.get(position).and_then(Value::as_str)
                        .and_then(|member| Uuid::parse_str(member).ok()).ok_or_else(missing)?;
                    let note = s.get(space, &id.to_string())?;
                    return Ok((json!({"title":note.title,"markdown":note.markdown,"back":root}),space,"notes".into(),id));
                }
                let mut rows = Vec::new();
                for (position, member) in members.iter().enumerate().skip(offset).take(50) {
                    let Some(note) = member.as_str().and_then(|s| Uuid::parse_str(s).ok()) else { continue; };
                    match content.live_metadata(space, "notes", note) {
                        Ok(metadata) => rows.push(json!({"title":metadata["title"],"href":format!("{root}/notes/{position}")})),
                        Err(e) if e.code == ErrorCode::NotFound => (),
                        Err(e) => return Err(e),
                    }
                }
                let mut response = json!({"title":collection["title"],"members":rows});
                if offset + 50 < members.len() {
                    response["next"] = json!(format!("{root}?after={}",offset + 50));
                }
                Ok((response,space,kind,id))
            }
            _ => Err(missing()),
        }
    }).await?;
    if let Some((account, _)) = crate::account_http::identify(http, headers).await {
        let (caps, _) =
            crate::resolve::item_capabilities(state, account, space, &item_kind, item_id).await?;
        if caps.is_some_and(|caps| caps.contains(calternal_types::Capability::SpaceRead)) {
            value["item"] = json!({"kind":item_kind,"id":item_id});
        }
    }
    if !http
        .auth
        .item_link_is_live(grant)
        .await
        .map_err(auth_error)?
    {
        return Err(missing().into());
    }
    Ok(value)
}

// The host supplies a safe fallback for external guests. The web lane owns the
// styled guest presentation. Treat all note and collection strings as data.
fn document(value: &Value) -> String {
    let title = escaped(value["title"].as_str().unwrap_or(""));
    let mut html = format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>{title}</title><main><h1>{title}</h1>"
    );
    if let Some(kind) = value["item"]["kind"].as_str()
        && let Some(id) = value["item"]["id"].as_str()
    {
        html.push_str(&format!("<a href=\"/{kind}/{id}\">Open in Calternal</a>"));
    }
    if let Some(back) = value["back"].as_str() {
        html.push_str(&format!("<a href=\"{}\">Collection</a>", escaped(back)));
    }
    if let Some(markdown) = value["markdown"].as_str() {
        html.push_str(&format!("<pre>{}</pre>", escaped(markdown)));
    } else if let Some(members) = value["members"].as_array() {
        html.push_str("<ul>");
        for member in members {
            let title = escaped(member["title"].as_str().unwrap_or(""));
            let href = escaped(member["href"].as_str().unwrap_or(""));
            html.push_str(&format!("<li><a href=\"{href}\">{title}</a></li>"));
        }
        html.push_str("</ul>");
        if let Some(next) = value["next"].as_str() {
            html.push_str(&format!("<a href=\"{}\">More notes</a>", escaped(next)));
        }
    }
    html.push_str("</main></html>");
    html
}

fn escaped(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
