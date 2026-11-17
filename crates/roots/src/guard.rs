//! Descriptor-pinned startup pairing, validation, setup and lock lifetime.
use crate::Roots;
use calternal_fsio::DataLock;
use rustix::fs::{self, AtFlags, Mode, OFlags, RenameFlags};
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const MARKER: &str = ".calternal-install-id";
const ROLES: [&str; 3] = ["userdata-ssd", "userdata-hdd", "appdata"];
const RETRY: Duration = Duration::from_millis(100);

/// Lifetime ownership of all three roots. Dropping releases every acquired lock.
/// Keep this guard alive until all tasks capable of root I/O have stopped.
/// It holds directory descriptors as well as lock descriptors; it does not make
/// unrelated path-based I/O in other crates descriptor-relative.
pub struct RootLocks {
    _locks: [DataLock; 3],
    _directories: [OwnedFd; 3],
}

/// Validates three roles against an externally pinned installation UUID, then
/// locks all three together. `create` permits only explicit first setup of empty
/// or missing directories; it cannot repair any paired or occupied installation.
/// New setup publishes role-bearing markers durably, but creates no databases.
/// Partial lock acquisition releases earlier locks on failure. Validation errors
/// create neither directories nor lock files; I/O failures during explicit setup
/// may leave partial setup which requires deliberate offline recovery.
pub fn prepare(roots: &Roots, installation: Uuid, create: bool) -> io::Result<RootLocks> {
    let paths =
        [&roots.userdata_ssd, &roots.userdata_hdd, &roots.appdata].map(|p| absolute_plain(p));
    let [a, b, c] = paths;
    let paths = [a?, b?, c?];
    for (i, a) in paths.iter().enumerate() {
        for b in &paths[i + 1..] {
            if a.starts_with(b) || b.starts_with(a) {
                return Err(invalid(
                    "storage roles require distinct, nonnested root directories",
                ));
            }
        }
    }
    // Pin every existing prefix before considering creation of a missing root.
    let mut probes = paths
        .iter()
        .map(|p| Probe::open(p))
        .collect::<io::Result<Vec<_>>>()?;
    let paired = probes
        .iter()
        .map(|p| p.marker())
        .collect::<io::Result<Vec<_>>>()?;
    let setup = create && paired.iter().all(Option::is_none);
    if setup {
        for probe in &probes {
            if probe.has_data()? {
                return Err(invalid(
                    "unpaired storage contains data; explicit offline migration is required",
                ));
            }
        }
    } else {
        for (i, probe) in probes.iter().enumerate() {
            if !probe.missing.is_empty() {
                return Err(invalid(format!(
                    "{} root {} is missing; disk may not be mounted",
                    ROLES[i],
                    paths[i].display()
                )));
            }
            if paired[i].as_deref() != Some(marker(installation, ROLES[i]).as_str()) {
                return Err(invalid(format!(
                    "missing, invalid or mismatched storage pairing/role at {}",
                    paths[i].display()
                )));
            }
        }
        require_auth(&probes[2].fd)?;
        check_legacy(&probes[0].fd, &probes[2].fd)?;
        check_legacy(&probes[1].fd, &probes[2].fd)?;
    }
    // Reject all invalid lock entries before opening any CREATE lock sentinel.
    for probe in &probes {
        if probe.missing.is_empty() {
            regular_optional(&probe.fd, ".lock")?;
        }
    }
    // Reject aliases such as bind mounts of the very same directory too.
    let identities = probes
        .iter()
        .filter(|p| p.missing.is_empty())
        .map(|p| fs::fstat(&p.fd))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, a) in identities.iter().enumerate() {
        if identities[i + 1..]
            .iter()
            .any(|b| a.st_dev == b.st_dev && a.st_ino == b.st_ino)
        {
            return Err(invalid("root roles alias the same directory"));
        }
    }
    for probe in &mut probes {
        probe.create_missing()?;
    }
    let locks = probes
        .iter()
        .zip(&paths)
        .map(|(p, path)| DataLock::acquire_at(&p.fd, path))
        .collect::<io::Result<Vec<_>>>()?;
    // A competing first setup may have published markers while we took locks.
    for (i, probe) in probes.iter().enumerate() {
        let now = probe.marker()?;
        if now != paired[i] {
            return Err(invalid(
                "storage markers changed during lock acquisition; retry validation",
            ));
        }
    }
    if setup {
        // Recheck under all locks; never adopt data that arrived during setup.
        for probe in &probes {
            if probe.has_data()? {
                return Err(invalid("storage changed during setup; refusing adoption"));
            }
        }
        for (i, probe) in probes.iter().enumerate() {
            write_marker(&probe.fd, marker(installation, ROLES[i]).as_bytes(), false)?;
        }
    } else {
        require_auth(&probes[2].fd)?;
        check_legacy(&probes[0].fd, &probes[2].fd)?;
        check_legacy(&probes[1].fd, &probes[2].fd)?;
    }
    Ok(RootLocks {
        _locks: locks
            .try_into()
            .unwrap_or_else(|_| unreachable!("three locks")),
        _directories: probes
            .into_iter()
            .map(|p| p.fd)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or_else(|_| unreachable!("three roots")),
    })
}

