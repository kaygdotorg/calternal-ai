//! Publish a sleeping space as one durable, immutable Firecracker generation.
use calternal_types::SpaceId;
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

/// How Firecracker memory was materialized for this generation.
#[derive(Clone, Copy)]
pub(super) enum Storage {
    /// Complete Firecracker snapshot.
    Full,
    /// Previous image cloned and patched with this checkpoint’s dirty extents.
    Reflink,
}

impl Storage {
    /// Persisted storage mode and status label.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Reflink => "dirty-merge-reflink",
        }
    }
}

/// Identity and compatibility requirements for reopening a published guest.
#[derive(Clone)]
pub(super) struct Binding {
    /// Space that owns the memory and scratch disk.
    pub(super) space: SpaceId,
    /// Digest of the immutable guest image manifest.
    pub(super) image: String,
    /// Firecracker release reported during machine initialization.
    pub(super) firecracker: String,
}

/// One materialized checkpoint and the host device names captured by its VMM.
#[derive(Clone)]
pub(super) struct Generation {
    /// Unique checkpoint identity.
    pub(super) id: Uuid,
    /// Server-issued monotonic authorization number.
    pub(super) counter: u64,
    /// Private generation directory.
    pub(super) path: PathBuf,
    /// Memory image materialization mode.
    pub(super) storage: Storage,
    /// Host scratch disk filename recorded by Firecracker.
    pub(super) disk_name: String,
    /// Host control drive filename recorded by Firecracker.
    pub(super) control_name: String,
}

impl Generation {
    /// Create a draft under the space gate while keeping the published generation.
    pub(super) fn create(
        root: &Path,
        disk: &Path,
        control: &Path,
        counter: u64,
    ) -> io::Result<Self> {
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
            counter,
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

    /// Validate the bounded manifest and component metadata before mapping memory.
    pub(super) fn current(root: &Path, binding: &Binding) -> io::Result<Option<Self>> {
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
        if field("format: ")? != "1"
            || field("space: ")?
                .parse::<SpaceId>()
                .map_err(|_| invalid())?
                != binding.space
            || field("image: ")? != binding.image
            || field("firecracker: ")? != binding.firecracker
        {
            return Err(invalid());
        }
        let counter = field("counter: ")?
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(invalid)?;
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
            counter,
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
    pub(super) fn publish(&self, root: &Path, binding: &Binding) -> io::Result<()> {
        self.publish_with(root, binding, |path, bytes| {
            calternal_fsio::write_atomic_mode(path, bytes, 0o600)
        })
    }

    fn publish_with(
        &self,
        root: &Path,
        binding: &Binding,
        commit: impl FnOnce(&Path, &[u8]) -> io::Result<()>,
    ) -> io::Result<()> {
        let mut manifest = format!(
            "format: 1\nspace: {}\nimage: {}\nfirecracker: {}\ncounter: {}\ngeneration: {}\nstorage: {}\nactive-disk: {}\nactive-control: {}\n",
            binding.space,
            binding.image,
            binding.firecracker,
            self.counter,
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
        commit(&root.join(MANIFEST), manifest.as_bytes())
    }
}

/// Use pointer age for published data and directory age for abandoned drafts.
pub(super) async fn expired(
    root: &Path,
    retention: std::time::Duration,
) -> super::SandboxResult<bool> {
    let metadata = match tokio::fs::symlink_metadata(root.join(MANIFEST)).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match tokio::fs::symlink_metadata(root).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(super::SandboxError::new(error)),
            }
        }
        Err(error) => return Err(super::SandboxError::new(error)),
    };
    let modified = metadata.modified().map_err(super::SandboxError::new)?;
    Ok(modified.elapsed().map_or(true, |age| age >= retention))
}

/// Read the recorded server number without opening the snapshot components.
pub(super) fn saved_counter(root: &Path) -> io::Result<Option<u64>> {
    Ok(read_manifest(root)?.and_then(|manifest| {
        manifest
            .lines()
            .find_map(|line| line.strip_prefix("counter: "))
            .and_then(|number| number.parse().ok())
    }))
}

