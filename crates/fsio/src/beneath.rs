//! Files inside one folder, and never outside it.
//!
//! A path that comes from a person (an attachment's address, an upload's
//! name) must never reach a file outside their space: no `..`, no absolute
//! path, no symlink that points out. A [`Folder`] is an open folder, and
//! every path below it is resolved by the kernel *inside* it
//! (`openat2` with `RESOLVE_BENEATH` on Linux; one component at a time
//! without following symlinks elsewhere). Checking the path as text first
//! is only a second fence: even a path that passes it can't climb out.
//!
//! Writing works on the open folder too, so nothing can be swapped in
//! between a check and a write: [`Folder::create_temp`] makes a hidden
//! temporary file, and [`TempFile::persist`] flushes it, gives it its real
//! name without ever replacing an existing file, and flushes the folder,
//! the same promise as [`crate::write_atomic`]. A temporary file that is
//! dropped before that is removed.

use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use std::fs::File;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::path::Path;

/// An open folder. Paths below it never resolve outside it.
#[derive(Debug)]
pub struct Folder {
    fd: OwnedFd,
    path: std::path::PathBuf,
}

impl Folder {
    /// Opens (and creates, if missing) a folder the server itself chose,
    /// like a space. Only call this with trusted paths.
    pub fn open(path: &Path) -> io::Result<Self> {
        std::fs::create_dir_all(path)?;
        let fd = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        Ok(Self {
            fd,
            path: path.to_owned(),
        })
    }

    /// Opens the folder at `rel` below this one. With `create`, missing
    /// folders are made (and their parents flushed, so they survive a
    /// power cut). Fails with `InvalidInput` for a path that isn't plain
    /// (`..`, absolute, empty parts) and with an error for anything that
    /// would leave this folder.
    pub fn open_sub(&self, rel: &str, create: bool) -> io::Result<Folder> {
        let parts = plain_parts(rel)?;
        match beneath(&self.fd, rel, OFlags::RDONLY | OFlags::DIRECTORY) {
            Err(e) if create && e.kind() == io::ErrorKind::NotFound => {}
            other => {
                return other.map(|fd| Folder {
                    fd,
                    path: self.path.join(rel),
                });
            }
        }
        let mut cur = self.fd.try_clone()?;
        for part in parts {
            let next = match beneath(&cur, part, OFlags::RDONLY | OFlags::DIRECTORY) {
                Ok(fd) => fd,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    match rustix::fs::mkdirat(&cur, part, Mode::from_raw_mode(0o755)) {
                        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                        Err(e) => return Err(e.into()),
                    }
                    rustix::fs::fsync(&cur)?;
                    beneath(&cur, part, OFlags::RDONLY | OFlags::DIRECTORY)?
                }
                Err(e) => return Err(e),
            };
            cur = next;
        }
        Ok(Folder {
            fd: cur,
            path: self.path.join(rel),
        })
    }

    /// Opens the regular file at `rel` below this one for reading. A
    /// symlink, folder, pipe or device is refused as `NotFound`, so it
    /// looks exactly like a file that isn't there.
    pub fn open_file(&self, rel: &str) -> io::Result<File> {
        plain_parts(rel)?;
        // NONBLOCK: opening a pipe someone planted must not hang. It has
        // no effect on reading a regular file.
        let fd = beneath(
            &self.fd,
            rel,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        )?;
        let st = rustix::fs::fstat(&fd)?;
        if rustix::fs::FileType::from_raw_mode(st.st_mode) != rustix::fs::FileType::RegularFile {
            return Err(io::ErrorKind::NotFound.into());
        }
        Ok(File::from(fd))
    }

    /// The names in this folder (not `.` and `..`).
    pub fn names(&self) -> io::Result<Vec<String>> {
        let mut dir = rustix::fs::Dir::read_from(&self.fd)?;
        let mut out = Vec::new();
        while let Some(entry) = dir.read() {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy();
            if name != "." && name != ".." {
                out.push(name.into_owned());
            }
        }
        Ok(out)
    }

    /// Removes the file `name` (one name, no folders) from this folder.
    pub fn remove(&self, name: &str) -> io::Result<()> {
        single(name)?;
        rustix::fs::unlinkat(&self.fd, name, AtFlags::empty())?;
        crate::usage::changed(&self.path.join(name));
        Ok(())
    }

    /// Flushes this folder's entries (new names) to disk.
    pub fn sync(&self) -> io::Result<()> {
        rustix::fs::fsync(&self.fd).map_err(Into::into)
    }

    /// Makes a new, empty temporary file named `name` in this folder. It
    /// must not exist yet. Name it with [`crate::TEMP_PREFIX`] so watchers
    /// and listings skip it.
    pub fn create_temp(&self, name: &str) -> io::Result<TempFile> {
        single(name)?;
        let fd = rustix::fs::openat(
            &self.fd,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o644),
        )?;
        Ok(TempFile {
            dir: self.fd.try_clone()?,
            name: name.to_owned(),
            file: File::from(fd),
            placed: false,
            path: self.path.clone(),
        })
    }
}

