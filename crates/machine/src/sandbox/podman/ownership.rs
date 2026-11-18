//! Podman artifact ownership is the canonical machine sandbox root.

use super::{SandboxResult, podman};
use std::path::Path;

const ROOT: &str = "org.calternal.sandbox-root";
const WORK_MOUNT: &str =
    "{{.Id}} {{range .Mounts}}{{if eq .Destination \"/work\"}}{{.Source}}{{end}}{{end}}";
const PREVIOUS_WORKER_ROOT: &str = "org.calternal.worker-root";

/// Identifies the canonical host root responsible for an engine container.
pub(super) fn root_label(root: &Path) -> String {
    format!("{ROOT}={}", root.display())
}

/// Remove known IDs in engine-sized batches. Absent IDs count as removed.
pub(super) async fn remove(engine: &Path, ids: &[&str]) -> SandboxResult<()> {
    for batch in ids.chunks(64) {
        let mut args = vec!["rm", "-f", "-t", "0", "--ignore"];
        args.extend_from_slice(batch);
        podman(engine, &args).await?;
    }
    Ok(())
}

async fn labelled(engine: &Path, label: &str, name: Option<&str>) -> SandboxResult<String> {
    let filter = format!("label={label}");
    let mut args = vec!["ps", "-aq", "--filter", &filter];
    if let Some(name) = name {
        args.extend(["--filter", name]);
    }
    podman(engine, &args).await
}

/// Find the named container by root or verified mount, then remove by ID.
pub(super) async fn remove_named(engine: &Path, root: &Path, name: &str) -> SandboxResult<()> {
    let filter = format!("name=^{name}$");
    let ids = labelled(engine, &root_label(root), Some(&filter)).await?;
    remove(engine, &ids.split_whitespace().collect::<Vec<_>>()).await?;
    if let Ok(rows) = podman(
        engine,
        &["container", "inspect", "--format", WORK_MOUNT, name],
    )
    .await
    {
        remove(engine, &owned(root, &rows)).await?;
    }
    Ok(())
}

/// Remove account containers owned by this machine's canonical sandbox root.
pub(super) async fn remove_account(
    engine: &Path,
    root: &Path,
    account: uuid::Uuid,
) -> SandboxResult<()> {
    let filter = format!("name=^calternal-account-{account}-thread-");
    let ids = labelled(engine, &root_label(root), Some(&filter)).await?;
    remove(engine, &ids.split_whitespace().collect::<Vec<_>>()).await
}

/// Reclaim machine-owned containers on startup. The caller holds the root lock.
/// Lists engine labels and inspects mount sources in batches for older artifacts.
pub(super) async fn sweep(engine: &Path, root: &Path) -> SandboxResult<()> {
    let ids = labelled(engine, &root_label(root), None).await?;
    remove(engine, &ids.split_whitespace().collect::<Vec<_>>()).await?;
    let old_workers = format!("{PREVIOUS_WORKER_ROOT}={}", root.display());
    let ids = labelled(engine, &old_workers, None).await?;
    remove(engine, &ids.split_whitespace().collect::<Vec<_>>()).await?;

    // Earlier thread containers carried only a thread label. Their /work bind
    // source proves ownership; a name or account ID alone is insufficient.
    let ids = labelled(engine, "org.calternal.thread", None).await?;
    let ids: Vec<_> = ids.split_whitespace().collect();
    for batch in ids.chunks(64) {
        let mut args = vec!["container", "inspect", "--format", WORK_MOUNT];
        args.extend_from_slice(batch);
        if let Ok(rows) = podman(engine, &args).await {
            remove(engine, &owned(root, &rows)).await?;
        }
    }
    Ok(())
}

fn owned<'a>(root: &Path, rows: &'a str) -> Vec<&'a str> {
    rows.lines()
        .filter_map(|line| {
            let (id, path) = line.split_once(' ')?;
            is_thread_work(root, Path::new(path)).then_some(id)
        })
        .collect()
}

fn is_thread_work(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let Some(parts) = relative
        .iter()
        .map(|part| part.to_str())
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    matches!(parts.as_slice(), ["accounts", account, "threads", thread, "work"]
        if uuid::Uuid::parse_str(account).is_ok() && uuid::Uuid::parse_str(thread).is_ok())
}
