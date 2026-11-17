use crate::{Result, Step, err, invalid};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

/// Descriptor-pinned private files; only creation, replacement and removal sync directories.
pub(crate) struct Storage {
    root: File,
    observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
}
impl Storage {
    /// Opens an existing mode-0700 root without following its final component.
    pub(crate) fn open(
        path: &Path,
        observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
    ) -> Result<(Self, bool)> {
        use rustix::fs::{Mode, OFlags, open};
        use std::os::unix::fs::PermissionsExt;
        let root = File::from(
            open(
                path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(err)?,
        );
        if root.metadata().map_err(err)?.permissions().mode() & 0o777 != 0o700 {
            return Err(invalid());
        }
        let s = Self { root, observe };
        let empty = s.names()?.is_empty();
        Ok((s, empty))
    }
    fn path(&self) -> PathBuf {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", self.root.as_raw_fd()))
    }
    fn file(&self, name: &str, flags: rustix::fs::OFlags) -> Result<File> {
        use rustix::fs::{Mode, OFlags, openat};
        use std::os::unix::fs::PermissionsExt;
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(invalid());
        }
        let file = File::from(
            openat(
                &self.root,
                name,
                flags | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(err)?,
        );
        let m = file.metadata().map_err(err)?;
        if !m.is_file() || m.permissions().mode() & 0o777 != 0o600 {
            return Err(invalid());
        }
        Ok(file)
    }
    /// Reports a fault boundary; sync events count completed system calls.
    pub(crate) fn tick(&self, step: Step) -> Result<()> {
        (self.observe)(step)
    }
    /// Reads a no-follow regular mode-0600 file.
    pub(crate) fn read(&self, name: &str) -> Result<Vec<u8>> {
        let mut b = Vec::new();
        self.file(name, rustix::fs::OFlags::RDONLY)?
            .read_to_end(&mut b)
            .map_err(err)?;
        Ok(b)
    }
    /// Creates and syncs a fresh empty segment before publishing its snapshot pointer.
    pub(crate) fn create(&self, name: &str) -> Result<()> {
        use rustix::fs::OFlags;
        self.file(name, OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL)?
            .sync_all()
            .map_err(err)?;
        self.tick(Step::FileSynced)?;
        self.sync()
    }
    /// Appends one complete frame and flushes it before returning.
    pub(crate) fn append(&self, name: &str, bytes: &[u8]) -> Result<()> {
        let mut f = self.file(
            name,
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::APPEND,
        )?;
        self.tick(Step::BeforeAppend)?;
        // Separate boundary exercises a torn length prefix with real partial bytes.
        let split = bytes.len().min(8);
        f.write_all(&bytes[..split]).map_err(err)?;
        self.tick(Step::AfterPrefix)?;
        f.write_all(&bytes[split..]).map_err(err)?;
        self.tick(Step::AfterAppend)?;
        f.sync_all().map_err(err)?;
        self.tick(Step::FileSynced)
    }
    /// Reestablishes durability after an uncertain prior append or cold restart.
    pub(crate) fn flush(&self, name: &str) -> Result<()> {
        self.file(name, rustix::fs::OFlags::WRONLY)?
            .sync_all()
            .map_err(err)?;
        self.tick(Step::FileSynced)
    }
    /// Trims only a database-proven uncommitted torn proposal tail, then syncs it.
    pub(crate) fn trim(&self, name: &str, length: u64) -> Result<()> {
        self.tick(Step::BeforeTailRepair)?;
        let f = self.file(name, rustix::fs::OFlags::WRONLY)?;
        f.set_len(length).map_err(err)?;
        f.sync_all().map_err(err)?;
        self.tick(Step::FileSynced)?;
        self.tick(Step::AfterTailRepair)
    }
    /// Atomically replaces the checksummed snapshot using the ported fsio primitive.
    pub(crate) fn snapshot(&self, bytes: &[u8]) -> Result<()> {
        self.tick(Step::BeforeSnapshot)?;
        calternal_fsio::write_atomic_mode(&self.path().join("snapshot.json"), bytes, 0o600)
            .map_err(err)?;
        self.tick(Step::FileSynced)?;
        self.tick(Step::DirectorySynced)?;
        self.tick(Step::AfterSnapshot)
    }
    /// Lists names for bounded segment/staging-file cleanup under the root lock.
    pub(crate) fn names(&self) -> Result<Vec<String>> {
        std::fs::read_dir(self.path())
            .map_err(err)?
            .map(|e| {
                e.map(|e| e.file_name().to_string_lossy().into_owned())
                    .map_err(err)
            })
            .collect()
    }
    /// Unlinks an obsolete segment only after the new snapshot is durable.
    pub(crate) fn remove(&self, name: &str) -> Result<()> {
        self.tick(Step::BeforeCompact)?;
        std::fs::remove_file(self.path().join(name)).map_err(err)?;
        self.tick(Step::AfterCompact)
    }
    /// Flushes directory metadata after segment creation or retirement.
    pub(crate) fn sync(&self) -> Result<()> {
        self.tick(Step::BeforeCompactSync)?;
        self.root.sync_all().map_err(err)?;
        self.tick(Step::DirectorySynced)?;
        self.tick(Step::AfterCompactSync)
    }
    /// Returns inode/size/time evidence for safely reusing a private-file read cache.
    pub(crate) fn stamp(&self, name: &str) -> Result<Vec<u64>> {
        use std::os::unix::fs::MetadataExt;
        let m = self
            .file(name, rustix::fs::OFlags::RDONLY)?
            .metadata()
            .map_err(err)?;
        Ok(vec![
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime() as u64,
            m.mtime_nsec() as u64,
            m.ctime() as u64,
            m.ctime_nsec() as u64,
        ])
    }
}
