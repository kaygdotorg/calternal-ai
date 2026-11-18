//! Reuse of disposable worker files under the locked, private installation root.
use rustix::fs::{Mode, OFlags, open};
use std::{collections::HashSet, fs, io, io::Read, os::unix::fs::MetadataExt, path::Path};

/// Read through a no-follow descriptor and compare to the verified archive.
/// Unreadable or unexpected content is a cache miss. No fsync on a hit.
pub(super) fn matches_private_file(path: &Path, expected: &[u8]) -> bool {
    let Ok(descriptor) = open(
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return false;
    };
    let file = fs::File::from(descriptor);
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    if !metadata.is_file()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.nlink() != 1
        || metadata.len() != expected.len() as u64
    {
        return false;
    }
    let mut bytes = Vec::with_capacity(expected.len());
    file.take(expected.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .is_ok()
        && bytes == expected
}

/// Keep only the installed and retained regular JavaScript cache entries.
/// One cache directory scan. The catalog remains the source of truth if a
/// crash leaves a stale file behind.
pub(super) fn remove_stale<'a>(
    cache: &Path,
    active: impl Iterator<Item = &'a Path>,
) -> io::Result<()> {
    let active: HashSet<_> = active.collect();
    for entry in fs::read_dir(cache)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "js")
            && !active.contains(path.as_path())
            && entry.file_type()?.is_file()
        {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}
