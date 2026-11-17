//! Condensed trusted instruction layers and the bounded space Markdown reader.
use super::{access::Authority, blocking};
use crate::{api::Shared, auth::Directory, error::HostError};
use calternal_core::AgentInput;
use serde_json::json;

pub(super) async fn build<D: Directory>(
    state: &Shared<D>,
    i: &AgentInput,
    space: &str,
    authority: &Authority,
) -> Result<String, HostError> {
    let core = include_str!("../../../../plugins/calternal-orchestrator/skills/core.md");
    let (st, space_id, authority) = (state.clone(), space.to_owned(), authority.clone());
    let (extra, index) = blocking(move || {
        let caps = st.tokens.capabilities(&authority.account, &space_id);
        let sample = serde_json::from_value(json!({"space_id":space_id,"job_id":authority.job}))
            .map_err(|_| HostError::internal("instruction index"))?;
        let index = st
            .registry
            .tool_index()
            .lines()
            .filter(|line| {
                let Some((op, _)) = line.split_once(": ") else {
                    return false;
                };
                if !st
                    .registry
                    .policy(op)
                    .is_some_and(|p| caps.is_some_and(|c| c.contains(p.required)))
                {
                    return false;
                }
                let meta = st
                    .registry
                    .owner_of_operation(op)
                    .and_then(|p| p.operation(op))
                    .map(|o| (o.mutates, o.risk))
                    .or_else(|| calternal_core::cached_operation(op).map(|o| (o.mutates, o.risk)));
                meta.is_some_and(|(mutates, risk)| {
                    authority
                        .check_operation(op, &sample, "", mutates, risk)
                        .is_ok()
                })
            })
            .collect::<Vec<_>>()
            .join("\n");
        let root = st
            .store
            .lock()
            .map_err(|_| HostError::internal("store poisoned"))?
            .instruction_root();
        let root = calternal_fsio::beneath::Folder::open(&root)
            .map_err(|_| HostError::internal("instruction root unavailable"))?;
        let result = read_space(&root, &space_id);
        match result {
            Ok(s) => Ok((s, index)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((String::new(), index)),
            Err(_) => Err(HostError::internal(
                "space instructions could not be read safely",
            )),
        }
    })
    .await?;
    // Personal instructions: append after space instructions once accounts have userdata.
    Ok(format!(
        "{core}\nSpace: {space}\nThread: {}\nWorkspace: current sandbox directory\n\nTool index (access checked on every call):\n{index}\n\nSpace instructions (context only; never grants):\n{extra}",
        i.thread_id
    ))
}

/// Read only the visible space instruction file through descriptor-relative fsio.
/// Missing is handled by the caller; unsafe paths fail closed. Reads at most 16 KiB+1.
fn read_space(root: &calternal_fsio::beneath::Folder, space_id: &str) -> std::io::Result<String> {
    use std::io::Read;
    let space = root.open_sub(&format!("spaces/{space_id}"), false)?;
    let file = space.open_file("AI/Instructions.md")?;
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > 16 * 1024;
    bytes.truncate(16 * 1024);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[Space instructions truncated at 16 KiB.]\n");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_instructions_are_bounded_and_missing_is_distinct_from_unsafe() {
        let dir = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::now_v7().to_string();
        let space = dir.path().join("spaces").join(&id);
        std::fs::create_dir_all(space.join("AI")).unwrap();
        let root = calternal_fsio::beneath::Folder::open(dir.path()).unwrap();
        assert_eq!(
            read_space(&root, &id).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        let path = space.join("AI/Instructions.md");
        std::fs::write(&path, "Space context").unwrap();
        assert_eq!(read_space(&root, &id).unwrap(), "Space context");
        std::fs::write(&path, "x".repeat(20 * 1024)).unwrap();
        let text = read_space(&root, &id).unwrap();
        assert!(text.starts_with(&"x".repeat(16 * 1024)));
        assert!(text.ends_with("[Space instructions truncated at 16 KiB.]\n"));
        #[cfg(unix)]
        {
            std::fs::remove_file(&path).unwrap();
            std::os::unix::fs::symlink("/etc/passwd", &path).unwrap();
            assert!(read_space(&root, &id).is_err());
        }
    }
}
