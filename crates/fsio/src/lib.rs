//! # Safe file writes
//!
//! Calternal writes note content through [`write_unless_changed`]: flush
//! an adjacent temporary image, then place it exclusively or exchange it
//! with the checked existing file. [`rename_safely`] moves notes without
//! replacing a destination. Unsupported atomic operations fail safely.
//! [`write_atomic`] replaces application-owned metadata and cache files.
//! A crash leaves a whole image rather than an empty or half-written file.
//!
//! These are host filesystem mechanisms, not tenant authorization or a sandbox.
//! Callers select authorized roots and retain their guards for all I/O.
//!
//! Also here:
//! - [`write_unless_changed`]: the same, but it never replaces a version
//!   another app wrote since the caller last looked (note saves use it,
//!   in two halves: [`Prepared`]).
//! - [`DataLock`]: only one Calternal process may own a data folder.
//! - [`names`]: file names are normalised (NFC) and compared ignoring case
//!   and normalisation, so `Note.md` and `note.md` never become two notes.
//! - [`write_atomic_from`]: [`write_atomic`] for a stream (an upload). Big
//!   writes go to the disk in steps, so they never make a note save wait
//!   behind them ([`paced`]).
//! - [`beneath`]: opening and writing files below a folder (a space) by a
//!   path a person gave, without any way to reach outside it.

#![deny(missing_docs, rustdoc::broken_intra_doc_links)]
#![allow(clippy::disallowed_methods)] // This crate is the one allowed place.

pub mod paced;
pub mod private;
pub mod usage;

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub mod beneath;

mod bulk;
mod directory;
pub mod names;
mod swap;
#[cfg(target_os = "linux")]
use bulk::BulkStage;
pub use bulk::{BatchWrite, write_new_many, write_new_many_observed, write_new_many_outcome};
pub use directory::{
    DirectoryPublication, PreparedDirectory, clean_prepared_directories, prepare_new_directory,
};

#[cfg(feature = "test-hooks")]
pub use swap::set_exchange_hooks;
pub use swap::{Prepared, recover_exact, write_unless_changed};

use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Prefix of temporary files. Watchers ignore names starting with it.
pub const TEMP_PREFIX: &str = ".calternal-tmp-";

/// Writes `bytes` to `path` so that a crash leaves either the old content
/// or the new content, never anything in between.
///
/// Steps: create a temporary file in the same folder, write, `fsync` it,
/// rename it over `path`, then `fsync` the folder so the rename itself
/// survives a power cut.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_with(path, |f| {
        paced::write_all(f, bytes)?;
        Ok(bytes.len() as u64)
    })
    .map(|_| ())
}

/// Writes a private app-owned file atomically, applying `mode` to the
/// temporary file before it becomes visible. This keeps keys private across
/// replacement instead of briefly publishing a default-permission file.
#[cfg(unix)]
pub fn write_atomic_mode(path: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;

    write_with_options(
        path,
        |options| {
            options.mode(mode);
        },
        |f| {
            paced::write_all(f, bytes)?;
            Ok(bytes.len() as u64)
        },
    )
    .map(|_| ())
}

/// [`write_atomic`] for content that arrives as a stream, such as an
/// upload. Returns how many bytes were written.
pub fn write_atomic_from(path: &Path, src: &mut impl Read) -> io::Result<u64> {
    write_with(path, |f| paced::copy(f, src))
}

/// The atomic recipe around `fill`, which writes the temporary file.
fn write_with(path: &Path, fill: impl FnOnce(&mut File) -> io::Result<u64>) -> io::Result<u64> {
    write_with_options(path, |_| {}, fill)
}

/// Writes through a same-folder temporary file after configuring its create
/// options. The callback matters for private metadata whose permissions must
/// be correct before the atomic rename publishes it.
fn write_with_options(
    path: &Path,
    configure: impl FnOnce(&mut OpenOptions),
    fill: impl FnOnce(&mut File) -> io::Result<u64>,
) -> io::Result<u64> {
    let dir = parent(path)?;
    let tmp = temp_path(path)?;
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        configure(&mut options);
        let mut f = options.open(&tmp)?;
        let n = fill(&mut f)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)?;
        sync_dir(dir)?;
        usage::changed(path);
        Ok(n)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Renames `from` to `to` safely, including case-only renames
