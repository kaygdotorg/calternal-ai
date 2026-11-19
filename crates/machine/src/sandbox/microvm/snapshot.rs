//! Publish a sleeping space as one durable, immutable Firecracker generation.
use rustix::{
    fs::{SeekFrom, ioctl_ficlone, seek},
    io::Errno,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek as _, Write as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MANIFEST: &str = "current";
const COMPONENTS: [&str; 4] = ["state", "memory", "disk", "control"];
const MANIFEST_BYTES: u64 = 4096;

#[derive(Clone, Copy)]
pub(super) enum Storage {
    Full,
    Reflink,
}

impl Storage {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reflink => "dirty-merge-reflink",
        }
    }
}

#[derive(Clone)]
pub(super) struct Generation {
    pub(super) id: Uuid,
    pub(super) path: PathBuf,
    pub(super) storage: Storage,
    pub(super) disk_name: String,
    pub(super) control_name: String,
}

impl Generation {
    pub(super) fn create(root: &Path, disk: &Path, control: &Path) -> io::Result<Self> {
        calternal_fsio::create_dir_all_durable(root)?;
        let keep = read_manifest(root)?
            .as_deref()
            .and_then(|manifest| {
                manifest
                    .lines()
                    .find_map(|line| line.strip_prefix("generation: "))
            })
            .and_then(|id| Uuid::parse_str(id).ok());
        prune_drafts(root, keep)?;
        let id = Uuid::new_v4();
        let path = root.join(id.to_string());
        fs::create_dir(&path)?;
        Ok(Self {
            id,
            path,
            storage: Storage::Full,
            disk_name: disk
                .file_name()
                .and_then(|part| part.to_str())
                .ok_or_else(invalid)?
                .to_owned(),
            control_name: control
                .file_name()
                .and_then(|part| part.to_str())
                .ok_or_else(invalid)?
                .to_owned(),
        })
    }

    pub(super) fn current(root: &Path, image: &str) -> io::Result<Option<Self>> {
        let Some(text) = read_manifest(root)? else {
            return Ok(None);
        };
        let mut fields = text.lines();
        let mut field = |prefix| {
            fields
                .next()
                .and_then(|line| line.strip_prefix(prefix))
                .ok_or_else(invalid)
        };
        if field("format: ")? != "1" || field("image: ")? != image {
            return Err(invalid());
        }
        let id = Uuid::parse_str(field("generation: ")?).map_err(|_| invalid())?;
        let storage = match field("storage: ")? {
            "full" => Storage::Full,
            "dirty-merge-reflink" => Storage::Reflink,
            _ => return Err(invalid()),
        };
        let disk_name = device_name(field("active-disk: ")?, "ext4")?;
        let control_name = device_name(field("active-control: ")?, "key")?;
        let generation = Self {
            path: root.join(id.to_string()),
            id,
            storage,
            disk_name,
            control_name,
        };
        // Inspect stable inode metadata only. Scanning memory here would read
        // the working set eagerly and defeat Firecracker's lazy restore.
        for name in COMPONENTS {
            let recorded = fields.next().ok_or_else(invalid)?;
            if record(&generation, name)? != recorded {
                return Err(invalid());
            }
        }
        if fields.next().is_some() {
            return Err(invalid());
        }
        Ok(Some(generation))
    }

    /// Persist each materialized component and the generation directory before
    /// fsyncing the atomic pointer replacement. The previous generation is
    /// still usable after a failure before publication.
    pub(super) fn publish(&self, root: &Path, image: &str) -> io::Result<()> {
        let mut manifest = format!(
            "format: 1\nimage: {image}\ngeneration: {}\nstorage: {}\nactive-disk: {}\nactive-control: {}\n",
            self.id,
            self.storage.label(),
            self.disk_name,
            self.control_name
        );
        for name in COMPONENTS {
            File::open(self.path.join(name))?.sync_all()?;
            manifest.push_str(&record(self, name)?);
            manifest.push('\n');
        }
        calternal_fsio::sync_dir(&self.path)?;
        calternal_fsio::sync_dir(root)?;
        calternal_fsio::write_atomic_mode(&root.join(MANIFEST), manifest.as_bytes(), 0o600)
    }
}

