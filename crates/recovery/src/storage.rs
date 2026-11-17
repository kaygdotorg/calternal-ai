use crate::{Result, Step, err, invalid};
use calternal_fsio as fsio;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Pinned Linux directory access and observed fsio durability primitives.
pub(crate) struct Storage {
    root: File,
    observe: Arc<dyn Fn(Step) -> Result<()> + Send + Sync>,
}
impl Storage {
    /// Pins and validates the private root and its fixed subdirectories; returns whether it was empty.
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
        let store = Self { root, observe };
        let empty = std::fs::read_dir(store.path())
            .map_err(err)?
            .next()
            .is_none();
        if empty {
            for sub in ["accounts", "spaces", "index", "transactions"] {
                fsio::create_dir_all_durable(&store.path().join(sub)).map_err(err)?;
            }
        }
        for sub in ["accounts", "spaces", "index", "transactions"] {
            store.folder(sub)?;
        }
        Ok((store, empty))
    }
    fn fd_path(file: &File) -> PathBuf {
        use std::os::fd::AsRawFd;
        PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }
    fn path(&self) -> PathBuf {
        Self::fd_path(&self.root)
    }
    fn folder(&self, sub: &str) -> Result<File> {
        use rustix::fs::{Mode, OFlags, openat};
        if sub == "." {
            return self.root.try_clone().map_err(err);
        }
        if !["accounts", "spaces", "index", "transactions"].contains(&sub) {
            return Err(invalid());
        }
        openat(
            &self.root,
            sub,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(err)
    }
    fn name(name: &str) -> Result<()> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            Err(invalid())
        } else {
            Ok(())
        }
    }
    /// Reports an injectable coordinator boundary or completed durability operation.
    pub(crate) fn tick(&self, step: Step) -> Result<()> {
        (self.observe)(step)
    }
    /// Atomically places mode-0600 bytes relative to a pinned directory and syncs placement.
    pub(crate) fn write(&self, sub: &str, name: &str, bytes: &[u8]) -> Result<()> {
        Self::name(name)?;
        let folder = self.folder(sub)?;
        self.tick(Step::BeforeDirectorySync)?;
        fsio::write_atomic_mode(&Self::fd_path(&folder).join(name), bytes, 0o600).map_err(err)?;
        self.tick(Step::FileSynced)?;
        self.tick(Step::DirectorySynced)?;
        self.tick(Step::AfterDirectorySync)
    }
    /// Reads only a regular mode-0600 file through no-follow descriptors.
    pub(crate) fn read(&self, sub: &str, name: &str) -> Result<Vec<u8>> {
        use rustix::fs::{Mode, OFlags, openat};
        use std::os::unix::fs::PermissionsExt;
        Self::name(name)?;
        let folder = self.folder(sub)?;
        let mut file = File::from(
            openat(
                &folder,
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(err)?,
        );
        let m = file.metadata().map_err(err)?;
        if !m.is_file() || m.permissions().mode() & 0o777 != 0o600 {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(err)?;
        Ok(bytes)
    }
    /// Lists a pinned directory in deterministic order without following entries.
    pub(crate) fn names(&self, sub: &str) -> Result<Vec<String>> {
        let mut names = std::fs::read_dir(Self::fd_path(&self.folder(sub)?))
            .map_err(err)?
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(err)?;
        names.sort();
        Ok(names)
    }
    /// Idempotently unlinks a retired file; the caller must sync the affected directory.
    pub(crate) fn remove(&self, sub: &str, name: &str) -> Result<()> {
        Self::name(name)?;
        self.tick(Step::BeforeCompact)?;
        match std::fs::remove_file(Self::fd_path(&self.folder(sub)?).join(name)) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(err(e)),
        }
        self.tick(Step::AfterCompact)
    }
    /// Durably flushes a pinned directory after reclamation.
    pub(crate) fn sync(&self, sub: &str) -> Result<()> {
        fsio::sync_dir(&Self::fd_path(&self.folder(sub)?)).map_err(err)?;
        self.tick(Step::DirectorySynced)
    }
}
