//! Public site association for passkeys in an iOS web view.

use axum::Extension;
use axum::body::Bytes;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use calternal_types::Error;

const SETTING: &str = "CALTERNAL_APPLE_APP_IDS";
/// The exact path Apple requests for an associated domain.
pub(super) const PATH: &str = "/.well-known/apple-app-site-association";

/// The optional Apple site association document serialized at startup.
#[derive(Clone)]
pub struct AppleAppSite(Option<Bytes>);

impl AppleAppSite {
    /// Read the operator setting once. An unset variable disables the document.
    /// Invalid identifiers or more than eight entries prevent startup.
    pub fn from_env() -> Result<Self, Error> {
        let invalid = || {
            Error::invalid_input(format!(
                "{SETTING} must contain one to eight comma-separated TEAMID.bundle.id identifiers."
            ))
        };
        let value = match std::env::var(SETTING) {
            Ok(value) => value,
            Err(std::env::VarError::NotPresent) => return Ok(Self(None)),
            Err(std::env::VarError::NotUnicode(_)) => return Err(invalid()),
        };
        let mut apps = Vec::new();
        for app in value.split(',') {
            if apps.len() == 8 || !valid_id(app) {
                return Err(invalid());
            }
            apps.push(app);
        }
        let document = serde_json::json!({ "webcredentials": { "apps": apps } });
        Ok(Self(Some(Bytes::from(document.to_string()))))
    }
}

fn valid_id(app: &str) -> bool {
    let Some((team, bundle)) = app.split_once('.') else {
        return false;
    };
    team.len() == 10
        && team
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        && bundle.contains('.')
        && bundle.split('.').all(|label| {
            let label = label.as_bytes();
            label.first().is_some_and(u8::is_ascii_alphanumeric)
                && label.last().is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .iter()
                    .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
        })
}

/// Serve the startup-built document, cloning only its shared byte handle.
pub(super) async fn get(site: Option<Extension<AppleAppSite>>) -> Response {
    let Some(Extension(AppleAppSite(Some(document)))) = site else {
        return StatusCode::NOT_FOUND.into_response();
    };
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        document,
    )
        .into_response()
}