/// (`note.md` to `Note.md`) on file systems that ignore case: those go
/// through a temporary name first. Refuses to overwrite an existing,
/// different file.
pub fn rename_safely(from: &Path, to: &Path) -> io::Result<()> {
    rename_with_hook(from, to, || {})
}

fn rename_with_hook(from: &Path, to: &Path, before_place: impl FnOnce()) -> io::Result<()> {
    rename_with_hooks(from, to, before_place, || {})
}

fn rename_with_hooks(
    from: &Path,
    to: &Path,
    before_place: impl FnOnce(),
    before_case_place: impl FnOnce(),
) -> io::Result<()> {
    let same_name = names::fold(&file_name(from)?) == names::fold(&file_name(to)?)
        && parent(from)? == parent(to)?;
    before_place();
    match move_exclusive(from, to) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && same_name => {
            // A case-insensitive volume may resolve both names to one file.
            // Stage it without replacing anything, and use exclusive placement
            // again: an outside writer can occupy either name at any point.
            if !swap::same_file(
                &std::fs::symlink_metadata(from)?,
                &std::fs::symlink_metadata(to)?,
            ) {
                return Err(e);
            }
            let tmp = temp_path(to)?;
            move_exclusive(from, &tmp)?;
            before_case_place();
            if let Err(e) = move_exclusive(&tmp, to) {
                if let Err(back) = move_exclusive(&tmp, from) {
                    let flush_error = sync_dir(parent(from)?)
                        .err()
                        .map(|error| format!("; folder flush also failed: {error}"))
                        .unwrap_or_default();
                    // The source name is no longer ours. Callers must surface
                    // this path instead of retrying a destination collision.
                    return Err(io::Error::other(format!(
                        "{e}; original preserved at {}; could not restore its name: {back}{flush_error}",
                        tmp.display()
                    )));
                }
                sync_dir(parent(from)?)?;
                return Err(e);
            }
        }
        Err(e) => return Err(e),
    }
    usage::moved(from, to);
    let (to_dir, from_dir) = (parent(to)?, parent(from)?);
    if from_dir == to_dir {
        return sync_dir(to_dir);
    }
    // Both folders changed: flush them at the same time (the file system
    // can commit both in one go) rather than one after the other.
    std::thread::scope(|s| {
        let other = s.spawn(|| sync_dir(from_dir));
        let here = sync_dir(to_dir);
        let there = other
            .join()
            .unwrap_or_else(|_| Err(io::Error::other("folder flush panicked")));
        here.and(there)
    })
}

/// Moves an entry without replacing any destination, including a dangling
/// symlink. Unsupported filesystems fail safely; a link/unlink move could
/// unlink an outside writer's replacement of the source between those steps.
fn move_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    {
        use rustix::fs::{CWD, RenameFlags, renameat_with};
        renameat_with(CWD, from, CWD, to, RenameFlags::NOREPLACE).map_err(io::Error::from)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
    {
        let _ = (from, to);
        Err(io::ErrorKind::Unsupported.into())
    }
}

/// Replaces many app-owned files at once (copies a plugin keeps up to
/// date), each whole: written to a temporary name, renamed over the old
/// one, then all made durable together, with one `syncfs` on Linux, or
/// each file and its folder flushed elsewhere. A crash before the flush
/// leaves each file old or new; callers that must know which keep their
/// own record and write again (see the contacts card copies).
pub fn replace_many(files: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for (path, bytes) in files {
        let dir = parent(path)?;
        std::fs::create_dir_all(dir)?;
        let tmp = temp_path(path)?;
        let written = (|| {
            let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
            paced::write_all(&mut f, bytes)?;
            #[cfg(not(target_os = "linux"))]
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, path)
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        written?;
        usage::changed(path);
        if !dirs.iter().any(|d| d == dir) {
            dirs.push(dir.to_owned());
        }
    }
    #[cfg(target_os = "linux")]
    if let Some(dir) = dirs.first() {
        rustix::fs::syncfs(File::open(dir)?).map_err(io::Error::from)?;
    }
    #[cfg(not(target_os = "linux"))]
    for dir in &dirs {
        sync_dir(dir)?;
    }
    Ok(())
}

/// Flushes a folder's entries (new names, renames) to disk.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Creates a folder tree and flushes each new name in its parent folder.
/// Existing ancestors are left alone; each missing folder is created only
/// after its parent is available and synced before the next child is made.
pub fn create_dir_all_durable(path: &Path) -> io::Result<()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    create_dir_all_durable_with(&absolute, &mut sync_dir)
}

