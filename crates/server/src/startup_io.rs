//! Durable startup files use blocking workers rather than the async root task.
use std::path::Path;

/// Publish an owner-only startup file durably on a blocking worker. Returns I/O
/// and worker failures; costs one byte copy, one task and file/directory syncs.
pub(super) async fn write_private_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (path, bytes) = (path.to_owned(), bytes.to_owned());
    tokio::task::spawn_blocking(move || calternal_fsio::write_atomic_mode(&path, &bytes, 0o600))
        .await
        .map_err(std::io::Error::other)?
}

/// Fixed next step for startup storage errors; borrowed text with no I/O.
pub(super) const DATA_HINT: &str =
    "Check that --appdata exists (or can be created) and that you can write to it.";

/// Open validated storage and the trusted plugin catalog on a blocking worker.
/// Recovery remains eager for authority; content spaces recover lazily. Refuses
/// mismatched roots or unreadable recovery state before any request is admitted.
pub(super) fn open_stores(
    roots: calternal_roots::Roots,
    installation: uuid::Uuid,
    create: bool,
    receipt_retention: std::time::Duration,
    plugin_root: Option<std::path::PathBuf>,
    test_plugin_root: Option<std::path::PathBuf>,
) -> Result<
    (
        calternal_server::store::Store,
        std::path::PathBuf,
        calternal_recovery::Coordinator,
        calternal_server::plugins::Registry,
    ),
    calternal_cli::output::CliError,
> {
    use super::boot;
    use calternal_server::{plugins::Registry, store::Store};
    let store = Store::open(&roots, installation, create, receipt_retention).map_err(|e| {
        boot(
            1,
            "failed",
            e.message,
            "Pass the three roots this installation was set up with (--create only for \
             first setup), and make sure no other server is using them.",
        )
    })?;
    let data_dir = std::fs::canonicalize(&roots.appdata)
        .map_err(|e| boot(1, "io", format!("appdata: {e}"), DATA_HINT))?;
    // Every security change is written to this log in userdata-ssd before it is
    // acknowledged (crates/recovery), so auth.db can be rebuilt if appdata is lost.
    let recovery = store.open_recovery(create).map_err(|e| {
        boot(
            1,
            "failed",
            e.message,
            "The recovery log in userdata-ssd is missing, damaged or belongs to another installation.",
        )
    })?;
    let test_root = test_plugin_root.map(|r| std::fs::canonicalize(&r).unwrap_or(r));
    let registry = if let Some(root) = plugin_root {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        Registry::load_with_tests(&root, test_root.as_deref())
    } else {
        Registry::load_server_defaults(test_root.as_deref())
    };
    calternal_server::api::configure_search(&store, &registry)
        .map_err(|e| boot(1, "failed", e.message, DATA_HINT))?;
    Ok((store, data_dir, recovery, registry))
}
