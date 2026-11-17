//! A current view of directory names without rescanning a large folder on
//! every allocation. Kernel events are drained synchronously, independently
//! of the notes watcher. Lost events always cause a fresh snapshot.
use super::fold;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    io,
    path::Path,
};

/// Current folded directory names, refreshed under the caller's allocation lock.
/// Lost kernel events force a complete snapshot before names are allocated.
#[derive(Default)]
pub struct DirectoryNames {
    names: Names,
    #[cfg(any(target_os = "linux", target_os = "android"))]
    watch: Option<Watch>,
}

impl DirectoryNames {
    /// Refresh while holding the caller's allocation lock. Linux drains every
    /// pending entry event; other platforms read the directory each time.
    pub fn refresh(&mut self, directory: &Path) -> io::Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = match std::fs::metadata(directory) {
                Ok(metadata) => metadata,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    self.watch = None;
                    self.names = Names::default();
                    return Ok(());
                }
                Err(e) => return Err(e),
            };
            let identity = (metadata.dev(), metadata.ino());
            if let Some(watch) = &mut self.watch
                && watch.identity == identity
                && let Ok((_, false)) = watch.drain(directory, &mut self.names)
            {
                return Ok(());
            }
            self.watch = None;
            for _ in 0..3 {
                let mut watch = match Watch::new(directory) {
                    Ok(watch) => watch,
                    // Watch limits or unsupported filesystems cannot make names
                    // stale: use the ordinary directory read on every call.
                    Err(_) => {
                        self.names = snapshot(directory)?;
                        return Ok(());
                    }
                };
                let mut names = snapshot(directory)?;
                let metadata = std::fs::metadata(directory)?;
                let stable_identity = (metadata.dev(), metadata.ino()) == watch.identity;
                // Any entry event during the scan could otherwise be counted
                // twice. Retry with a new watch and a quiet, complete snapshot.
                if stable_identity
                    && matches!(watch.drain(directory, &mut names), Ok((false, false)))
                {
                    self.names = names;
                    self.watch = Some(watch);
                    return Ok(());
                }
            }
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "directory changed during name allocation",
            ))
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            self.names = snapshot(directory)?;
            Ok(())
        }
    }

    /// Whether a folded candidate is present, including case-only duplicates.
    pub fn taken(&self, folded: &str) -> bool {
        self.names.counts.contains_key(folded)
    }
}

#[derive(Default)]
struct Names {
    raw: HashSet<OsString>,
    counts: HashMap<String, usize>,
}

impl Names {
    fn insert(&mut self, raw: OsString) {
        if !self.raw.contains(&raw) {
            *self.counts.entry(fold(&raw.to_string_lossy())).or_default() += 1;
            self.raw.insert(raw);
        }
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    fn remove(&mut self, raw: &std::ffi::OsStr) {
        if self.raw.remove(raw) {
            let key = fold(&raw.to_string_lossy());
            if let Some(count) = self.counts.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    self.counts.remove(&key);
                }
            }
        }
    }
}

fn snapshot(directory: &Path) -> io::Result<Names> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(e) => return Err(e),
    };
    let mut names = Names::default();
    for entry in entries {
        names.insert(entry?.file_name());
    }
    Ok(names)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
struct Watch {
    fd: rustix::fd::OwnedFd,
    identity: (u64, u64),
}

#[cfg(any(target_os = "linux", target_os = "android"))]
impl Watch {
    fn new(directory: &Path) -> io::Result<Self> {
        use rustix::fs::inotify::{self, CreateFlags as C, WatchFlags as W};
        use std::os::unix::fs::MetadataExt;
        let metadata = std::fs::metadata(directory)?;
        // Remote mounts can change without local inotify events. Cache only
        // native ext, XFS, Btrfs, tmpfs and overlay directories; scan elsewhere.
        let kind = rustix::fs::statfs(directory)?.f_type as u64 & 0xffff_ffff;
        if ![0xef53, 0x5846_5342, 0x9123_683e, 0x0102_1994, 0x794c_7630].contains(&kind) {
            return Err(io::ErrorKind::Unsupported.into());
        }
        let fd = inotify::init(C::NONBLOCK | C::CLOEXEC)?;
        inotify::add_watch(
            &fd,
            directory,
            W::ONLYDIR
                | W::CREATE
                | W::DELETE
                | W::MOVED_FROM
                | W::MOVED_TO
                | W::MOVE_SELF
                | W::DELETE_SELF,
        )?;
        Ok(Self {
            fd,
            identity: (metadata.dev(), metadata.ino()),
        })
    }

