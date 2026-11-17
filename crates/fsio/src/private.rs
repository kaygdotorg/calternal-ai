//! Owner-only (0700 directory, 0600 file) preparation for protected state:
//! databases, keys, tokens and private trees.
//!
//! Creation modes apply before publication, independently of the process umask.
//! New directory entries are synced with [`crate::sync_dir`]. These helpers do not
//! replace installation root validation or root locking.
//!
//! Cost: [`directory()`] stats every missing ancestor once, then one `chmod` on the
//! target; [`file()`] does one stat, one open and one `chmod`; each new entry adds one
//! parent-directory fsync. Call them when a tree or file is first prepared, not per
//! write.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Create a directory and missing parents with mode 0700, sync each new entry,
/// and enforce mode 0700 on the requested directory if it already exists.
/// Existing ancestors are never chmodded. Symlinks and non-directories at the
/// requested path or a newly traversed ancestor are refused.
pub fn directory(path: &Path) -> io::Result<()> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    let path = std::path::absolute(path)?;
    create(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
}

fn create(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => return Ok(()),
        Ok(_) => {
            return Err(io::Error::other(
                "private directory is not a real directory",
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    create(parent)?;
    match fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            crate::sync_dir(parent)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            let m = fs::symlink_metadata(path)?;
            if !m.is_dir() {
                return Err(io::Error::other(
                    "private directory is not a real directory",
                ));
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        }
        Err(e) => Err(e),
    }
}

/// Open or precreate a regular state file in an already prepared directory,
/// applying mode 0600 before writes and syncing its directory on creation.
/// Existing bytes are preserved; symlinks and non-files are refused. SQLite
/// inherits the database file's private mode for its WAL and shared-memory files.
pub fn file(path: &Path) -> io::Result<File> {
    let path = std::path::absolute(path)?;
    let existed = match fs::symlink_metadata(&path) {
        Ok(m) if m.is_file() => true,
        Ok(_) => return Err(io::Error::other("private state is not a regular file")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)?;
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    if !existed {
        crate::sync_dir(path.parent().ok_or(io::ErrorKind::InvalidInput)?)?;
    }
    Ok(file)
}

/// Publish private bytes atomically without replacing an existing destination.
/// The parent must already be a trusted private directory. A synced temporary
/// file is renamed exclusively, so a crash cannot leave a second hard link to
/// published key material. An interrupted pre-publication temporary file holds
/// unpublished bytes only. Returns `AlreadyExists` without altering the target.
/// Cost: one bounded write, one file fsync, one rename and one directory fsync.
pub fn publish(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let tmp = crate::temp_path(path)?;
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        crate::move_exclusive(&tmp, path)?;
        crate::sync_dir(dir)
    })();
    let _ = fs::remove_file(&tmp);
    result
}

#[cfg(test)]
mod tests {
    use crate::tests::tmp;

    #[test]
    fn private_publication_never_replaces_a_key_or_leaves_an_extra_link() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let dir = tmp();
        let path = dir.join("secret.key");
        super::publish(&path, &[7u8; 32]).unwrap();
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::metadata(&path).unwrap().nlink(), 1);
        assert_eq!(
            super::publish(&path, &[9u8; 32]).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&path).unwrap(), [7u8; 32]);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
