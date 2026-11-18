//! Bounded binary I/O for trusted private asset caches.

use calternal_fsio::beneath::Folder;
use std::{
    io::{self, Read},
    path::Path,
};

/// Read one regular file from a server-owned private directory, refusing
/// symlinks and non-regular files. Missing or oversized bytes are cache misses.
/// Cost: one directory open, one file open, and a read bounded by `cap + 1`.
/// Call on a blocking worker.
pub(crate) fn read(path: &Path, cap: usize) -> io::Result<Option<Vec<u8>>> {
    let dir = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let file = match Folder::open_existing(dir).and_then(|dir| dir.open_file(name)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    file.take((cap as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok((bytes.len() <= cap).then_some(bytes))
}

/// Atomically replace a file with owner-only bytes in a trusted private directory.
/// The replacement is either old or complete after interruption. Call on a
/// blocking worker. Cost: one bounded write, one rename, two fsyncs.
pub(crate) fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    calternal_fsio::write_atomic_mode(path, bytes, 0o600)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn cache_writes_private_replacements_and_never_reads_a_symlink() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("cached");
        write(&path, b"before").unwrap();
        write(&path, b"after").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(read(&path, 5).unwrap().unwrap(), b"after");
        assert!(read(&path, 4).unwrap().is_none());
        let link = root.path().join("link");
        symlink(&path, &link).unwrap();
        assert!(!matches!(read(&link, 5), Ok(Some(_))));
    }
}
