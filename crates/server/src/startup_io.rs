//! Durable startup files use blocking workers rather than the async root task.
use std::path::Path;

pub(super) async fn write_private_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (path, bytes) = (path.to_owned(), bytes.to_owned());
    tokio::task::spawn_blocking(move || calternal_fsio::write_atomic_mode(&path, &bytes, 0o600))
        .await
        .map_err(std::io::Error::other)?
}