fn read_manifest(root: &Path) -> io::Result<Option<String>> {
    let file = match File::open(root.join(MANIFEST)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut manifest = String::new();
    file.take(MANIFEST_BYTES + 1)
        .read_to_string(&mut manifest)?;
    if manifest.len() as u64 > MANIFEST_BYTES {
        return Err(invalid());
    }
    Ok(Some(manifest))
}

/// Prune interrupted generations under the space's placement gate, without
/// unlinking the generation named by the durable current pointer.
pub(super) fn prune_drafts(root: &Path, keep: Option<Uuid>) -> io::Result<()> {
    for candidate in fs::read_dir(root)? {
        let candidate = candidate?;
        if let Ok(id) = Uuid::parse_str(&candidate.file_name().to_string_lossy())
            && Some(id) != keep
            && candidate.file_type()?.is_dir()
        {
            fs::remove_dir_all(candidate.path())?;
        }
    }
    Ok(())
}

fn device_name(value: &str, extension: &str) -> io::Result<String> {
    let path = Path::new(value);
    if path.file_name().and_then(|part| part.to_str()) != Some(value)
        || path.extension().and_then(|part| part.to_str()) != Some(extension)
    {
        return Err(invalid());
    }
    Ok(value.to_owned())
}

pub(super) fn clone_or_copy(source: &Path, destination: &Path) -> io::Result<()> {
    let input = File::open(source)?;
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    match ioctl_ficlone(&output, &input) {
        Ok(()) => Ok(()),
        Err(Errno::NOTSUP | Errno::NOTTY | Errno::XDEV | Errno::INVAL | Errno::NOSYS) => {
            io::copy(&mut &input, &mut &output).map(|_| ())
        }
        Err(error) => Err(error.into()),
    }
}

fn record(generation: &Generation, component: &str) -> io::Result<String> {
    let metadata = fs::symlink_metadata(generation.path.join(component))?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(invalid());
    }
    Ok(format!(
        "{component}: {} {} {} {} {} {}",
        generation.id,
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    ))
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "inconsistent small VM snapshot generation",
    )
}

/// Clone the previous full image, then replace the dirty file's allocated
/// ranges, including written zero pages. Unsupported cloning or sparse seeks
/// requests a full snapshot from Firecracker before publication.
pub(super) fn merge_dirty(base: &Path, diff: &Path, target: &Path) -> io::Result<bool> {
    let base = File::open(base)?;
    let mut diff = File::open(diff)?;
    let mut memory = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(target)?;
    let supported = (|| {
        match ioctl_ficlone(&memory, &base) {
            Ok(()) => {}
            Err(Errno::NOTSUP | Errno::NOTTY | Errno::XDEV | Errno::INVAL | Errno::NOSYS) => {
                return Ok(false);
            }
            Err(error) => return Err(error.into()),
        }
        let length = diff.metadata()?.len();
        if length != base.metadata()?.len() {
            return Err(invalid());
        }
        let mut cursor = 0;
        while cursor < length {
            let start = match seek(&diff, SeekFrom::Data(cursor)) {
                Ok(start) if start < length => start,
                Ok(_) | Err(Errno::NXIO) => break,
                Err(Errno::INVAL | Errno::NOTSUP) => return Ok(false),
                Err(error) => return Err(error.into()),
            };
            let end = seek(&diff, SeekFrom::Hole(start))?.min(length);
            if end <= start {
                return Err(invalid());
            }
            diff.seek(io::SeekFrom::Start(start))?;
            memory.seek(io::SeekFrom::Start(start))?;
            if io::copy(&mut (&mut diff).take(end - start), &mut memory)? != end - start {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            cursor = end;
        }
        memory.flush()?;
        Ok(true)
    })();
    if !matches!(supported, Ok(true)) {
        drop(memory);
        fs::remove_file(target)?;
    }
    supported
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(generation: &Generation, content: &[u8]) {
        for component in COMPONENTS {
            fs::write(generation.path.join(component), content).unwrap();
        }
    }

    #[test]
    fn unpublished_snapshot_preserves_previous_and_a_mixed_inode_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let old = Generation::create(
            root.path(),
            Path::new("fixture.ext4"),
            Path::new("fixture.key"),
        )
        .unwrap();
        fill(&old, b"old");
        old.publish(root.path(), "fixture").unwrap();
        let new = Generation::create(
            root.path(),
            Path::new("fixture.ext4"),
            Path::new("fixture.key"),
        )
        .unwrap();
        fill(&new, b"new");
        assert_eq!(
            Generation::current(root.path(), "fixture")
                .unwrap()
                .unwrap()
                .id,
            old.id
        );
        new.publish(root.path(), "fixture").unwrap();
        assert_eq!(
            Generation::current(root.path(), "fixture")
                .unwrap()
                .unwrap()
                .id,
            new.id
        );
        fs::rename(old.path.join("memory"), new.path.join("memory")).unwrap();
        assert_eq!(
            Generation::current(root.path(), "fixture")
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
