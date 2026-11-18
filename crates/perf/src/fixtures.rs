//! Seed once through the real API, then copy stopped roots for identical AB/BA arms.
use crate::{Args, http::Result, server::Server};
use serde_json::{Value, json};
use std::path::Path;
pub(crate) struct Fixture {
    pub(crate) cookie: String,
    pub(crate) spaces: Vec<Value>,
    pub(crate) first: Value,
}
pub(crate) fn build(a: &Args, root: &Path, notes: usize, spaces: usize) -> Result<Fixture> {
    let (_server, mut h, _) = Server::start(
        &a.bin,
        root,
        if a.paired_search {
            a.baseline_plugins.as_deref().unwrap_or(&a.plugins)
        } else {
            &a.plugins
        },
        a.test_plugin_root.as_deref(),
        a.startup_timeout_seconds,
    )?;
    h.enroll(&std::fs::read_to_string(root.join("appdata/setup.token"))?)?;
    // Account enrollment provisions one owned space in the same authority commit.
    // Reuse its UUID so fixture cardinality includes every space in the account.
    let (directory, _) = h.op("spaces.list", json!({}))?;
    let personal = directory["spaces"]
        .as_array()
        .filter(|rows| rows.len() == 1)
        .and_then(|rows| rows.first())
        .filter(|row| row["personal"] == true)
        .map(|row| row["id"].clone())
        .ok_or("fixture enrollment must provision exactly one personal space; use a matching server build")?;
    let mut ids = Vec::new();
    let mut first = Value::Null;
    for s in 0..spaces {
        let id = if s == 0 {
            personal.clone()
        } else {
            let (space, _) = h.op(
                "spaces.create",
                json!({"name":format!("Performance fixture {s}")}),
            )?;
            space["space"]["id"].clone()
        };
        for i in 0..notes {
            let body = format!(
                "# Note {i}\n\nseed: {}\n#perf #work\n- [ ] deterministic task\n{}\n",
                a.seed,
                "fixture text ".repeat(if i % 100 == 0 { 100 } else { 10 })
            );
            let (n, _) = h.op(
                "notes.create",
                json!({"space_id":id,"title":format!("Fixture {i:06}"),"markdown":body}),
            )?;
            if s == 0 && i == 0 {
                first = n["note"].clone();
            }
            if i % 1000 == 0 {
                eprintln!("fixture {spaces}x{notes}: space {s}, note {i}");
            }
        }
        ids.push(id);
    }
    Ok(Fixture {
        cookie: h.cookie,
        spaces: ids,
        first,
    })
}
/// Prepare a fresh, owner-only directory for a disposable perf run.
pub(crate) fn private_work(path: &Path) -> Result<()> {
    if path.symlink_metadata().is_ok() {
        return Err("use a fresh --work directory".into());
    }
    Ok(calternal_fsio::private::directory(path)?)
}

pub(crate) fn copy(source: &Path, dest: &Path) -> Result<()> {
    calternal_fsio::private::directory(dest)?;
    for d in ["appdata", "userdata-ssd", "userdata-hdd"] {
        if !std::process::Command::new("cp")
            .args(["-a", "--reflink=auto"])
            .arg(source.join(d))
            .arg(dest.join(d))
            .status()?
            .success()
        {
            return Err("copying stopped fixture failed".into());
        }
    }
    Ok(())
}
pub(crate) fn remove_roots(root: &Path) -> Result<()> {
    for d in ["appdata", "userdata-ssd", "userdata-hdd"] {
        std::fs::remove_dir_all(root.join(d))?;
    }
    Ok(())
}

// Build disposable roots for startup-only profiling; no cookie or passkey is exported.
pub(crate) fn prepare(a: &Args) -> Result<i32> {
    let notes: usize = a.sizes.parse()?;
    let spaces: usize = a.spaces.parse()?;
    if notes == 0
        || notes > 100_000
        || spaces == 0
        || spaces > 1000
        || notes.checked_mul(spaces).is_none_or(|n| n > 100_000)
    {
        return Err("prepare at most 100000 notes across at most 1000 spaces".into());
    }
    if a.work.exists() {
        return Err("prepared roots already exist".into());
    }
    let staging = tempfile::Builder::new()
        .prefix("calternal-perf-prepare-")
        .tempdir_in("/dev/shm")?;
    build(a, staging.path(), notes, spaces)?;
    copy(staging.path(), &a.work)?;
    let report = serde_json::json!({"prepared":a.work,"spaces":spaces,"notes_per_space":notes,"timed":false});
    crate::report::print(a, &report, &a.work);
    Ok(0)
}

/// Reset the stopped fixture's package catalog so startup seeds the chosen
/// on/off manifest. The copied authority, journal and note files remain intact.
pub(crate) fn reset_search_catalog(root: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let path = root
            .join("appdata")
            .join(format!("installed-plugins.sqlite3{suffix}"));
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