/// Deliberately re-pairs three existing roots during an offline migration.
/// The caller must explicitly confirm the operation. It never creates root
/// directories or databases, and requires nonempty auth and legacy history
/// presence before locking or replacing markers. This checks presence only,
/// not migration provenance, inventory equality, or FO1 recovery snapshots.
/// Existing valid role labels cannot be swapped by re-pair. Corrupt/missing
/// markers can be replaced only through this explicitly confirmed operation.
pub fn re_pair(roots: &Roots, installation: Uuid, confirmed: bool) -> io::Result<()> {
    if !confirmed {
        return Err(invalid("offline re-pair requires explicit confirmation"));
    }
    let paths = [&roots.userdata_ssd, &roots.userdata_hdd, &roots.appdata]
        .into_iter()
        .map(|p| absolute_plain(p))
        .collect::<io::Result<Vec<_>>>()?;
    for (i, a) in paths.iter().enumerate() {
        if paths[i + 1..]
            .iter()
            .any(|b| a.starts_with(b) || b.starts_with(a))
        {
            return Err(invalid(
                "storage roles require distinct, nonnested root directories",
            ));
        }
    }
    let probes = paths
        .iter()
        .map(|p| Probe::open(p))
        .collect::<io::Result<Vec<_>>>()?;
    for (i, probe) in probes.iter().enumerate() {
        if !probe.missing.is_empty() {
            return Err(invalid("re-pair requires existing mounted roots"));
        }
        if let Some(record) = probe.marker()? {
            let mut lines = record.lines();
            if lines
                .next()
                .and_then(|id| Uuid::parse_str(id).ok())
                .is_some()
                && let Some(role) = lines.next()
                && ROLES.contains(&role)
                && role != ROLES[i]
            {
                return Err(invalid("re-pair cannot swap root roles"));
            }
        }
        regular_optional(&probe.fd, ".lock")?;
    }
    let identities = probes
        .iter()
        .map(|p| fs::fstat(&p.fd))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, a) in identities.iter().enumerate() {
        if identities[i + 1..]
            .iter()
            .any(|b| a.st_dev == b.st_dev && a.st_ino == b.st_ino)
        {
            return Err(invalid("root roles alias the same directory"));
        }
    }
    require_auth(&probes[2].fd)?;
    check_legacy(&probes[0].fd, &probes[2].fd)?;
    check_legacy(&probes[1].fd, &probes[2].fd)?;
    let _locks = probes
        .iter()
        .zip(&paths)
        .map(|(p, path)| DataLock::acquire_at(&p.fd, path))
        .collect::<io::Result<Vec<_>>>()?;
    require_auth(&probes[2].fd)?;
    check_legacy(&probes[0].fd, &probes[2].fd)?;
    check_legacy(&probes[1].fd, &probes[2].fd)?;
    for (i, probe) in probes.iter().enumerate() {
        write_marker(&probe.fd, marker(installation, ROLES[i]).as_bytes(), true)?;
    }
    Ok(())
}

