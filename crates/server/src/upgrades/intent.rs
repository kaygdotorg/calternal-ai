//! Durable local state around the point where a release can admit writes.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const FORMAT: &str = "calternal-upgrade-intent/v1";
const LIMIT: u64 = 32 * 1024 * 1024;

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
        }
    }

    pub(super) fn load(path: &Path, installation: Uuid) -> io::Result<Option<Self>> {
        use std::io::Read;
        let file = match std::fs::File::open(path) {
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
        calternal_fsio::format::require(&intent.format, FORMAT)?;
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
        calternal_fsio::write_atomic_mode(path, &bytes, 0o600)
    }
}