    /// `(any events, invalidated)`. Reconcile affected names with the real
    /// directory after draining. Replacing or exchanging an existing name
    /// does not necessarily emit a DELETE followed by a CREATE.
    fn drain(&mut self, directory: &Path, names: &mut Names) -> io::Result<(bool, bool)> {
        use rustix::fs::inotify::{ReadFlags as R, Reader};
        use std::os::unix::ffi::OsStrExt;
        let mut buffer = [std::mem::MaybeUninit::uninit(); 8192];
        let mut reader = Reader::new(&self.fd, &mut buffer);
        let (mut any, mut invalid) = (false, false);
        let mut dirty = HashSet::new();
        loop {
            let event = match reader.next() {
                Ok(event) => event,
                Err(rustix::io::Errno::AGAIN) => break,
                Err(e) => return Err(e.into()),
            };
            any = true;
            invalid |= event.events().intersects(
                R::QUEUE_OVERFLOW | R::IGNORED | R::UNMOUNT | R::DELETE_SELF | R::MOVE_SELF,
            );
            if let Some(name) = event.file_name() {
                dirty.insert(std::ffi::OsStr::from_bytes(name.to_bytes()).to_owned());
            }
        }
        if !invalid {
            for raw in dirty {
                match std::fs::symlink_metadata(directory.join(&raw)) {
                    Ok(_) => names.insert(raw),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => names.remove(&raw),
                    Err(e) => return Err(e),
                }
            }
        }
        Ok((any, invalid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Outside changes stay current whether case variants are distinct entries or one alias.
    #[test]
    fn immediate_outside_events_case_duplicates_and_replacement_are_current() {
        let root = crate::tests::tmp();
        let dir = root.join("Tasks");
        let mut names = DirectoryNames::default();
        names.refresh(&dir).unwrap();
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("MiXeD.md"), b"outside").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("mixed.md"));
        std::fs::write(dir.join("MIXED.md"), b"second").unwrap();
        let distinct_case_entries = std::fs::read_dir(&dir).unwrap().count() == 2;
        std::fs::write(dir.join("Cafe\u{301}.md"), b"unicode").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("café.md"));
        std::fs::remove_file(dir.join("MiXeD.md")).unwrap();
        names.refresh(&dir).unwrap();
        assert_eq!(names.taken("mixed.md"), distinct_case_entries);
        if !distinct_case_entries {
            // Removing the first spelling removed the sole aliased entry too.
            std::fs::write(dir.join("MIXED.md"), b"second").unwrap();
        }
        std::fs::rename(dir.join("MIXED.md"), dir.join("Mixed 2.MD")).unwrap();
        names.refresh(&dir).unwrap();
        assert!(!names.taken("mixed.md"));
        assert!(names.taken("mixed 2.md"));
        assert_eq!(
            super::super::unique_md_name("Mixed", |n| names.taken(n)),
            "Mixed.md"
        );
        crate::write_atomic(&dir.join("Mixed 2.MD"), b"replacement bytes").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("mixed 2.md"));
        crate::write_unless_changed(&dir.join("Mixed 2.MD"), b"exchanged bytes", |_| true).unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("mixed 2.md"));
        std::fs::remove_file(dir.join("Mixed 2.MD")).unwrap();
        names.refresh(&dir).unwrap();
        assert!(!names.taken("mixed 2.md"));
        std::fs::rename(&dir, root.join("Old Tasks")).unwrap();
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("Replacement.md"), b"replacement").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("replacement.md"));
        assert!(!names.taken("mixed 2.md"));
        // Rebuilding a lost watch must read every current name again.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            names.watch = None;
        }
        std::fs::write(dir.join("Unindexed.md"), b"not indexed").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("unindexed.md"));
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn random_outside_entry_changes_match_the_real_directory() {
        let root = crate::tests::tmp();
        let dir = root.join("Tasks");
        std::fs::create_dir(&dir).unwrap();
        let mut names = DirectoryNames::default();
        names.refresh(&dir).unwrap();
        let files = [
            "Mixed.md",
            "MIXED.md",
            "Mixed 2.md",
            "Cafe\u{301}.md",
            "CAFÉ.md",
            "Other.md",
        ];
        let mut seed = 671u64;
        for _ in 0..256 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let at = (seed >> 32) as usize % files.len();
            let path = dir.join(files[at]);
            match (seed >> 40) % 7 {
                0 => std::fs::write(&path, b"outside").unwrap(),
                1 => {
                    if path.exists() {
                        std::fs::remove_file(&path).unwrap();
                    }
                }
                2 => {
                    if path.exists() {
                        std::fs::rename(&path, dir.join(files[(at + 1) % files.len()])).unwrap();
                    }
                }
                3 => crate::write_atomic(&path, b"atomic replacement").unwrap(),
                4 => {
                    crate::write_unless_changed(&path, b"exchanged", |_| true).unwrap();
                }
                5 => {
                    std::fs::remove_dir_all(&dir).unwrap();
                    std::fs::create_dir(&dir).unwrap();
                }
                _ => {
                    let old = root.join("Old Tasks");
                    std::fs::rename(&dir, &old).unwrap();
                    std::fs::create_dir(&dir).unwrap();
                    std::fs::write(&path, b"new directory").unwrap();
                    std::fs::remove_dir_all(old).unwrap();
                }
            }
            names.refresh(&dir).unwrap();
            let actual: HashSet<_> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|entry| fold(&entry.unwrap().file_name().to_string_lossy()))
                .collect();
            let cached: HashSet<_> = names.names.counts.keys().cloned().collect();
            assert_eq!(cached, actual, "operation seed {seed}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn a_full_kernel_queue_rebuilds_before_allocating() {
        let limit: usize = std::fs::read_to_string("/proc/sys/fs/inotify/max_queued_events")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        if limit > 32_768 {
            return;
        } // Bound this host-dependent stress case.
        let dir = crate::tests::tmp();
        let mut names = DirectoryNames::default();
        names.refresh(&dir).unwrap();
        if names.watch.is_none() {
            std::fs::remove_dir_all(dir).unwrap();
            return;
        }
        let churn = dir.join("Churn.md");
        for _ in 0..limit + 16 {
            std::fs::write(&churn, b"temporary").unwrap();
            std::fs::remove_file(&churn).unwrap();
        }
        // This CREATE is beyond the full queue. Only a fresh directory scan
        // can discover it; silently dropping the overflow would miss it.
        std::fs::write(dir.join("After overflow.md"), b"outside").unwrap();
        names.refresh(&dir).unwrap();
        assert!(names.taken("after overflow.md"));
        assert!(!names.taken("churn.md"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