/// Read the restore failure budget for the exact checkpoint number.
pub(super) fn restore_failures(root: &Path, counter: u64) -> io::Result<u8> {
    let mut text = String::new();
    match File::open(root.join("current.failures")) {
        Ok(file) => {
            file.take(64).read_to_string(&mut text)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(if text.trim() == format!("{counter} 2") {
        2
    } else if text.trim() == format!("{counter} 1") {
        1
    } else {
        0
    })
}

/// Record at most two failures for the authorized generation across role restarts.
pub(super) fn failed_restore(root: &Path, counter: u64) -> io::Result<bool> {
    let failures = (restore_failures(root, counter)? + 1).min(2);
    calternal_fsio::write_atomic_mode(
        &root.join("current.failures"),
        format!("{counter} {failures}\n").as_bytes(),
        0o600,
    )?;
    Ok(failures == 2)
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

/// Clone on reflink filesystems, or copy allocated extents without filling holes.
pub(super) fn clone_or_copy(source: &Path, destination: &Path) -> io::Result<()> {
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(destination)?;
    match ioctl_ficlone(&output, &input) {
        Ok(()) => Ok(()),
        Err(Errno::NOTSUP | Errno::NOTTY | Errno::XDEV | Errno::INVAL | Errno::NOSYS) => {
            let length = input.metadata()?.len();
            if !copy_extents(&mut input, &mut output, length)? {
                input.rewind()?;
                output.rewind()?;
                calternal_fsio::paced::copy(&mut output, &mut input)?;
            }
            output.set_len(length)
        }
        Err(error) => Err(error.into()),
    }
}

fn copy_extents(input: &mut File, output: &mut File, length: u64) -> io::Result<bool> {
    let mut cursor = 0;
    let mut buffer = vec![0; calternal_fsio::paced::STEP];
    while cursor < length {
        let start = match seek(&*input, SeekFrom::Data(cursor)) {
            Ok(start) if start < length => start,
            Ok(_) | Err(Errno::NXIO) => break,
            Err(Errno::INVAL | Errno::NOTSUP) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        let end = match seek(&*input, SeekFrom::Hole(start)) {
            Ok(end) => end.min(length),
            Err(Errno::INVAL | Errno::NOTSUP) => return Ok(false),
            Err(error) => return Err(error.into()),
        };
        if end <= start {
            return Err(invalid());
        }
        input.seek(io::SeekFrom::Start(start))?;
        output.seek(io::SeekFrom::Start(start))?;
        cursor = start;
        while cursor < end {
            let count = (end - cursor).min(buffer.len() as u64) as usize;
            let chunk = &mut buffer[..count];
            input.read_exact(chunk)?;
            calternal_fsio::paced::write_chunk(output, chunk, cursor)?;
            cursor += count as u64;
        }
    }
    Ok(true)
}

fn record(generation: &Generation, component: &str) -> io::Result<String> {
    let metadata = fs::symlink_metadata(generation.path.join(component)).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            invalid()
        } else {
            error
        }
    })?;
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
        let supported = copy_extents(&mut diff, &mut memory, length)?;
        memory.flush()?;
        Ok(supported)
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

    fn binding() -> Binding {
        Binding {
            space: SpaceId::new(),
            image: "fixture".into(),
            firecracker: "v1.16.1".into(),
        }
    }

    fn fill(generation: &Generation, content: &[u8]) {
        for component in COMPONENTS {
            fs::write(generation.path.join(component), content).unwrap();
        }
    }

    #[test]
    fn sparse_copy_keeps_holes_and_dirty_zero_blocks() {
        use std::os::unix::fs::FileExt as _;
        let root = tempfile::tempdir().unwrap();
        let source = File::create(root.path().join("source")).unwrap();
        let mut input = File::open(root.path().join("source")).unwrap();
        let mut output = File::create(root.path().join("copy")).unwrap();
        let length = 16 * 1024 * 1024;
        source.set_len(length).unwrap();
        source.write_all_at(&[1; 4096], 0).unwrap();
        source.write_all_at(&[0; 4096], length - 4096).unwrap();
        assert!(copy_extents(&mut input, &mut output, length).unwrap());
        output.set_len(length).unwrap();
        assert_eq!(output.metadata().unwrap().len(), length);
        assert!(output.metadata().unwrap().blocks() * 512 < length / 2);
        assert_eq!(
            seek(&output, SeekFrom::Data(length / 2)).unwrap(),
            length - 4096
        );
        let mut buffer = [0; 4096];
        File::open(root.path().join("copy"))
            .unwrap()
            .read_exact_at(&mut buffer, 0)
            .unwrap();
        assert_eq!(buffer, [1; 4096]);
    }

    #[test]
    fn unpublished_snapshot_preserves_previous_and_a_mixed_inode_is_refused() {
        if let (Some(root), Ok(space), Ok(draft)) = (
            std::env::var_os("CALTERNAL_SNAPSHOT_PUBLICATION_ROOT"),
            std::env::var("CALTERNAL_SNAPSHOT_PUBLICATION_SPACE"),
            std::env::var("CALTERNAL_SNAPSHOT_PUBLICATION_DRAFT"),
        ) {
            let root = PathBuf::from(root);
            let binding = Binding {
                space: space.parse().unwrap(),
                ..binding()
            };
            let incoming = Generation {
                id: draft.parse().unwrap(),
                counter: 2,
                path: root.join(&draft),
                storage: Storage::Full,
                disk_name: "fixture.ext4".into(),
                control_name: "fixture.key".into(),
            };
            incoming
                .publish_with(&root, &binding, |_, _| std::process::exit(86))
                .unwrap();
            panic!("publication failed to reach the crash injection");
        }
        let root = tempfile::tempdir().unwrap();
        let binding = binding();
        let old = Generation::create(
            root.path(),
            Path::new("fixture.ext4"),
            Path::new("fixture.key"),
            1,
        )
        .unwrap();
        fill(&old, b"old");
        old.publish(root.path(), &binding).unwrap();
        let new = Generation::create(
            root.path(),
            Path::new("fixture.ext4"),
            Path::new("fixture.key"),
            2,
        )
        .unwrap();
        fill(&new, b"new");
        assert_eq!(
            Generation::current(root.path(), &binding)
                .unwrap()
                .unwrap()
                .id,
            old.id
        );
        let crashed = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "sandbox::microvm::snapshot::tests::unpublished_snapshot_preserves_previous_and_a_mixed_inode_is_refused"])
            .env("CALTERNAL_SNAPSHOT_PUBLICATION_ROOT", root.path())
            .env("CALTERNAL_SNAPSHOT_PUBLICATION_SPACE", binding.space.to_string())
            .env("CALTERNAL_SNAPSHOT_PUBLICATION_DRAFT", new.id.to_string())
            .output().unwrap();
        assert_eq!(crashed.status.code(), Some(86), "{crashed:?}");
        assert_eq!(
            Generation::current(root.path(), &binding)
                .unwrap()
                .unwrap()
                .id,
            old.id,
        );
        let foreign = Binding {
            space: SpaceId::new(),
            ..binding.clone()
        };
        assert!(Generation::current(root.path(), &foreign).is_err());
        let upgraded = Binding {
            firecracker: "v1.17.0".into(),
            ..binding.clone()
        };
        assert!(Generation::current(root.path(), &upgraded).is_err());
        new.publish(root.path(), &binding).unwrap();
        assert_eq!(
            Generation::current(root.path(), &binding)
                .unwrap()
                .unwrap()
                .id,
            new.id
        );
        fs::rename(old.path.join("memory"), new.path.join("memory")).unwrap();
        assert_eq!(
            Generation::current(root.path(), &binding)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
