//! Public signed machine releases and a private single-use enrollment command.
use super::*;
use crate::auth::Directory;
use axum::{extract::Path, http::StatusCode, response::IntoResponse};
use calternal_core::MachineInstallResult;
use serde_json::{Value, json};

const INSTALLER: &str = include_str!("../../../../scripts/install-machine.sh");
const PUBLIC_INSTALLER: &str = "https://calternal.com/install";
const TTL: Duration = Duration::from_secs(15 * 60);

/// Issue a personal install command containing the one-time code for this
/// person session. The public bootstrap never carries an enrollment code.
pub(crate) async fn script<D: Directory>(
    state: crate::api::Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
) -> Result<Value, Error> {
    let server = state
        .directory
        .http_auth()
        .ok_or_else(|| Error::unavailable("configure the public origin before enrolling machines"))?
        .config
        .origin_header();
    let image = std::env::var("CALTERNAL_MACHINE_INSTALL_IMAGE").map_err(|_| {
        Error::unavailable("Ask the server administrator to configure the standard sandbox image.")
    })?;
    if image.is_empty()
        || !image
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"./:@_-".contains(&b))
    {
        return Err(Error::unavailable(
            "The configured sandbox image name is invalid.",
        ));
    }
    blocking(move || {
        let (code, expires_ms) = machines.issue_code_for(Owner::Account { id: account }, TTL)?;
        let script = format!(
            "curl --disable --proto '=https' --tlsv1.2 -fsS {} | CALTERNAL_ENROLL_CODE={} sh -s -- --server {} --image {}",
            quote(PUBLIC_INSTALLER),
            quote(&code),
            quote(&server),
            quote(&image),
        );
        Ok(json!(MachineInstallResult { script, expires_ms }))
    })
    .await
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) async fn installer() -> axum::response::Response {
    let key = calternal_machine::release::public_key().unwrap_or_default();
    let script = INSTALLER.replacen("release_key=", &format!("release_key={}", quote(key)), 1);
    (
        [
            ("content-type", "text/plain; charset=utf-8"),
            ("cache-control", "no-store"),
        ],
        script,
    )
        .into_response()
}

pub(super) async fn version() -> axum::response::Response {
    match distribution() {
        Some((id, _)) => ([("cache-control", "no-store")], format!("{id}\n")).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn distribution() -> Option<(&'static str, std::path::PathBuf)> {
    calternal_machine::release::public_key()?;
    let id = calternal_machine::release::build_id()?;
    let path = std::path::PathBuf::from(std::env::var_os("CALTERNAL_MACHINE_RELEASE_DIR")?);
    path.is_absolute().then_some((id, path))
}

pub(super) async fn asset(
    Path((version, platform, asset)): Path<(String, String, String)>,
) -> axum::response::Response {
    use calternal_fsio::beneath::Folder;
    use calternal_machine::release::MAX_BINARY;
    use tokio::io::AsyncReadExt as _;

    let Some((current, root)) = distribution() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if version != current
        || !matches!(
            platform.as_str(),
            "linux-x86_64" | "linux-aarch64" | "macos-x86_64" | "macos-aarch64"
        )
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    let limit = match asset.as_str() {
        "manifest" => 512,
        "manifest.sig" => 4096,
        "calternal" => MAX_BINARY,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let file = tokio::task::spawn_blocking(move || {
        let folder = Folder::open_existing(&root)?
            .child(current, false)?
            .child(&platform, false)?;
        let file = folder.open_file(&asset)?;
        let size = file.metadata()?.len();
        Ok::<_, std::io::Error>((file, size))
    })
    .await;
    let Ok(Ok((file, size))) = file else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if size == 0 || size > limit {
        return StatusCode::NOT_FOUND.into_response();
    }
    let file = tokio::fs::File::from_std(file).take(size);
    let stream = futures_util::stream::try_unfold(file, |mut file| async move {
        let mut bytes = vec![0; 64 * 1024];
        let len = file.read(&mut bytes).await?;
        bytes.truncate(len);
        Ok::<_, std::io::Error>((len > 0).then_some((bytes, file)))
    });
    (
        [
            ("content-type", "application/octet-stream"),
            ("cache-control", "no-store"),
            ("content-length", &size.to_string()),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response()
}
