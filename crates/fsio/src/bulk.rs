//! Writing many new files at once (an import, a contacts sync): the bulk path of [`crate::write_new_many`].
//!
//! Each file goes to a temporary name (on Linux, optionally an unnamed file staged outside the watched
//! tree, see `BulkStage`), is moved into place without replacing anything, and then every folder's file
//! system is made durable together (`syncfs` on Linux, a flush per folder elsewhere). Split out of
//! `lib.rs` for the ADR 0070 size ceiling (#1153); the code is unchanged.

#[cfg(target_os = "linux")]
use crate::file_name;
#[cfg(not(target_os = "linux"))]
use crate::sync_dir;
use crate::{move_exclusive, parent, temp_path, usage};
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

/// Writes many new files at once (an import), each whole and none over
/// an existing file: every file goes to a temporary name, then into place
/// without replacing anything, and then all of them are made durable
/// together. On Linux that is one `syncfs` of their file system instead
/// of two flushes per file (on a spinning disk, minutes against seconds
/// for thousands of notes); elsewhere each file and its folder is flushed.
///
/// Returns, per file in order, whether it was placed; a file that would
/// have replaced another is left out (`AlreadyExists`), its bytes never
/// written over anything. Everything placed is durable when this returns.
pub fn write_new_many(files: &[(PathBuf, Vec<u8>)]) -> io::Result<Vec<io::Result<()>>> {
    write_new_many_observed(files, None, |_| {})
}

/// As [`write_new_many`], reporting placements before the final directory
/// sync and reporting `files.len() + 1` after that sync. The observer lets
/// recovery tests stop at exact durable boundaries without using timers.
/// On Linux, a staging folder outside the watched tree can avoid temporary
/// write events: only the final, whole file appears at the destination.
/// Different filesystems or inheritance policies use a destination-local
/// temporary file instead.
pub fn write_new_many_observed(
    files: &[(PathBuf, Vec<u8>)],
    staging: Option<&Path>,
    observe: impl FnMut(usize),
) -> io::Result<Vec<io::Result<()>>> {
    let batch = write_new_many_outcome(files, staging, observe)?;
    batch.synced?;
    Ok(batch.placed)
}

/// Per-file ownership survives a failed final flush, so the notes job can
/// record and undo every file it placed before returning that failure.
pub struct BatchWrite {
    /// Which destinations this batch placed, in input order.
    pub placed: Vec<io::Result<()>>,
    /// Whether all placed files are durable.
    pub synced: io::Result<()>,
}

/// As [`write_new_many_observed`], retaining placements on a sync failure.
pub fn write_new_many_outcome(
    files: &[(PathBuf, Vec<u8>)],
    staging: Option<&Path>,
    mut observe: impl FnMut(usize),
) -> io::Result<BatchWrite> {
    let placing = std::time::Instant::now();
    // Repeated destinations retain the sequential first-writer behavior.
    let mut unique = std::collections::HashSet::new();
    let serial = files.len() <= 64 || !files.iter().all(|(path, _)| unique.insert(path));
    let (out, dirs) = if serial {
        place_bulk_chunk(files, staging, &mut observe)
    } else {
        std::thread::scope(|scope| {
            let (send, recv) = std::sync::mpsc::sync_channel(4);
            let chunk = files.len().div_ceil(4);
            let workers: Vec<_> = files
                .chunks(chunk)
                .map(|files| {
                    let send = send.clone();
                    scope.spawn(move || {
                        let (out, dirs) = place_bulk_chunk(files, staging, |_| {
                            let _ = send.send(());
                        });
                        (out, dirs)
                    })
                })
                .collect();
            drop(send);
            for (i, ()) in recv.into_iter().enumerate() {
                observe(i + 1);
            }
            let mut out = Vec::with_capacity(files.len());
            let mut dirs = std::collections::HashMap::new();
            // Join in input order so collisions and errors retain their slots.
            for worker in workers {
                let (placed, made) = worker
                    .join()
                    .map_err(|_| io::Error::other("bulk file worker panicked"))?;
                out.extend(placed);
                dirs.extend(made);
            }
            Ok::<_, io::Error>((out, dirs.into_iter().collect()))
        })?
    };
    tracing::debug!(
        stage = "bulk.files.place",
        files = files.len(),
        elapsed_us = placing.elapsed().as_micros() as u64,
        "bulk files placed"
    );
    let syncing = std::time::Instant::now();
    let synced = sync_bulk_dirs(&dirs);
    tracing::debug!(
        stage = "bulk.files.sync",
        directories = dirs.len(),
        elapsed_us = syncing.elapsed().as_micros() as u64,
        "bulk files synced"
    );
    usage::wrote(
        files
            .iter()
            .zip(&out)
            .filter(|(_, placed)| placed.is_ok())
            .map(|((path, bytes), _)| (path.as_path(), bytes.len() as u64)),
    );
    if synced.is_ok() {
        observe(files.len() + 1);
    }
    Ok(BatchWrite {
        placed: out,
        synced,
    })
}

