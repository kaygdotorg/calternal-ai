//! Incremental logical disk usage. File mutations refresh only their changed paths;
//! reconciliation walks without holding either the counter or authentication locks.
//!
//! This source-compatible counter keys `users/<id>` as a person. It is retained
//! for the filesystem port's mutation hooks and regression tests, not integrated
//! as the new application's space quota policy. Joined-tier accounting and
//! validated owning-space bindings belong to the later quota/storage port.
mod files;

use files::Files;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};

#[derive(Default)]
struct State {
    files: Files,
    totals: HashMap<String, u64>,
    generation: u64,
    directory_generation: u64,
    changes: HashMap<PathBuf, u64>,
}
/// A counter for the person Homes beneath a data root. With a separate
/// state root, what plugins report for a person's app data there (see
/// [`provide`]) counts too.
pub struct Usage {
    root: PathBuf,
    /// `<state>/users`, when it is not the data root.
    state_users: Option<PathBuf>,
    state: Mutex<State>,
    totals: arc_swap::ArcSwap<HashMap<String, u64>>,
    reconcile: Mutex<()>,
    ready: std::sync::atomic::AtomicBool,
}

/// What a plugin keeps for one person on the state root, in bytes, given
/// that person's folder there (`<state>/users/<id>`).
pub type Provider = Arc<dyn Fn(&Path) -> u64 + Send + Sync>;

fn providers() -> &'static Mutex<Vec<(String, Provider)>> {
    static PROVIDERS: OnceLock<Mutex<Vec<(String, Provider)>>> = OnceLock::new();
    PROVIDERS.get_or_init(Default::default)
}

/// Lets plugin `name` report what it keeps for a person on the state root
/// (the contacts plugin: every version of their cards, photos included),
/// so storage limits count it although no file there has that size. A
/// second call with the same name replaces the first.
pub fn provide(name: &str, provider: Provider) {
    let mut all = providers().lock().unwrap_or_else(|p| p.into_inner());
    all.retain(|(n, _)| n != name);
    all.push((name.to_owned(), provider));
}
static COUNTERS: OnceLock<Mutex<Vec<Weak<Usage>>>> = OnceLock::new();
impl Usage {
    /// Registers a counter. Recount existing files before admitting an upload.
    pub fn new(data: &Path) -> Arc<Self> {
        Self::with_state(data, data)
    }