/// Retries only lock contention for at most `wait`, notifying `on_wait` once.
/// The caller chooses a bound covering its shutdown grace. Validation failures
/// return immediately. The Tokio timer makes release/deadline tests deterministic.
pub async fn prepare_with_wait(
    roots: &Roots,
    installation: Uuid,
    create: bool,
    wait: Duration,
    mut on_wait: impl FnMut(),
) -> io::Result<RootLocks> {
    let deadline = tokio::time::Instant::now() + wait;
    let mut reported = false;
    loop {
        match prepare(roots, installation, create) {
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                if !reported {
                    on_wait();
                    reported = true;
                }
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if remaining.is_zero() {
                    return Err(e);
                }
                tokio::time::sleep(RETRY.min(remaining)).await;
            }
            result => return result,
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn marker(id: Uuid, role: &str) -> String {
    format!("{id}\n{role}\n")
}

fn absolute_plain(path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if path
        .components()
        .any(|c| !matches!(c, Component::RootDir | Component::Normal(_)))
    {
        return Err(invalid(
            "storage root must have plain components without . or ..",
        ));
    }
    Ok(path)
}

fn dir_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

struct Probe {
    fd: OwnedFd,
    missing: Vec<std::ffi::OsString>,
}

impl Probe {
    fn open(path: &Path) -> io::Result<Self> {
        let mut fd = fs::open("/", dir_flags(), Mode::empty())?;
        let mut missing = Vec::new();
        for part in path.components().filter_map(|c| {
            if let Component::Normal(p) = c {
                Some(p)
            } else {
                None
            }
        }) {
            if missing.is_empty() {
                match fs::openat(&fd, part, dir_flags(), Mode::empty()) {
                    Ok(next) => {
                        fd = next;
                        continue;
                    }
                    Err(rustix::io::Errno::NOENT) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            missing.push(part.to_owned());
        }
        Ok(Self { fd, missing })
    }

    fn marker(&self) -> io::Result<Option<String>> {
        if !self.missing.is_empty() {
            return Ok(None);
        }
        let Some(file) = regular_optional(&self.fd, MARKER)? else {
            return Ok(None);
        };
        let mut bytes = String::new();
        file.take(256).read_to_string(&mut bytes)?;
        Ok(Some(bytes))
    }

    fn has_data(&self) -> io::Result<bool> {
        if self.missing.is_empty() {
            has_data(&self.fd)
        } else {
            Ok(false)
        }
    }

    fn create_missing(&mut self) -> io::Result<()> {
        for part in self.missing.drain(..) {
            match fs::mkdirat(&self.fd, &part, Mode::from_raw_mode(0o755)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(e) => return Err(e.into()),
            }
            fs::fsync(&self.fd)?;
            self.fd = fs::openat(&self.fd, &part, dir_flags(), Mode::empty())?;
        }
        Ok(())
    }
}

fn regular_optional(dir: &OwnedFd, name: impl AsRef<Path>) -> io::Result<Option<File>> {
    let fd = match fs::openat(
        dir,
        name.as_ref(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(invalid("expected a regular storage file"));
    }
    Ok(Some(file))
}

fn require_auth(appdata: &OwnedFd) -> io::Result<()> {
    let system = fs::openat(appdata, "system", dir_flags(), Mode::empty()).map_err(|e| {
        invalid(format!(
            "appdata missing system/auth.db: {e}; restore original appdata"
        ))
    })?;
    if regular_optional(&system, "auth.db")?
        .is_some_and(|f| f.metadata().is_ok_and(|m| m.len() > 0))
    {
        Ok(())
    } else {
        Err(invalid(
            "appdata missing nonempty system/auth.db; refusing fresh databases until validated FO1 recovery",
        ))
    }
}

fn names(dir: &OwnedFd) -> io::Result<Vec<std::ffi::OsString>> {
    let mut entries = fs::Dir::read_from(dir)?;
    let mut result = Vec::new();
    while let Some(entry) = entries.read() {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_bytes() != b"." && name.to_bytes() != b".." {
            use std::os::unix::ffi::OsStrExt;
            result.push(std::ffi::OsStr::from_bytes(name.to_bytes()).to_owned());
        }
    }
    Ok(result)
}

fn has_data(dir: &OwnedFd) -> io::Result<bool> {
    for name in names(dir)? {
        let st = fs::statat(dir, &name, AtFlags::SYMLINK_NOFOLLOW)?;
        let kind = fs::FileType::from_raw_mode(st.st_mode);
        if name == ".lock" && kind == fs::FileType::RegularFile {
            continue;
        }
        if kind != fs::FileType::Directory {
            return Ok(true);
        }
        let child = fs::openat(dir, &name, dir_flags(), Mode::empty())?;
        if has_data(&child)? {
            return Ok(true);
        }
    }
    Ok(false)
}

// Preserve the source's legacy auth/history presence fence. This is not a
// provenance or content-equivalence check, nor a registry of future space stores.
fn check_legacy(space: &OwnedFd, appdata: &OwnedFd) -> io::Result<()> {
    let mut relative = vec![PathBuf::from("system/auth.db")];
    match fs::openat(space, "spaces", dir_flags(), Mode::empty()) {
        Ok(spaces) => {
            for user in names(&spaces)? {
                relative.push(Path::new("spaces").join(user).join("notes/history.db"));
            }
        }
        Err(rustix::io::Errno::NOENT) => {}
        Err(e) => return Err(e.into()),
    }
    for rel in relative {
        if exists_plain(space, &rel)? && !exists_plain(appdata, &rel)? {
            return Err(invalid(format!(
                "copy {} to appdata while stopped before splitting storage",
                rel.display()
            )));
        }
    }
    Ok(())
}

fn exists_plain(root: &OwnedFd, rel: &Path) -> io::Result<bool> {
    let mut dir = root.try_clone()?;
    let mut parts = rel.components().peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return Ok(regular_optional(&dir, Path::new(part.as_os_str()))?.is_some());
        }
        match fs::openat(&dir, part.as_os_str(), dir_flags(), Mode::empty()) {
            Ok(next) => dir = next,
            Err(rustix::io::Errno::NOENT) => return Ok(false),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(false)
}

fn write_marker(dir: &OwnedFd, bytes: &[u8], replace: bool) -> io::Result<()> {
    let name = format!(
        "{}{}-install-id",
        calternal_fsio::TEMP_PREFIX,
        Uuid::new_v4()
    );
    let result = (|| {
        let fd = fs::openat(
            dir,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )?;
        let mut file = File::from(fd);
        file.write_all(bytes)?;
        file.sync_all()?;
        if replace {
            fs::renameat(dir, name.as_str(), dir, MARKER)?;
        } else {
            fs::renameat_with(dir, name.as_str(), dir, MARKER, RenameFlags::NOREPLACE)?;
        }
        fs::fsync(dir)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::unlinkat(dir, name.as_str(), AtFlags::empty());
    }
    result
}
