//! Cached file identity and incremental hash state verified outside admission.
use super::io::WITNESS_BYTES;
use crate::{
    Error, Result,
    types::{blocked, internal},
};
use calternal_fsio::beneath::Folder;
use std::{
    fs::Metadata,
    io::{self, Read},
    os::unix::fs::MetadataExt,
    path::PathBuf,
};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Stamp {
    device: u64,
    inode: u64,
    length: u64,
    change: (i64, i64),
    modified: (i64, i64),
}

impl Stamp {
    pub(crate) fn new(meta: &Metadata) -> Self {
        Self {
            device: meta.dev(),
            inode: meta.ino(),
            length: meta.len(),
            change: (meta.ctime(), meta.ctime_nsec()),
            modified: (meta.mtime(), meta.mtime_nsec()),
        }
    }

    pub(super) fn len(&self) -> u64 {
        self.length
    }
}

#[derive(Clone)]
pub(crate) struct Ready {
    pub(crate) seq: i64,
    pub(crate) stamp: Stamp,
    pub(super) hasher: blake3::Hasher,
    pub(super) tail: Vec<u8>,
}

impl Ready {
    /// Advance the digest and tail using only the one committed fragment.
    pub(super) fn advance(&mut self, record: &[u8]) {
        self.hasher.update(record);
        let keep = WITNESS_BYTES.saturating_sub(record.len());
        self.tail.drain(..self.tail.len().saturating_sub(keep));
        self.tail
            .extend_from_slice(&record[record.len().saturating_sub(WITNESS_BYTES)..]);
    }
}

pub(crate) struct Task {
    pub(crate) id: Uuid,
    pub(crate) seq: i64,
    pub(crate) root: PathBuf,
    pub(crate) path: String,
    pub(crate) prefix: u64,
    pub(crate) digest: String,
}

impl Task {
    pub(crate) fn run(&self) -> Result<Ready> {
        let folder = Folder::open_existing(&self.root).map_err(internal)?;
        let mut file = folder.open_file(&self.path).map_err(file_error)?;
        let stamp = Stamp::new(&file.metadata().map_err(internal)?);
        if stamp.len() < self.prefix {
            return Err(blocked());
        }
        let mut hasher = blake3::Hasher::new();
        let mut remaining = self.prefix;
        let mut buffer = [0; 32 * 1024];
        let mut tail = Vec::new();
        while remaining > 0 {
            let size = remaining.min(buffer.len() as u64) as usize;
            let bytes = &mut buffer[..size];
            file.read_exact(bytes).map_err(file_error)?;
            hasher.update(bytes);
            let keep = WITNESS_BYTES.saturating_sub(size);
            tail.drain(..tail.len().saturating_sub(keep));
            tail.extend_from_slice(&bytes[size.saturating_sub(WITNESS_BYTES)..]);
            remaining -= size as u64;
        }
        if hasher.finalize().to_hex().as_str() != self.digest
            || Stamp::new(&file.metadata().map_err(internal)?) != stamp
        {
            return Err(blocked());
        }
        Ok(Ready {
            seq: self.seq,
            stamp,
            hasher,
            tail,
        })
    }
}

pub(crate) fn pending(error: &Error) -> bool {
    error
        .details
        .as_ref()
        .is_some_and(|d| d["reason"] == "thread_verifying")
}

pub(crate) fn verifying() -> Error {
    Error::unavailable("verifying the thread file; retry shortly")
        .with_details(serde_json::json!({"reason":"thread_verifying"}))
}

pub(super) fn file_error(error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::NotFound | io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
            blocked()
        }
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => verifying(),
        _ => internal(error),
    }
}