/// A temporary file being filled. Dropping it removes it; [`Self::persist`]
/// gives it its real name.
#[derive(Debug)]
pub struct TempFile {
    dir: OwnedFd,
    name: String,
    file: File,
    placed: bool,
    path: std::path::PathBuf,
}

impl TempFile {
    /// The file, to write into.
    pub fn file(&mut self) -> &mut File {
        &mut self.file
    }

    /// Its temporary name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Flushes the file, renames it to the first name `pick` offers that
    /// isn't taken (`pick(1)`, `pick(2)`, …; `None` gives up), then flushes
    /// the folder. Never replaces an existing file. Returns the name used.
    pub fn persist(mut self, mut pick: impl FnMut(u32) -> Option<String>) -> io::Result<String> {
        self.file.sync_all()?;
        for n in 1.. {
            let Some(to) = pick(n) else { break };
            single(&to)?;
            match place(&self.dir, &self.name, &to) {
                Ok(()) => {
                    self.placed = true;
                    rustix::fs::fsync(&self.dir)?;
                    crate::usage::changed(&self.path.join(&to));
                    return Ok(to);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "every name offered is taken",
        ))
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.placed {
            let _ = rustix::fs::unlinkat(&self.dir, self.name.as_str(), AtFlags::empty());
        }
    }
}

/// Renames `from` to `to` in `dir`, refusing to replace `to`.
fn place(dir: &OwnedFd, from: &str, to: &str) -> io::Result<()> {
    match rustix::fs::renameat_with(dir, from, dir, to, RenameFlags::NOREPLACE) {
        Ok(()) => Ok(()),
        // File systems without "rename unless it exists": link, which
        // never replaces either, then drop the temporary name.
        Err(rustix::io::Errno::INVAL | rustix::io::Errno::NOSYS) => {
            rustix::fs::linkat(dir, from, dir, to, AtFlags::empty())?;
            rustix::fs::unlinkat(dir, from, AtFlags::empty())?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Opens `rel` below `dir`, never resolving outside it.
fn beneath(dir: &impl AsFd, rel: &str, flags: OFlags) -> io::Result<OwnedFd> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        use rustix::fs::ResolveFlags;
        // The kernel may ask for a retry if a rename raced the lookup.
        for _ in 0..16 {
            match rustix::fs::openat2(
                dir,
                rel,
                flags | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
            ) {
                Err(rustix::io::Errno::AGAIN) => continue,
                // Kernels older than 5.6: walk it ourselves, below.
                Err(rustix::io::Errno::NOSYS) => break,
                Err(rustix::io::Errno::XDEV) => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "path leaves its folder",
                    ));
                }
                other => return other.map_err(Into::into),
            }
        }
    }
    walk(dir, rel, flags)
}