fn place_bulk_chunk(
    files: &[(PathBuf, Vec<u8>)],
    staging: Option<&Path>,
    mut observe: impl FnMut(usize),
) -> (Vec<io::Result<()>>, Vec<(PathBuf, bool)>) {
    #[cfg(target_os = "linux")]
    let mut stage = staging.and_then(BulkStage::open);
    #[cfg(not(target_os = "linux"))]
    let _ = staging;
    let mut out = Vec::with_capacity(files.len());
    let mut dirs: Vec<(PathBuf, bool)> = Vec::new();
    for (path, bytes) in files {
        let placed = (|| {
            let dir = parent(path)?;
            let at = match dirs.iter().position(|(d, _)| d == dir) {
                Some(at) => at,
                None => {
                    std::fs::create_dir_all(dir)?;
                    #[cfg(target_os = "linux")]
                    let can_stage = stage.as_ref().is_some_and(|s| s.can_place_in(dir));
                    #[cfg(not(target_os = "linux"))]
                    let can_stage = false;
                    dirs.push((dir.to_owned(), can_stage));
                    dirs.len() - 1
                }
            };
            #[cfg(target_os = "linux")]
            if dirs[at].1
                && let Some(stage) = &mut stage
            {
                if stage.place(path, bytes)? {
                    return Ok(());
                }
                // A bind mount or inaccessible staging folder may still
                // require the original destination-local write path.
                dirs[at].1 = false;
            }
            #[cfg(not(target_os = "linux"))]
            let _ = at;
            write_bulk_temp(&temp_path(path)?, path, bytes)
        })();
        out.push(placed);
        observe(out.len());
    }
    (out, dirs)
}

fn sync_bulk_dirs(dirs: &[(PathBuf, bool)]) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    use std::os::unix::fs::MetadataExt;
    #[cfg(target_os = "linux")]
    {
        let mut synced = std::collections::HashSet::new();
        for (dir, _) in dirs {
            let file = File::open(dir)?;
            if synced.insert(file.metadata()?.dev()) {
                rustix::fs::syncfs(file).map_err(io::Error::from)?;
            }
        }
    }
    #[cfg(not(target_os = "linux"))]
    for (dir, _) in dirs {
        sync_dir(dir)?;
    }
    Ok(())
}