fn create_dir_all_durable_with(
    path: &Path,
    sync: &mut impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<()> {
    if directory_exists(path)? {
        return Ok(());
    }

    let parent = parent(path)?;
    create_dir_all_durable_with(parent, sync)?;
    match std::fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            if !directory_exists(path)? {
                return Err(error);
            }
        }
        Err(error) => return Err(error),
    }
    sync(parent)
}

fn directory_exists(path: &Path) -> io::Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(io::ErrorKind::AlreadyExists.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Exclusive ownership of a data folder. Two servers writing the same
/// notes would corrupt them, so the second one refuses to start. The lock
/// is released when the process exits, even after a crash.
pub struct DataLock {
    _file: File,
    path: PathBuf,
}

impl DataLock {
    /// Takes the lock on `<dir>/.lock`, or fails right away if another
    /// process holds it.
    pub fn acquire(dir: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Self::acquire_existing(dir)
    }

    /// Locks an existing root without creating a missing mount directory.
    pub fn acquire_existing(dir: &Path) -> io::Result<Self> {
        let path = dir.join(".lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        Self::from_file(file, path)
    }

    /// Locks `.lock` relative to an already pinned directory descriptor, without
    /// following a substituted lock symlink. Rejects nonregular lock entries.
    /// `display_root` is diagnostic only; no I/O resolves through that path.
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    pub fn acquire_at(directory: &impl std::os::fd::AsFd, display_root: &Path) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, openat};
        let file = File::from(openat(
            directory,
            ".lock",
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?);
        if !file.metadata()?.is_file() {
            return Err(io::Error::other("root lock is not a regular file"));
        }
        Self::from_file(file, display_root.join(".lock"))
    }

    fn from_file(file: File, path: PathBuf) -> io::Result<Self> {
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(std::fs::TryLockError::WouldBlock) => Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "another Calternal server is using {} (lock {})",
                    path.parent().unwrap_or(Path::new(".")).display(),
                    path.display()
                ),
            )),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    /// The lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for DataLock {
    fn drop(&mut self) {
        // Closing only this descriptor leaves an inherited or duplicated
        // open file description locked. The guard owns the lock's lifetime,
        // so release it explicitly before the descriptor is closed.
        let _ = self._file.unlock();
    }
}

fn parent(path: &Path) -> io::Result<&Path> {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => Ok(p),
        Some(_) => Ok(Path::new(".")),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} has no folder", path.display()),
        )),
    }
}

fn file_name(path: &Path) -> io::Result<String> {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} has no file name", path.display()),
            )
        })
}

fn temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = file_name(path)?;
    let unique = format!(
        "{:x}{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    Ok(parent(path)?.join(format!("{TEMP_PREFIX}{unique}-{name}")))
}

#[cfg(test)]
mod tests;

/// Removes a file and updates registered usage counters.
pub fn remove_file(path: impl AsRef<Path>) -> io::Result<()> {
    let path = path.as_ref();
    std::fs::remove_file(path)?;
    usage::changed(path);
    Ok(())
}

/// Removes only the captured file image. Move it out of the public name
/// before checking bytes, so an outside replacement is never unlinked.
/// The fixed private name lets journal recovery finish an interrupted removal.
pub fn remove_unless_changed(path: &Path, expected: impl Fn(&[u8]) -> bool) -> io::Result<bool> {
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("missing file name"))?;
    let held = path.with_file_name(format!("{TEMP_PREFIX}delete-{}", name.to_string_lossy()));
    if !held.exists() {
        match move_exclusive(path, &held) {
            Ok(()) => sync_dir(parent(path)?)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(e) => return Err(e),
        }
    }
    let bytes = std::fs::read(&held)?;
    if !expected(&bytes) {
        // Never replace an outside file created while the original was held.
        move_exclusive(&held, path).map_err(|e| {
            io::Error::other(format!("outside edit preserved at {}: {e}", held.display()))
        })?;
        sync_dir(parent(path)?)?;
        return Ok(false);
    }
    std::fs::remove_file(&held)?;
    sync_dir(parent(path)?)?;
    usage::changed(path);
    Ok(!path.exists())
}
