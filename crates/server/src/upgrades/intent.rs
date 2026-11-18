//! Durable local state around the point where a release can admit writes.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FORMAT: &str = "calternal-upgrade-intent/v1";
const FULL_FORMAT: &str = "calternal-upgrade-intent/v2";
pub(super) const LIMIT: u64 = 32 * 1024 * 1024;

/// A deployment's recovery decision. Preparing has a complete restore point.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Phase {
    Copying,
    Preparing,
    Committed,
}

/// One validated role-relative file and its state before migration.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SavedFile {
    pub(super) root: super::Root,
    pub(super) path: String,
    /// None records an absent SQLite sidecar or a file the step will create.
    pub(super) copy: Option<CopyProof>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CopyProof {
    pub(super) mode: u32,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Intent {
    format: String,
    pub(super) installation: Uuid,
    pub(super) previous: String,
    pub(super) candidate: String,
    pub(super) phase: Phase,
    pub(super) files: Vec<SavedFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) entries: Option<Vec<super::full::Entry>>,
}

impl Intent {
    pub(super) fn copying(installation: Uuid, previous: String, candidate: String) -> Self {
        Self {
            format: FORMAT.into(),
            installation,
            previous,
            candidate,
            phase: Phase::Copying,
            files: Vec::new(),
            entries: None,
        }
    }

    pub(super) fn make_full(&mut self) {
        self.format = FULL_FORMAT.into();
        self.entries = Some(Vec::new());
    }

    pub(super) fn needs_recovery(&self) -> bool {
        self.phase != Phase::Committed || self.entries.is_some() || !self.files.is_empty()
    }

    pub(super) fn retire(&mut self) {
        self.format = FORMAT.into();
        self.files.clear();
        self.entries = None;
    }

    pub(super) fn load(path: &Path, installation: Uuid) -> io::Result<Option<Self>> {
        use std::io::Read;
        let file = match open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        file.take(LIMIT + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > LIMIT {
            return Err(io::Error::other("upgrade intent exceeds its size limit"));
        }
        let intent: Self = serde_json::from_slice(&bytes)?;
        match intent.format.as_str() {
            FORMAT if intent.entries.is_none() => {}
            FULL_FORMAT if intent.entries.is_some() && intent.files.is_empty() => {}
            _ => {
                return Err(io::Error::other(
                    "unsupported or invalid upgrade intent format",
                ));
            }
        }
        if intent.phase == Phase::Copying
            && (!intent.files.is_empty() || intent.entries.as_ref().is_some_and(|e| !e.is_empty()))
        {
            return Err(io::Error::other(
                "copying intent must have an empty manifest",
            ));
        }
        if intent.installation != installation {
            return Err(io::Error::other(
                "upgrade intent belongs to another installation",
            ));
        }
        Ok(Some(intent))
    }

    pub(super) fn publish(&self, path: &Path) -> io::Result<()> {
        let bytes = serde_json::to_vec(&self)?;
        if bytes.len() as u64 > LIMIT {
            return Err(io::Error::other("upgrade intent exceeds its size limit"));
        }
        write(path, &bytes)
    }
}

#[cfg(target_os = "linux")]
fn open(path: &Path) -> io::Result<std::fs::File> {
    folder(path)?.file(path.file_name().expect("intent basename"))
}

#[cfg(target_os = "linux")]
fn folder(path: &Path) -> io::Result<super::tree::Tree> {
    let system = path.parent().expect("intent parent");
    super::tree::Tree::open(system.parent().expect("appdata root"))?
        .directory(system.file_name().expect("system basename"))
}

#[cfg(not(target_os = "linux"))]
fn open(path: &Path) -> io::Result<std::fs::File> {
    std::fs::File::open(path)
}

#[cfg(target_os = "linux")]
fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let folder = folder(path)?;
    let temporary = format!("{}upgrade-intent", calternal_fsio::TEMP_PREFIX);
    let stage = std::ffi::OsStr::new(&temporary);
    match folder.remove(stage, rustix::fs::AtFlags::empty()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut file = folder.create(stage)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    folder.replace(stage, &folder, path.file_name().expect("intent basename"))
}

#[cfg(not(target_os = "linux"))]
fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    calternal_fsio::write_atomic_mode(path, bytes, 0o600)
}

pub(super) fn remove(path: &Path) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        folder(path)?.remove(
            path.file_name().expect("intent basename"),
            rustix::fs::AtFlags::empty(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::fs::remove_file(path)?;
        calternal_fsio::sync_dir(path.parent().expect("intent parent"))
    }
}
