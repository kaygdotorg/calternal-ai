//! Multipart upload and authenticated same-origin face delivery.
use super::{
    Directory, Identity, Shared, assets, db,
    face::{Face, Format},
    run, validate_digest, validate_id,
};
use crate::plugins::Session;
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Multipart, Path, State, multipart::MultipartRejection},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use calternal_core::FontStoredFamily;
use calternal_types::Error;
use std::{sync::LazyLock, time::Duration};

pub(in crate::api) fn routes<D: Directory>() -> Router<Shared<D>> {
    Router::new()
        .route("/api/v1/fonts/face/{id}/{digest}", get(face::<D>))
        .route(
            "/api/v1/fonts/upload",
            post(upload::<D>).layer(DefaultBodyLimit::max(db::UPLOAD_BYTES + 64 * 1024)),
        )
}

async fn face<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Path((id, digest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let result = async {
        validate_id(&id)?;
        validate_digest(&digest)?;
        let etag = format!("\"{digest}\"");
        let (format, bytes, private) = run(move || {
            let _step = state.store.content().lifecycle().step(who.id.as_uuid())?;
            state
                .store
                .appearance
                .storage
                .deliver(&who.id.to_string(), &id, &digest)
        })
        .await?;
        let policy = if private {
            "no-store"
        } else {
            "private, max-age=31536000, immutable"
        };
        Ok::<_, Error>(assets::respond(
            bytes,
            format.mime(),
            &etag,
            policy,
            true,
            &headers,
        ))
    }
    .await;
    result.unwrap_or_else(crate::api::error_response)
}

async fn upload<D: Directory>(
    State(state): State<Shared<D>>,
    Extension(who): Extension<Identity>,
    Extension(session): Extension<Session>,
    multipart: Result<Multipart, MultipartRejection>,
) -> Response {
    let result = async {
        let permit = state
            .store
            .appearance
            .uploads
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::unavailable("font upload service is busy"))?;
        let mut multipart = multipart.map_err(|_| invalid())?;
        let (family, files) = tokio::time::timeout(Duration::from_secs(120), parse(&mut multipart))
            .await
            .map_err(|_| Error::unavailable("font upload timed out"))??;
        if !session.is_live(&state.directory).await {
            return Err(Error::unauthorized("session expired or revoked"));
        }
        run(move || {
            let _permit = permit;
            let _step = state.store.content().lifecycle().step(who.id.as_uuid())?;
            let mut uploads = Vec::with_capacity(files.len());
            for (name, bytes) in files {
                let format = Format::identify(&bytes)
                    .ok_or_else(|| Error::invalid_input("file must be WOFF2, WOFF, TTF or OTF"))?;
                let digest = crate::store::sha256_hex(&bytes);
                uploads.push(db::Upload {
                    face: Face::uploaded(&name, format, digest),
                    bytes,
                });
            }
            state
                .store
                .appearance
                .storage
                .write(|c| db::upload(c, &who.id.to_string(), &family, uploads))
        })
        .await
    }
    .await;
    result
        .map(|family: FontStoredFamily| Json(family).into_response())
        .unwrap_or_else(crate::api::error_response)
}

type UploadFields = (String, Vec<(String, Vec<u8>)>);

async fn parse(multipart: &mut Multipart) -> Result<UploadFields, Error> {
    let mut family = None;
    let mut files = Vec::new();
    let mut total = 0;
    while let Some(mut field) = multipart.next_field().await.map_err(|_| invalid())? {
        match field.name() {
            Some("family") if family.is_none() => {
                let mut data = Vec::new();
                while let Some(part) = field.chunk().await.map_err(|_| invalid())? {
                    if part.len() > 240 - data.len() {
                        return Err(invalid());
                    }
                    data.extend_from_slice(&part);
                }
                let data = std::str::from_utf8(&data).map_err(|_| invalid())?.trim();
                static FAMILY: LazyLock<regex::Regex> = LazyLock::new(|| {
                    regex::Regex::new(r"^[\p{L}\p{N}][\p{L}\p{N} ._-]{0,59}$").unwrap()
                });
                if !FAMILY.is_match(data) || data.eq_ignore_ascii_case("System") {
                    return Err(Error::invalid_input(
                        "font family must use letters, numbers, spaces, dots, underscores or hyphens and cannot be System",
                    ));
                }
                family = Some(data.to_owned());
            }
            Some("file") if files.len() < db::UPLOAD_FILES => {
                let filename = field
                    .file_name()
                    .filter(|name| !name.is_empty() && name.len() <= 200)
                    .ok_or_else(invalid)?
                    .to_owned();
                let mut bytes = Vec::new();
                while let Some(part) = field.chunk().await.map_err(|_| invalid())? {
                    if part.len() > db::FILE_BYTES - bytes.len()
                        || part.len() > db::UPLOAD_BYTES - total
                    {
                        return Err(Error::invalid_input(
                            "font upload exceeds the 8 MiB file or 50 MiB request limit",
                        ));
                    }
                    total += part.len();
                    bytes.extend_from_slice(&part);
                }
                files.push((filename, bytes));
            }
            _ => return Err(invalid()),
        }
    }
    let family = family.filter(|_| !files.is_empty()).ok_or_else(invalid)?;
    Ok((family, files))
}

fn invalid() -> Error {
    Error::invalid_input("multipart font upload requires one family and 1–12 file fields")
}
