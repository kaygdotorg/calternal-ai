//! A disposable image of a new folder. Its files can be written and synced
//! while history commits; publishing is one exclusive directory rename.
//! Existing folders and different inheritance policies use ordinary writes.

use crate::{DataLock, TEMP_PREFIX, parent};
use std::fs;
#[cfg(target_os = "linux")]
use std::fs::{File, OpenOptions};
use std::io;
#[cfg(target_os = "linux")]
use std::io::Write;
use std::path::{Path, PathBuf};

/// A synced folder image outside the watched tree. Dropping an unpublished
/// image removes only its private cache. The lock keeps recovery from removing
/// a live writer's cache, including one whose caller stopped waiting.
pub struct PreparedDirectory {
    cache: PathBuf,
    image: PathBuf,
    target: PathBuf,
    files: Vec<(PathBuf, Vec<u8>)>,
    _lock: DataLock,
    #[cfg(target_os = "linux")]
    policy: Option<crate::BulkStage>,
}

/// A folder was published. Its parent flushes may still have failed: callers
/// must retain ownership receipts and roll back before reporting failure.
pub struct DirectoryPublication {
    /// Whether the rename is durable in both parents.
    pub synced: io::Result<()>,
}

impl PreparedDirectory {
    /// Whether this image is exactly the files the caller still plans to make.
    pub fn matches(&self, files: &[(PathBuf, Vec<u8>)]) -> bool {
        self.files == files
    }

    /// Publishes without replacing any destination, then syncs both parents.
    /// `published` observes the rename before those parent flushes.
    pub fn publish(self, published: impl FnOnce()) -> io::Result<DirectoryPublication> {
        #[cfg(target_os = "linux")]
        if !self
            .policy
            .as_ref()
            .is_some_and(|policy| policy.can_place_in(parent(&self.target).unwrap()))
        {
            return Err(io::ErrorKind::Unsupported.into());
        }
        crate::move_exclusive(&self.image, &self.target)?;
        crate::usage::wrote(
            self.files
                .iter()
                .map(|(path, bytes)| (path.as_path(), bytes.len() as u64)),
        );
        published();
        let from = parent(&self.image)?;
        let to = parent(&self.target)?;
        let synced = std::thread::scope(|scope| {
            let other = scope.spawn(|| crate::sync_dir(from));
            let here = crate::sync_dir(to);
            let there = other
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("folder flush panicked")));
            here.and(there)
        });
        Ok(DirectoryPublication { synced })
    }
}

impl Drop for PreparedDirectory {
    fn drop(&mut self) {
        // After publication the image has moved out. This path never names
        // the published folder; its remaining contents are app-owned cache.
        let _ = fs::remove_dir_all(&self.cache);
    }
}

/// Prepares a new, single-folder batch, or declines an existing destination,
/// unsupported platform, mount boundary or different inheritance policy.
/// `root` is private application metadata; `tag` is an opaque job identifier.
pub fn prepare_new_directory(
    files: Vec<(PathBuf, Vec<u8>)>,
    root: &Path,
    tag: &str,
) -> io::Result<Option<PreparedDirectory>> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (files, root, tag);
        Ok(None)
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        if tag.len() != 64 || !tag.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let Some((first, _)) = files.first() else {
            return Ok(None);
        };
        let target = parent(first)?.to_owned();
        if files
            .iter()
            .any(|(p, _)| p.parent() != Some(target.as_path()))
            || fs::symlink_metadata(&target).is_ok()
            || !parent(&target)?.is_dir()
        {
            return Ok(None);
        }
        let mut private = fs::DirBuilder::new();
        private.mode(0o700);
        match private.create(root) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let m = fs::symlink_metadata(root)?;
                if !m.is_dir() || m.permissions().mode() & 0o077 != 0 {
                    return Ok(None);
                }
            }
            Err(e) => return Err(e),
        }
        let cache = crate::temp_path(&root.join(format!("import-{tag}")))?;
        private.create(&cache)?;
        let lock = match DataLock::acquire_existing(&cache) {
            Ok(lock) => lock,
            Err(e) => {
                let _ = fs::remove_dir_all(&cache);
                return Err(e);
            }
        };
        let mut prepared = PreparedDirectory {
            image: cache.join("image"),
            cache,
            target,
            files,
            _lock: lock,
            policy: None,
        };
        let policy = crate::BulkStage::open(&prepared.cache);
        if !policy
            .as_ref()
            .is_some_and(|s| s.can_place_in(parent(&prepared.target).unwrap()))
        {
            return Ok(None);
        }
        prepared.policy = policy;
        fs::create_dir(&prepared.image)?;
        // Observe writeback errors from the whole batch, including writes
        // that the kernel flushes before the final syncfs call.
        let image_file = File::open(&prepared.image)?;
        let started = std::time::Instant::now();
        std::thread::scope(|scope| -> io::Result<()> {
            let chunk = prepared.files.len().div_ceil(4).max(1);
            let handles: Vec<_> = prepared
                .files
                .chunks(chunk)
                .map(|part| {
                    let image = &prepared.image;
                    scope.spawn(move || -> io::Result<()> {
                        for (path, bytes) in part {
                            let name = path.file_name().ok_or(io::ErrorKind::InvalidInput)?;
                            let mut file = OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(image.join(name))?;
                            file.write_all(bytes)?;
                        }
                        Ok(())
                    })
                })
                .collect();
            for handle in handles {
                handle
                    .join()
                    .map_err(|_| io::Error::other("folder image worker panicked"))??;
            }
            Ok(())
        })?;
        tracing::debug!(
            stage = "bulk.private.write",
            files = prepared.files.len(),
            elapsed_us = started.elapsed().as_micros() as u64,
            "private folder files written"
        );
        let syncing = std::time::Instant::now();
        rustix::fs::syncfs(image_file).map_err(io::Error::from)?;
        tracing::debug!(
            stage = "bulk.private.sync",
            elapsed_us = syncing.elapsed().as_micros() as u64,
            "private folder files synced"
        );
        Ok(Some(prepared))
    }
}

/// Removes only abandoned import images. Active images remain locked, and
/// unexpected names or symlinks in the metadata root are left alone.
pub fn clean_prepared_directories(root: &Path) -> io::Result<usize> {
    match fs::symlink_metadata(root) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {}
        Ok(_) => return Ok(0),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let own = name.starts_with(TEMP_PREFIX)
            && name.rsplit_once("-import-").is_some_and(|(_, tag)| {
                tag.len() == 64 && tag.bytes().all(|b| b.is_ascii_hexdigit())
            });
        if !own || !entry.file_type()?.is_dir() {
            continue;
        }
        match DataLock::acquire_existing(&entry.path()) {
            Ok(_lock) => {
                fs::remove_dir_all(entry.path())?;
                removed += 1;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::NotFound
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(removed)
}