/// [`beneath`] without `openat2`: one component at a time, refusing every
/// symlink.
fn walk(dir: &impl AsFd, rel: &str, flags: OFlags) -> io::Result<OwnedFd> {
    let parts = plain_parts(rel)?;
    let (last, folders) = parts.split_last().expect("plain_parts is never empty");
    let mut cur: Option<OwnedFd> = None;
    for part in folders {
        let at = cur.as_ref().map_or(dir.as_fd(), |f| f.as_fd());
        cur = Some(rustix::fs::openat(
            at,
            *part,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
    }
    let at = cur.as_ref().map_or(dir.as_fd(), |f| f.as_fd());
    Ok(rustix::fs::openat(
        at,
        *last,
        flags | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?)
}

/// The parts of a plain relative path: no leading `/`, no empty parts, no
/// `.` or `..`, no NUL or backslash.
pub fn plain_parts(rel: &str) -> io::Result<Vec<&str>> {
    let bad = || io::Error::new(io::ErrorKind::InvalidInput, "not a plain relative path");
    if rel.is_empty() || rel.starts_with('/') || rel.contains(['\0', '\\']) {
        return Err(bad());
    }
    let parts: Vec<&str> = rel.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err(bad());
    }
    Ok(parts)
}

/// Checks `name` is one plain name, not a path.
fn single(name: &str) -> io::Result<()> {
    match plain_parts(name)?.len() {
        1 => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a name, not a path",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// Separates concurrent fixtures even when the system clock reports the same timestamp.
    fn tmp() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "calternal-beneath-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn paths_that_climb_out_are_refused() {
        let root = tmp();
        let space = root.join("space");
        std::fs::create_dir_all(space.join("Photos")).unwrap();
        crate::write_atomic(&root.join("secret.txt"), b"secret").unwrap();
        crate::write_atomic(&space.join("Photos/ok.txt"), b"ok").unwrap();
        // Symlinks out of the space, as a file and as a folder.
        std::os::unix::fs::symlink(root.join("secret.txt"), space.join("Photos/link.txt")).unwrap();
        std::os::unix::fs::symlink(&root, space.join("Documents")).unwrap();
        std::os::unix::fs::symlink("../../secret.txt", space.join("Photos/rel.txt")).unwrap();
        let f = Folder::open(&space).unwrap();

        let mut s = String::new();
        f.open_file("Photos/ok.txt")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        assert_eq!(s, "ok");
        for bad in [
            "../secret.txt",
            "Photos/../../secret.txt",
            "/etc/passwd",
            "Photos/link.txt",
            "Photos/rel.txt",
            "Documents/secret.txt",
            "Photos//ok.txt",
            "./Photos/ok.txt",
            "Photos\\ok.txt",
            "Photos/ok.txt\0",
            "",
            "Photos",
        ] {
            assert!(f.open_file(bad).is_err(), "{bad:?} should be refused");
        }
        // Making folders can't escape either: `Documents` points out.
        assert!(f.open_sub("Documents/2026", true).is_err());
        assert!(!root.join("2026").exists());
        assert!(f.open_sub("../out", true).is_err());
        assert!(!root.join("out").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn walking_without_openat2_refuses_symlinks_too() {
        let root = tmp();
        let space = root.join("space");
        std::fs::create_dir_all(space.join("a")).unwrap();
        crate::write_atomic(&root.join("secret.txt"), b"secret").unwrap();
        std::os::unix::fs::symlink(&root, space.join("a/up")).unwrap();
        let f = Folder::open(&space).unwrap();
        assert!(walk(&f.fd, "a/up/secret.txt", OFlags::RDONLY).is_err());
        assert!(walk(&f.fd, "../secret.txt", OFlags::RDONLY).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn temp_files_persist_without_replacing_and_vanish_when_dropped() {
        let root = tmp();
        let f = Folder::open(&root).unwrap();
        let day = f.open_sub("Photos/2026/2026-10-17", true).unwrap();
        crate::write_atomic(&root.join("Photos/2026/2026-10-17/a.jpg"), b"first").unwrap();

        let mut t = day.create_temp(".calternal-tmp-1").unwrap();
        t.file().write_all(b"second").unwrap();
        let name = t
            .persist(|n| {
                Some(if n == 1 {
                    "a.jpg".into()
                } else {
                    format!("a {n}.jpg")
                })
            })
            .unwrap();
        assert_eq!(name, "a 2.jpg");
        assert_eq!(
            std::fs::read(root.join("Photos/2026/2026-10-17/a.jpg")).unwrap(),
            b"first"
        );
        assert_eq!(
            std::fs::read(root.join("Photos/2026/2026-10-17/a 2.jpg")).unwrap(),
            b"second"
        );

        let mut t = day.create_temp(".calternal-tmp-2").unwrap();
        t.file().write_all(b"never").unwrap();
        drop(t);
        let mut names = day.names().unwrap();
        names.sort();
        assert_eq!(names, vec!["a 2.jpg", "a.jpg"]);
        assert!(day.create_temp("../x").is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