    /// A counter for the Homes under `data` that also counts what plugins
    /// report (see [`provide`]) for each person's app data under `state`.
    /// On a single root the files themselves are counted instead.
    pub fn with_state(data: &Path, state: &Path) -> Arc<Self> {
        let root = data.join("users");
        let state_users = Some(state.join("users")).filter(|s| *s != root);
        let counter = Arc::new(Self {
            root,
            state_users,
            state: Mutex::new(State::default()),
            totals: arc_swap::ArcSwap::from_pointee(HashMap::new()),
            reconcile: Mutex::new(()),
            ready: std::sync::atomic::AtomicBool::new(false),
        });
        let mut counters = COUNTERS
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        counters.retain(|c| c.strong_count() > 0);
        counters.push(Arc::downgrade(&counter));
        counter
    }
    /// Initializes existing usage outside request authentication locks.
    pub fn ensure(&self) -> std::io::Result<()> {
        while !self.ready.load(std::sync::atomic::Ordering::Acquire) {
            self.recount()?;
        }
        Ok(())
    }
    /// Runs drift reconciliation in the background for this counter's lifetime.
    pub fn background(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            while let Some(counter) = weak.upgrade() {
                let _ = counter.recount();
                drop(counter);
                std::thread::sleep(std::time::Duration::from_secs(60));
            }
        });
    }
    fn person(&self, path: &Path) -> Option<String> {
        let rel = path.strip_prefix(&self.root).ok()?;
        let mut parts = rel.components();
        let person = parts.next()?.as_os_str().to_str()?.to_owned();
        if parts.any(|p| {
            p.as_os_str()
                .to_string_lossy()
                .starts_with(crate::TEMP_PREFIX)
        }) {
            return None;
        }
        Some(person)
    }
    /// The person's files, from the maintained snapshot without locking or
    /// touching the disk, plus what plugins report for their app data on a
    /// separate state root (a small read each).
    pub fn bytes(&self, person: &str) -> u64 {
        let files = self.totals.load().get(person).copied().unwrap_or(0);
        let Some(users) = &self.state_users else {
            return files;
        };
        let dir = users.join(person);
        let all: Vec<Provider> = providers()
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|(_, p)| p.clone())
            .collect();
        files + all.iter().map(|p| p(&dir)).sum::<u64>()
    }
    fn publish(&self, state: &State) {
        self.totals.store(Arc::new(state.totals.clone()));
    }
    fn refresh(&self, path: &Path) {
        self.refresh_using(path, || {
            std::fs::symlink_metadata(path)
                .ok()
                .filter(|m| m.is_file())
                .map_or(0, |m| m.len())
        });
    }
    fn refresh_using(&self, path: &Path, observe: impl Fn() -> u64) {
        let Some(person) = self.person(path) else {
            return;
        };
        loop {
            let generation = self
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .generation;
            let size = observe();
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // A newer notification must never be replaced by an older stat.
            // Retry outside the lock, so cached reads never wait for disk IO.
            if state.generation != generation {
                continue;
            }
            let old = if size == 0 {
                state.files.remove(path).unwrap_or(0)
            } else {
                state.files.insert(path, size).unwrap_or(0)
            };
            let total = state.totals.entry(person.clone()).or_default();
            *total = total.saturating_sub(old).saturating_add(size);
            state.generation = state.generation.wrapping_add(1);
            let version = state.generation;
            state.changes.insert(path.to_owned(), version);
            self.publish(&state);
            break;
        }
    }

    /// Repairs outside changes and merges application writes made during the walk.
    pub fn recount(&self) -> std::io::Result<()> {
        self.recount_using(|| {})
    }

    /// Pauses the first completed walk before publication so integration tests can
    /// change the tree deterministically. Available only with the test-hooks feature;
    /// retries use the normal recount path without calling the hook again.
    #[cfg(feature = "test-hooks")]
    pub fn recount_with_after_walk(&self, after_walk: impl FnOnce()) -> std::io::Result<()> {
        self.recount_using(after_walk)
    }

    /// Publishes a walked snapshot only after accounting for concurrent file and directory changes.
    fn recount_using(&self, after_walk: impl FnOnce()) -> std::io::Result<()> {
        let _reconcile = self.reconcile.lock().unwrap_or_else(|p| p.into_inner());
        let mut after_walk = Some(after_walk);
        loop {
            let directory_generation = self
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .directory_generation;
            let generation = self
                .state
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .generation;
            let mut files = Files::default();
            fn walk(
                folder: &crate::beneath::Folder,
                path: &Path,
                files: &mut Files,
                depth: usize,
            ) -> std::io::Result<()> {
                if depth > 64 {
                    return Err(std::io::Error::other("Home is nested too deeply"));
                }
                for name in folder.names()? {
                    if name.starts_with(crate::TEMP_PREFIX) {
                        continue;
                    }
                    if let Ok(file) = folder.open_file(&name) {
                        files.insert_in(path, std::ffi::OsStr::new(&name), file.metadata()?.len());
                    } else if let Ok(child) = folder.open_sub(&name, false) {
                        walk(&child, &path.join(&name), files, depth + 1)?;
                    }
                }
                Ok(())
            }
            if self.root.try_exists()? {
                let root = crate::beneath::Folder::open(&self.root)?;
                walk(&root, &self.root, &mut files, 0)?;
            }
            let mut totals = files.totals(|path| self.person(path));
            if let Some(after_walk) = after_walk.take() {
                after_walk();
            }
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            // A directory handle stays valid after rename, but the paths built
            // during that walk do not. Retry before publishing any such snapshot.
            if state.directory_generation != directory_generation {
                drop(state);
                continue;
            }
            for (path, version) in &state.changes {
                if *version <= generation {
                    continue;
                }
                let old = files.remove(path).unwrap_or(0);
                let current = state.files.get(path).unwrap_or(0);
                if current > 0 {
                    files.insert(path, current);
                }
                if let Some(person) = self.person(path) {
                    let total = totals.entry(person).or_default();
                    *total = total.saturating_sub(old).saturating_add(current);
                }
            }
            let old_files = std::mem::replace(&mut state.files, files);
            state.totals = totals;
            state.changes.clear();
            state.generation = state.generation.wrapping_add(1);
            self.publish(&state);
            self.ready.store(true, std::sync::atomic::Ordering::Release);
            drop(state);
            drop(old_files);
            return Ok(());
        }
    }
}

/// Reports a changed path to registered counters. No directory traversal occurs.
pub(crate) fn changed(path: &Path) {
    let counters = COUNTERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    for counter in counters {
        counter.refresh(path);
    }
}

/// Reports files this process just wrote whole, with their known sizes, so a
/// bulk write of thousands of files costs no extra `stat` per file.
pub(crate) fn wrote<'a>(files: impl Iterator<Item = (&'a Path, u64)>) {
    let counters = COUNTERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    if counters.is_empty() {
        return;
    }
    for (path, size) in files {
        for counter in &counters {
            counter.refresh_using(path, || size);
        }
    }
}