fn write_bulk_temp(tmp: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let written = (|| {
        let mut f = OpenOptions::new().write(true).create_new(true).open(tmp)?;
        std::io::Write::write_all(&mut f, bytes)?;
        #[cfg(not(target_os = "linux"))]
        f.sync_all()?;
        drop(f);
        move_exclusive(tmp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    written
}

/// Stage outside the watched note tree. Unnamed files avoid temporary
/// directory entries too; older filesystems use a named temporary here.
#[cfg(target_os = "linux")]
pub(crate) struct BulkStage {
    dir: PathBuf,
    file: File,
    device: u64,
    pub(crate) anonymous: bool,
    inheritance: Option<BulkInheritance>,
}

#[cfg(target_os = "linux")]
#[derive(PartialEq, Eq)]
struct BulkInheritance {
    group: u32,
    setgid: bool,
    attributes: std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
}

#[cfg(target_os = "linux")]
impl BulkInheritance {
    fn read(file: &File, metadata: &std::fs::Metadata) -> Option<Self> {
        use rustix::fs::{fgetxattr, flistxattr};
        use std::os::unix::fs::MetadataExt;
        let mut names = [0u8; 4096];
        let count = flistxattr(file, &mut names[..]).ok()?;
        let mut attributes = std::collections::BTreeMap::new();
        for name in names[..count].split(|b| *b == 0) {
            if !name.is_empty() {
                let mut value = [0u8; 4096];
                let size = fgetxattr(file, name, &mut value[..]).ok()?;
                attributes.insert(name.to_vec(), value[..size].to_vec());
            }
        }
        Some(Self {
            group: metadata.gid(),
            setgid: metadata.mode() & 0o2000 != 0,
            attributes,
        })
    }
}

#[cfg(target_os = "linux")]
impl BulkStage {
    pub(crate) fn open(dir: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt;
        let file = File::open(dir).ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_dir() {
            return None;
        }
        let device = metadata.dev();
        let inheritance = BulkInheritance::read(&file, &metadata);
        Some(Self {
            dir: dir.to_owned(),
            file,
            device,
            anonymous: true,
            inheritance,
        })
    }

    pub(crate) fn can_place_in(&self, dir: &Path) -> bool {
        use std::os::unix::fs::MetadataExt;
        let Some(expected) = self.inheritance.as_ref() else {
            return false;
        };
        let Ok(file) = File::open(dir) else {
            return false;
        };
        let Ok(metadata) = file.metadata() else {
            return false;
        };
        // A move must not change inherited group, ACL or security labels.
        // Conservatively compare every folder attribute; different or
        // unreadable policies use the destination.
        metadata.dev() == self.device
            && BulkInheritance::read(&file, &metadata).as_ref() == Some(expected)
    }

    /// False means this destination needs its own local temporary file.
    pub(crate) fn place(&mut self, path: &Path, bytes: &[u8]) -> io::Result<bool> {
        if self.anonymous {
            match self.place_anonymous(path, bytes) {
                Ok(Some(())) => return Ok(true),
                Ok(None) => self.anonymous = false,
                Err(e) if e.kind() == io::ErrorKind::CrossesDevices => return Ok(false),
                Err(e) => return Err(e),
            }
        }
        let tmp = temp_path(&self.dir.join(file_name(path)?))?;
        match write_bulk_temp(&tmp, path, bytes) {
            Ok(()) => Ok(true),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::CrossesDevices
                        | io::ErrorKind::PermissionDenied
                        | io::ErrorKind::NotFound
                ) =>
            {
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    fn place_anonymous(&self, path: &Path, bytes: &[u8]) -> io::Result<Option<()>> {
        use rustix::fs::{AtFlags, CWD, Mode, OFlags, linkat, openat};
        use rustix::io::Errno;
        use std::os::fd::AsRawFd;
        let fd = match openat(
            &self.file,
            ".",
            OFlags::TMPFILE | OFlags::WRONLY | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o666),
        ) {
            Ok(fd) => fd,
            Err(
                Errno::OPNOTSUPP
                | Errno::NOSYS
                | Errno::INVAL
                | Errno::ISDIR
                | Errno::ACCESS
                | Errno::PERM,
            ) => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let mut file = File::from(fd);
        std::io::Write::write_all(&mut file, bytes)?;
        // Following our own descriptor works without privileged AT_EMPTY_PATH.
        // linkat creates the final name only if it is free, after the complete
        // write; a killed process leaves no unlinked temporary file behind.
        let from = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
        match linkat(CWD, &from, CWD, path, AtFlags::SYMLINK_FOLLOW) {
            Ok(()) => Ok(Some(())),
            Err(Errno::NOENT | Errno::OPNOTSUPP | Errno::NOSYS | Errno::ACCESS | Errno::PERM) => {
                Ok(None)
            }
            Err(e) => Err(e.into()),
        }
    }
}
