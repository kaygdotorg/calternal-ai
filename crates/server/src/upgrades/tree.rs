//! Descriptor-relative filesystem operations for full restore points.

use rustix::fs::{self, AtFlags, FileType, Mode, OFlags, Stat};
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    os::{fd::OwnedFd, unix::ffi::OsStrExt},
    path::{Component, Path},
};

const DIRECTORY: OFlags = OFlags::RDONLY.union(OFlags::DIRECTORY);
const SAFE: OFlags = OFlags::NOFOLLOW.union(OFlags::CLOEXEC);

pub(super) struct Tree(OwnedFd);

impl Tree {
    pub(super) fn open(path: &Path) -> io::Result<Self> {
        let absolute;
        let path = if path.is_absolute() {
            path
        } else {
            absolute = std::env::current_dir()?.join(path);
            &absolute
        };
        let mut components = path.components();
        if components.next() != Some(Component::RootDir) {
            return Err(io::Error::other("snapshot root must be absolute"));
        }
        let mut folder = fs::open("/", DIRECTORY | SAFE, Mode::empty())?;
        for part in components {
            let Component::Normal(name) = part else {
                return Err(io::Error::other("invalid snapshot root path"));
            };
            // The configured role itself can be on a separate mount.
            folder = fs::openat(&folder, name, DIRECTORY | SAFE, Mode::empty())?;
        }
        Ok(Self(folder))
    }

    #[cfg(target_os = "linux")]
    fn existing(&self, name: &OsStr, flags: OFlags) -> io::Result<OwnedFd> {
        use fs::ResolveFlags;
        Ok(fs::openat2(
            &self.0,
            name,
            flags | SAFE,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?)
    }

    #[cfg(not(target_os = "linux"))]
    fn existing(&self, path: &OsStr, flags: OFlags) -> io::Result<OwnedFd> {
        let mut folder = self.0.try_clone()?;
        let mut parts = Path::new(path).components().peekable();
        while let Some(part) = parts.next() {
            let Component::Normal(name) = part else {
                return Err(io::Error::other("invalid snapshot path"));
            };
            let flags = if parts.peek().is_none() {
                flags
            } else {
                DIRECTORY
            };
            folder = fs::openat(&folder, name, flags | SAFE, Mode::empty())?;
        }
        Ok(folder)
    }

    pub(super) fn directory(&self, name: &OsStr) -> io::Result<Self> {
        self.existing(name, DIRECTORY).map(Self)
    }

    pub(super) fn names(&self) -> io::Result<Vec<OsString>> {
        let mut dir = fs::Dir::read_from(&self.0)?;
        let mut names = Vec::new();
        while let Some(entry) = dir.read() {
            let entry = entry?;
            let name = entry.file_name().to_bytes();
            if name != b"." && name != b".." {
                names.push(OsStr::from_bytes(name).to_owned());
            }
        }
        names.sort_unstable();
        Ok(names)
    }

    #[cfg(target_os = "linux")]
    pub(super) fn inspect(&self, name: &OsStr) -> io::Result<Stat> {
        // Linux permits O_PATH | O_NOFOLLOW on a final symlink even with
        // NO_SYMLINKS. fstat then describes the link entry itself.
        let fd = self.existing(name, OFlags::PATH)?;
        Ok(fs::fstat(fd)?)
    }

    #[cfg(not(target_os = "linux"))]
    pub(super) fn inspect(&self, name: &OsStr) -> io::Result<Stat> {
        Ok(fs::statat(&self.0, name, AtFlags::SYMLINK_NOFOLLOW)?)
    }

    pub(super) fn file(&self, name: &OsStr) -> io::Result<File> {
        let metadata = self.inspect(name)?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(io::Error::other("expected a regular snapshot file"));
        }
        let fd = self.existing(name, OFlags::RDONLY | OFlags::NONBLOCK)?;
        let stat = fs::fstat(&fd)?;
        if (stat.st_dev, stat.st_ino) != (metadata.st_dev, metadata.st_ino) {
            return Err(io::Error::other("snapshot file changed during open"));
        }
        Ok(File::from(fd))
    }

    pub(super) fn target(&self, name: &OsStr) -> io::Result<Vec<u8>> {
        Ok(fs::readlinkat(&self.0, name, Vec::new())?.into_bytes())
    }

    pub(super) fn mkdir(&self, name: &OsStr) -> io::Result<Self> {
        fs::mkdirat(&self.0, name, Mode::RUSR | Mode::WUSR | Mode::XUSR)?;
        self.sync()?;
        self.directory(name)
    }

    pub(super) fn chmod(&self, mode: u32) -> io::Result<()> {
        fs::fchmod(&self.0, Mode::from_raw_mode(mode))?;
        self.sync()
    }

    pub(super) fn create(&self, name: &OsStr) -> io::Result<File> {
        let fd = fs::openat(
            &self.0,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | SAFE,
            Mode::RUSR | Mode::WUSR,
        )?;
        Ok(File::from(fd))
    }

    pub(super) fn link(&self, name: &OsStr, into: &Self, stage: &OsStr) -> io::Result<()> {
        let before = self.inspect(name)?;
        if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile {
            return Err(io::Error::other("hard link anchor must be a regular file"));
        }
        fs::linkat(&self.0, name, &into.0, stage, AtFlags::empty())?;
        let after = into.inspect(stage)?;
        if (before.st_dev, before.st_ino) != (after.st_dev, after.st_ino) {
            return Err(io::Error::other("hard link anchor changed during restore"));
        }
        Ok(())
    }

    pub(super) fn symlink(&self, target: &[u8], name: &OsStr) -> io::Result<()> {
        Ok(fs::symlinkat(OsStr::from_bytes(target), &self.0, name)?)
    }

    pub(super) fn replace(&self, stage: &OsStr, into: &Self, name: &OsStr) -> io::Result<()> {
        fs::renameat(&self.0, stage, &into.0, name)?;
        into.sync()?;
        if !std::ptr::eq(self, into) {
            self.sync()?;
        }
        Ok(())
    }

    pub(super) fn remove(&self, name: &OsStr, flags: AtFlags) -> io::Result<()> {
        fs::unlinkat(&self.0, name, flags)?;
        self.sync()
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        Ok(fs::fsync(&self.0)?)
    }
}