/// Reports a successful move, retaining accounting for every child of a folder.
pub(crate) fn moved(from: &Path, to: &Path) {
    let counters = COUNTERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .filter_map(Weak::upgrade)
        .collect::<Vec<_>>();
    for counter in counters {
        if counter.person(from).is_none() && counter.person(to).is_none() {
            continue;
        }
        let directory = to.is_dir();
        let mut state = counter.state.lock().unwrap_or_else(|p| p.into_inner());
        if directory {
            state.directory_generation = state.directory_generation.wrapping_add(1);
        }
        let moving = if directory {
            state.files.under(from)
        } else {
            vec![from.to_owned()]
        };
        for old in moving {
            let Some(size) = state.files.remove(&old) else {
                continue;
            };
            let version = state.generation.wrapping_add(1);
            state.changes.insert(old.clone(), version);
            if let Some(person) = counter.person(&old) {
                let total = state.totals.entry(person).or_default();
                *total = total.saturating_sub(size);
            }
            let suffix = old.strip_prefix(from).unwrap();
            let new = if suffix.as_os_str().is_empty() {
                to.to_owned()
            } else {
                to.join(suffix)
            };
            state.changes.insert(new.clone(), version);
            if let Some(person) = counter.person(&new) {
                // The destination can already be counted: a walk that ran while
                // the file was being renamed may have listed both names. Its
                // old size leaves the total, or the file counts twice until the
                // next recount.
                let replaced = state.files.insert(&new, size).unwrap_or(0);
                let total = state.totals.entry(person).or_default();
                *total = total.saturating_sub(replaced).saturating_add(size);
            }
        }
        state.generation = state.generation.wrapping_add(1);
        counter.publish(&state);
        drop(state);
        counter.refresh(from);
        counter.refresh(to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_moves_trash_and_purge_update_without_a_recount() {
        let root = std::env::temp_dir().join(format!(
            "calternal-usage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let home = root.join("users/person");
        std::fs::create_dir_all(home.join("Notes")).unwrap();
        std::fs::create_dir_all(home.join("Trash")).unwrap();
        let usage = Usage::new(&root);
        usage.ensure().unwrap();
        let note = home.join("Notes/a.md");
        crate::write_atomic(&note, b"one").unwrap();
        assert_eq!(usage.bytes("person"), 3);
        crate::write_unless_changed(&note, b"second", |_| true).unwrap();
        assert_eq!(usage.bytes("person"), 6);
        crate::rename_safely(&note, &home.join("Trash/a.md")).unwrap();
        assert_eq!(usage.bytes("person"), 6, "trash still consumes disk");
        crate::remove_file(home.join("Trash/a.md")).unwrap();
        assert_eq!(usage.bytes("person"), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn stale_metadata_cannot_replace_a_newer_update() {
        let root =
            std::env::temp_dir().join(format!("calternal-usage-race-{}", std::process::id()));
        let dir = root.join("users/person");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.md");
        let usage = Usage::new(&root);
        usage.ensure().unwrap();
        crate::write_atomic(&path, b"old").unwrap();
        let (observed_tx, observed_rx) = std::sync::mpsc::channel();
        let (continue_tx, continue_rx) = std::sync::mpsc::channel();
        let other = usage.clone();
        let other_path = path.clone();
        let worker = std::thread::spawn(move || {
            let first = std::sync::atomic::AtomicBool::new(true);
            other.refresh_using(&other_path, || {
                let size = std::fs::metadata(&other_path).unwrap().len();
                if first.swap(false, std::sync::atomic::Ordering::Relaxed) {
                    observed_tx.send(()).unwrap();
                    continue_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                }
                size
            });
        });
        observed_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        crate::write_atomic(&path, b"newest").unwrap();
        continue_tx.send(()).unwrap();
        worker.join().unwrap();
        assert_eq!(usage.bytes("person"), 6);
        std::fs::remove_dir_all(root).unwrap();
    }

    /// A recount that walks a folder while a file is renamed in it can list both
    /// names (a directory read is not a snapshot) and publish the file twice. The
    /// move that follows must leave it counted once (#1119).
    #[test]
    fn a_move_onto_a_name_the_walk_already_counted_counts_the_file_once() {
        let root =
            std::env::temp_dir().join(format!("calternal-usage-both-{}", std::process::id()));
        let dir = root.join("users/person");
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("from"), dir.join("moved"));
        let usage = Usage::new(&root);
        usage.ensure().unwrap();
        crate::write_atomic(&from, &[0; 137]).unwrap();
        {
            // What the half-seen rename published: both names, each counted.
            let mut state = usage.state.lock().unwrap();
            state.files.insert(&to, 137);
            *state.totals.entry("person".into()).or_default() += 137;
            usage.publish(&state);
        }
        assert_eq!(usage.bytes("person"), 274);
        crate::rename_safely(&from, &to).unwrap();
        assert_eq!(usage.bytes("person"), 137);
        usage.recount().unwrap();
        assert_eq!(usage.bytes("person"), 137, "the recount agrees");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recount_preserves_concurrent_writes_and_deletes() {
        let root =
            std::env::temp_dir().join(format!("calternal-usage-recount-{}", std::process::id()));
        let dir = root.join("users/person");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.md");
        let gone = dir.join("gone.md");
        crate::write_atomic(&path, b"old").unwrap();
        crate::write_atomic(&gone, b"delete").unwrap();
        let usage = Usage::new(&root);
        usage
            .recount_using(|| {
                crate::write_atomic(&path, b"newest").unwrap();
                crate::remove_file(&gone).unwrap();
            })
            .unwrap();
        assert_eq!(usage.bytes("person"), 6);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bulk_writes_and_published_folders_count_without_a_recount() {
        let root =
            std::env::temp_dir().join(format!("calternal-usage-bulk-{}", std::process::id()));
        let notes = root.join("users/person/Notes");
        std::fs::create_dir_all(notes.join("Old")).unwrap();
        let usage = Usage::new(&root);
        usage.ensure().unwrap();
        // Many files take the parallel path; one is refused as taken.
        crate::write_atomic(&notes.join("Old/0.md"), b"keep").unwrap();
        let files: Vec<_> = (0..100)
            .map(|i| (notes.join(format!("Old/{i}.md")), vec![b'x'; 10]))
            .collect();
        let placed = crate::write_new_many(&files).unwrap();
        assert!(placed[0].is_err());
        assert_eq!(usage.bytes("person"), 4 + 99 * 10);
        #[cfg(target_os = "linux")]
        {
            let image: Vec<_> = (0..70)
                .map(|i| (notes.join(format!("New/{i}.md")), vec![b'y'; 3]))
                .collect();
            let cache = root.join("private");
            crate::prepare_new_directory(image, &cache, &"e".repeat(64))
                .unwrap()
                .expect("same filesystem and policy")
                .publish(|| {})
                .unwrap()
                .synced
                .unwrap();
            assert_eq!(usage.bytes("person"), 4 + 99 * 10 + 70 * 3);
            usage.recount().unwrap();
            assert_eq!(
                usage.bytes("person"),
                4 + 99 * 10 + 70 * 3,
                "recount agrees"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn a_separate_state_root_counts_what_plugins_report() {
        let root =
            std::env::temp_dir().join(format!("calternal-usage-provide-{}", std::process::id()));
        let (data, state) = (root.join("data"), root.join("state"));
        std::fs::create_dir_all(data.join("users/person")).unwrap();
        crate::write_atomic(&data.join("users/person/a.md"), b"abc").unwrap();
        provide(
            "usage-test",
            Arc::new(|dir: &Path| {
                if dir.ends_with("users/person") {
                    1000
                } else {
                    0
                }
            }),
        );
        let split = Usage::with_state(&data, &state);
        split.ensure().unwrap();
        assert_eq!(split.bytes("person"), 1003);
        // One root: its files are counted as they are, not reported twice.
        let single = Usage::new(&data);
        single.ensure().unwrap();
        assert_eq!(single.bytes("person"), 3);
        provide("usage-test", Arc::new(|_: &Path| 0));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_do_not_wait_for_the_counter_writer() {
        let usage = Usage::new(Path::new("/unused-usage-read-test"));
        let guard = usage.state.lock().unwrap();
        let counter = usage.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || tx.send(counter.bytes("person")).unwrap());
        let result = rx.recv_timeout(std::time::Duration::from_millis(50));
        drop(guard);
        reader.join().unwrap();
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn folder_move_during_recount_retries_without_old_paths() {
        let root = std::env::temp_dir().join(format!(
            "calternal-usage-folder-race-{}",
            std::process::id()
        ));
        let old = root.join("users/alice/Old");
        let new = root.join("users/bob/New");
        std::fs::create_dir_all(old.join("sub")).unwrap();
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        crate::write_atomic(&old.join("a.md"), b"abc").unwrap();
        crate::write_atomic(&old.join("sub/b.md"), b"12345").unwrap();
        let usage = Usage::new(&root);
        usage.ensure().unwrap();
        usage
            .recount_using(|| crate::rename_safely(&old, &new).unwrap())
            .unwrap();
        assert_eq!(usage.bytes("alice"), 0);
        assert_eq!(usage.bytes("bob"), 8);
        {
            let state = usage.state.lock().unwrap();
            assert!(state.files.under(&old).is_empty());
            assert_eq!(state.files.get(&new.join("sub/b.md")), Some(5));
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
