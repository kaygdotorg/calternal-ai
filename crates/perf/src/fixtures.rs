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
    let (_server, mut h, _) = Server::start(&a.bin, root, &a.plugins)?;
    h.enroll(&std::fs::read_to_string(root.join("appdata/setup.token"))?)?;
    let mut ids = Vec::new();
    let mut first = Value::Null;
    for s in 0..spaces {
        let (space, _) = h.op(
            "spaces.create",
            json!({"name":format!("Performance fixture {s}")}),
        )?;
        let id = space["space"]["id"].clone();
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
pub(crate) fn copy(source: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
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
