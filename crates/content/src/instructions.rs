//! Placement and journal recovery for the single reserved space file.
use super::*;
use calternal_fsio::beneath::Folder;
use std::io::{ErrorKind, Read, Write};

pub(super) const INTENT: &str = "standing-instructions";
const LIMIT: usize = 16 * 1024;

fn folder(root: &Folder, id: SpaceId, create: bool) -> std::io::Result<Folder> {
    let (directory, _) = calternal_types::SPACE_INSTRUCTIONS_PATH
        .split_once('/')
        .expect("reserved path");
    root.child("spaces", create)?
        .child(&id.to_string(), create)?
        .child(directory, create)
}

fn filename() -> &'static str {
    calternal_types::SPACE_INSTRUCTIONS_PATH
        .split_once('/')
        .expect("reserved path")
        .1
}

/// Read at most 16 KiB of standing instructions below the UUID directory.
/// A missing file gives empty context. Unsafe paths fail closed. Oversized
/// externally provisioned text is truncated and labelled.
fn read_instructions(root: &Folder, id: SpaceId) -> std::io::Result<String> {
    let file = match folder(root, id, false).and_then(|ai| ai.open_file(filename())) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(String::new()),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take((LIMIT + 1) as u64).read_to_end(&mut bytes)?;
    let truncated = bytes.len() > LIMIT;
    bytes.truncate(LIMIT);
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[Space instructions truncated at 16 KiB.]\n");
    }
    Ok(text)
}

impl Content {
    /// Read the reserved Markdown after finishing any pending replacement.
    /// Costs one space lock, pending publication checks and one bounded file read.
    pub fn standing_instructions(&self, space: SpaceId) -> Result<String> {
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            let root = Folder::open(&self.roots.userdata_ssd).map_err(internal)?;
            read_instructions(&root, space).map_err(internal)
        })
    }

    /// Replace only AI/AGENTS.md. The per-space lock spans the durable intent,
    /// descriptor-relative placement and journal receipt to serialize replacements.
    /// A crash replays the bounded intent before later writes or reads. The journal
    /// records only the mutation, and retrying a matching receipt does no new write.
    pub fn replace_standing_instructions(
        &self,
        space: SpaceId,
        request: Request<'_>,
        markdown: &str,
    ) -> Result<()> {
        let _actor = self.lifecycle.step(request.account.as_uuid())?;
        if markdown.len() > LIMIT {
            return Err(Error::invalid_input("standing instructions exceed 16 KiB"));
        }
        self.with_space(space, |s| {
            self.publish_pending(s)?;
            if let Some(saved) = self
                .journals
                .receipt(&s.journal, request.account, request.id)?
            {
                return if saved.operation == request.operation
                    && saved.fingerprint == request.fingerprint
                {
                    Ok(())
                } else {
                    Err(Error::request_reused())
                };
            }
            let root = Folder::open(&self.roots.userdata_ssd).map_err(internal)?;
            folder(&root, space, true)
                .and_then(|ai| ai.check_replacement(filename()))
                .map_err(internal)?;
            s.conn
                .execute(
                    "INSERT INTO outbox VALUES(?1,?2,?3,?4,NULL,?5,?6)",
                    params![
                        request.account.to_string(),
                        request.id,
                        request.operation,
                        request.fingerprint,
                        INTENT,
                        request.outbox_result(&Value::String(markdown.to_owned()))
                    ],
                )
                .map_err(internal)?;
            (self.fault)(FaultPoint::AfterCommit);
            self.publish_pending(s)
        })
    }

    pub(super) fn place_instructions(&self, id: SpaceId, payload: &Value) -> Result<()> {
        let markdown = payload
            .as_str()
            .ok_or_else(|| internal("invalid standing instructions intent"))?;
        let root = Folder::open(&self.roots.userdata_ssd).map_err(internal)?;
        let ai = folder(&root, id, true).map_err(internal)?;
        let staged_name = format!("{}standing-instructions", calternal_fsio::TEMP_PREFIX);
        match ai.remove(&staged_name) {
            Ok(()) => (),
            Err(e) if e.kind() == ErrorKind::NotFound => (),
            Err(e) => return Err(internal(e)),
        }
        let mut staged = ai.create_temp(&staged_name).map_err(internal)?;
        staged
            .file()
            .write_all(markdown.as_bytes())
            .map_err(internal)?;
        staged.replace(filename()).map_err(internal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_the_reserved_file_under_the_selected_uuid() {
        let temp = tempfile::tempdir().unwrap();
        let root = Folder::open(temp.path()).unwrap();
        let id = SpaceId::new();
        let ai = folder(&root, id, true).unwrap();
        let path = temp.path().join("spaces").join(id.to_string()).join("AI");
        std::fs::write(path.join("Instructions.md"), "Previous name").unwrap();
        assert_eq!(read_instructions(&root, id).unwrap(), "");
        let mut file = ai.create_temp(".calternal-tmp-test").unwrap();
        file.file().write_all(&vec![b'x'; LIMIT + 1]).unwrap();
        file.replace(filename()).unwrap();
        let text = read_instructions(&root, id).unwrap();
        assert!(text.starts_with(&"x".repeat(LIMIT)));
        assert!(text.ends_with(
            "[Space instructions truncated at 16 KiB.]
"
        ));

        let alias = SpaceId::new();
        std::os::unix::fs::symlink(
            path.parent().unwrap(),
            temp.path().join("spaces").join(alias.to_string()),
        )
        .unwrap();
        assert!(read_instructions(&root, alias).is_err());
    }
}
