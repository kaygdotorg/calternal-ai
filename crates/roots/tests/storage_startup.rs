//! Startup refusal fixtures ported from Calternal ea57b84b storage_startup.rs.
#![cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
use calternal_fsio::DataLock;
use calternal_roots::{Roots, prepare, prepare_with_wait, re_pair};
use std::{io, path::Path, time::Duration};
use tempfile::TempDir;
use uuid::Uuid;

const MARKER: &str = ".calternal-install-id";
struct Fixture {
    temp: TempDir,
    roots: Roots,
    id: Uuid,
}
impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let roots = Roots {
            ssd_space: temp.path().join("ssd-space"),
            hdd_space: temp.path().join("hdd-space"),
            state: temp.path().join("state"),
        };
        Self {
            temp,
            roots,
            id: Uuid::new_v4(),
        }
    }
    fn setup(&self) {
        drop(prepare(&self.roots, self.id, true).unwrap());
        // A disposable nonempty sentinel tests only the inherited presence fence.
        // Opening/migrating SQLite belongs to the separate auth worker port.
        put(
            &self.roots.state.join("system/auth.db"),
            b"disposable auth fixture",
        );
    }
    fn starts(&self, create: bool) -> bool {
        prepare(&self.roots, self.id, create).is_ok()
    }
    fn paths(&self) -> [&Path; 3] {
        [
            &self.roots.ssd_space,
            &self.roots.hdd_space,
            &self.roots.state,
        ]
    }
    fn clear_locks(&self) {
        for root in self.paths() {
            std::fs::remove_file(root.join(".lock")).unwrap();
        }
    }
    fn no_locks(&self) {
        for root in self.paths() {
            assert!(!root.join(".lock").exists());
        }
    }
}
fn put(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

// Source :201, extended to three roles.
#[test]
fn missing_space_refuses_without_creating_directory_or_lock() {
    let mut f = Fixture::new();
    f.roots.ssd_space = f.temp.path().join("unmounted/space");
    assert!(!f.starts(false));
    assert!(!f.roots.ssd_space.parent().unwrap().exists());
    assert!(!f.roots.state.exists());
    f.setup();
    f.clear_locks();
    std::fs::rename(&f.roots.ssd_space, f.temp.path().join("offline-space")).unwrap();
    assert!(!f.starts(true));
    assert!(!f.roots.ssd_space.exists());
    std::fs::create_dir_all(&f.roots.ssd_space).unwrap();
    assert!(!f.starts(false));
    f.no_locks();
}

// Source :221: empty/missing state must remain refused, including --create.
#[test]
fn missing_state_never_recreates_databases_even_with_create() {
    let f = Fixture::new();
    f.setup();
    f.clear_locks();
    let note = f.roots.hdd_space.join("spaces/person/Notes/Existing.md");
    put(&note, b"# Existing\noriginal bytes\n");
    std::fs::rename(&f.roots.state, f.temp.path().join("offline-ssd")).unwrap();
    for create in [false, true] {
        assert!(!f.starts(create));
        assert!(!f.roots.state.exists());
    }
    std::fs::create_dir(&f.roots.state).unwrap();
    assert!(!f.starts(false));
    assert!(!f.roots.state.join("system").exists());
    f.no_locks();
    assert_eq!(
        std::fs::read(note).unwrap(),
        b"# Existing\noriginal bytes\n"
    );
    put(
        &f.roots.state.join(MARKER),
        format!("{}\napp-state\n", f.id).as_bytes(),
    );
    assert!(!f.starts(false));
    assert!(!f.roots.state.join("system").exists());
    f.no_locks();
    put(&f.roots.state.join("system/auth.db"), b"");
    assert!(!f.starts(true));
    f.no_locks();
}

// Source :248 pairing assertions; explicit repair assertions are separate below.
#[test]
fn roots_must_keep_their_persistent_partner() {
    let a = Fixture::new();
    a.setup();
    a.clear_locks();
    let b = Fixture::new();
    b.setup();
    b.clear_locks();
    let original = std::fs::read(a.roots.state.join(MARKER)).unwrap();
    let mut roots = a.roots.clone();
    roots.state = b.roots.state.clone();
    for create in [false, true] {
        assert!(prepare(&roots, a.id, create).is_err());
    }
    roots = a.roots.clone();
    roots.hdd_space = b.roots.hdd_space.clone();
    for create in [false, true] {
        assert!(prepare(&roots, a.id, create).is_err());
    }
    assert_eq!(original, std::fs::read(a.roots.state.join(MARKER)).unwrap());
    a.no_locks();
    b.no_locks();
    for root in a.paths() {
        let saved = std::fs::read(root.join(MARKER)).unwrap();
        std::fs::remove_file(root.join(MARKER)).unwrap();
        assert!(!a.starts(false));
        a.no_locks();
        put(&root.join(MARKER), b"broken");
        assert!(!a.starts(false));
        a.no_locks();
        put(&root.join(MARKER), &saved);
    }
    let marker = a.roots.state.join(MARKER);
    std::fs::remove_file(&marker).unwrap();
    assert!(re_pair(&a.roots, a.id, false).is_err());
    let held = DataLock::acquire_existing(&a.roots.ssd_space).unwrap();
    assert!(re_pair(&a.roots, a.id, true).is_err());
    drop(held);
    re_pair(&a.roots, a.id, true).unwrap();
    assert!(a.starts(false));
    std::fs::remove_file(a.roots.ssd_space.join(MARKER)).unwrap();
    assert!(!a.starts(false));
    re_pair(&a.roots, a.id, true).unwrap();
    put(&marker, b"broken");
    assert!(!a.starts(false));
    re_pair(&a.roots, a.id, true).unwrap();
    assert!(a.starts(false));
}

// Source :293 refusal half; the deliberately offline re-pair is covered separately.
#[test]
fn all_missing_markers_with_existing_data_refuse_setup() {
    let f = Fixture::new();
    f.setup();
    f.clear_locks();
    let note = f.roots.hdd_space.join("spaces/person/Notes/Existing.md");
    put(&note, b"# Existing\nkeep these bytes\n");
    let auth = std::fs::read(f.roots.state.join("system/auth.db")).unwrap();
    for root in f.paths() {
        std::fs::remove_file(root.join(MARKER)).unwrap();
    }
    for create in [false, true] {
        assert!(!f.starts(create));
        f.no_locks();
        for root in f.paths() {
            assert!(!root.join(MARKER).exists());
        }
        assert_eq!(
            std::fs::read(f.roots.state.join("system/auth.db")).unwrap(),
            auth
        );
        assert_eq!(
            std::fs::read(&note).unwrap(),
            b"# Existing\nkeep these bytes\n"
        );
    }
    re_pair(&f.roots, f.id, true).unwrap();
    assert!(f.starts(false));
}

// Source :321, including each new space role and app state.
#[test]
fn create_refuses_unpaired_notes_or_databases_before_creating_other_root() {
    for role in 0..3 {
        let f = Fixture::new();
        let relative = if role == 2 {
            "spaces/person/notes/history.db"
        } else {
            "spaces/person/Notes/Existing.md"
        };
        let file = f.paths()[role].join(relative);
        put(&file, b"original");
        assert!(!f.starts(true));
        f.no_locks();
        for (i, root) in f.paths().iter().enumerate() {
            assert!(!root.join(MARKER).exists());
            if i != role {
                assert!(!root.exists());
            }
        }
        assert_eq!(std::fs::read(file).unwrap(), b"original");
    }
}

// Source :346; no write attempts to /sys or any nondisposable path.
#[test]
fn legacy_history_and_invalid_state_are_refused() {
    for space in 0..2 {
        let f = Fixture::new();
        f.setup();
        f.clear_locks();
        let relative = Path::new("spaces/person/notes/history.db");
        put(&f.paths()[space].join(relative), b"history");
        assert!(!f.starts(false));
        f.no_locks();
        put(&f.roots.state.join(relative), b"history");
        assert!(f.starts(false));
    }
    let f = Fixture::new();
    f.setup();
    f.clear_locks();
    // Source uses read-only /sys. Use only a disposable directory here, as
    // required by the lane; run as the ordinary unprivileged development user.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&f.roots.state, std::fs::Permissions::from_mode(0o555)).unwrap();
    let refused = !f.starts(false);
    std::fs::set_permissions(&f.roots.state, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        refused,
        "a read-only state root must not start without a lock"
    );
    let file = f.temp.path().join("file");
    put(&file, b"x");
    let mut roots = f.roots.clone();
    roots.state = file;
    assert!(prepare(&roots, f.id, true).is_err());
}

// Source :522: every root must be held; any failed acquisition releases the others.
#[test]
fn all_roots_remain_exclusively_locked() {
    let f = Fixture::new();
    f.setup();
    let locks = prepare(&f.roots, f.id, false).unwrap();
    for root in f.paths() {
        assert!(DataLock::acquire_existing(root).is_err());
    }
    assert!(!f.starts(false));
    drop(locks);
    assert!(f.starts(false));
    for root in f.paths() {
        let held = DataLock::acquire_existing(root).unwrap();
        assert!(!f.starts(false));
        for other in f.paths().into_iter().filter(|p| *p != root) {
            assert!(
                DataLock::acquire_existing(other).is_ok(),
                "no partial guard leak"
            );
        }
        drop(held);
        assert!(f.starts(false));
    }
}

// Source :536, callback-based release without arbitrary real sleeps.
#[tokio::test]
async fn startup_waits_for_a_previous_server_to_release_all_roots() {
    use std::sync::Arc;
    use tokio::sync::Notify;
    let f = Fixture::new();
    f.setup();
    let held = prepare(&f.roots, f.id, false).unwrap();
    let waiting = Arc::new(Notify::new());
    let release_signal = waiting.clone();
    let release = tokio::spawn(async move {
        release_signal.notified().await;
        drop(held);
    });
    let acquired = prepare_with_wait(&f.roots, f.id, false, Duration::from_secs(2), move || {
        waiting.notify_one()
    })
    .await
    .unwrap();
    release.await.unwrap();
    for root in f.paths() {
        assert!(DataLock::acquire_existing(root).is_err());
    }
    drop(acquired);
}

// Source :579, library caller supplies the longest shutdown grace.
#[tokio::test(start_paused = true)]
async fn a_lock_held_past_the_grace_still_fails_startup() {
    let f = Fixture::new();
    f.setup();
    let live = prepare(&f.roots, f.id, false).unwrap();
    let grace = Duration::from_secs(30);
    let started = tokio::time::Instant::now();
    let error = prepare_with_wait(&f.roots, f.id, false, grace, || {})
        .await
        .err()
        .unwrap();
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(started.elapsed() >= grace);
    drop(live);
}

#[test]
fn swapped_roles_and_wrong_pinned_identity_refuse_before_lock_creation() {
    let f = Fixture::new();
    f.setup();
    f.clear_locks();
    let mut roots = f.roots.clone();
    std::mem::swap(&mut roots.ssd_space, &mut roots.hdd_space);
    for create in [false, true] {
        assert!(prepare(&roots, f.id, create).is_err());
        f.no_locks();
    }
    assert!(prepare(&f.roots, Uuid::new_v4(), false).is_err());
    f.no_locks();
}

#[test]
fn every_missing_role_or_marker_refuses_before_writes() {
    for role in 0..3 {
        for missing_marker in [false, true] {
            let f = Fixture::new();
            f.setup();
            f.clear_locks();
            let root = f.paths()[role];
            if missing_marker {
                std::fs::remove_file(root.join(MARKER)).unwrap();
            } else {
                std::fs::rename(root, f.temp.path().join("offline")).unwrap();
            }
            for create in [false, true] {
                assert!(!f.starts(create));
                f.no_locks();
                if !missing_marker {
                    assert!(!root.exists());
                }
            }
        }
    }
}

#[test]
fn symlink_substitutions_never_receive_markers_locks_or_directories() {
    use std::os::unix::fs::symlink;
    for role in 0..3 {
        let f = Fixture::new();
        f.setup();
        f.clear_locks();
        let root = f.paths()[role];
        let saved = f.temp.path().join("saved");
        std::fs::rename(root, &saved).unwrap();
        symlink(&saved, root).unwrap();
        for create in [false, true] {
            assert!(!f.starts(create));
            f.no_locks();
        }
        assert!(!saved.join(".lock").exists());
    }
    for relative in [MARKER, ".lock", "system", "system/auth.db"] {
        let f = Fixture::new();
        f.setup();
        f.clear_locks();
        let target = f.roots.state.join(relative);
        let outside = f.temp.path().join("outside");
        if target.is_dir() {
            std::fs::rename(&target, &outside).unwrap();
        } else {
            if target.exists() {
                std::fs::rename(&target, &outside).unwrap();
            } else {
                put(&outside, b"outside");
            }
        }
        symlink(&outside, &target).unwrap();
        for create in [false, true] {
            assert!(!f.starts(create));
        }
        for root in [&f.roots.ssd_space, &f.roots.hdd_space] {
            assert!(!root.join(".lock").exists());
        }
        if relative != ".lock" {
            assert!(!f.roots.state.join(".lock").exists());
        }
    }
    let f = Fixture::new();
    let parent = f.temp.path().join("parent");
    let alias = f.temp.path().join("alias");
    std::fs::create_dir(&parent).unwrap();
    symlink(&parent, &alias).unwrap();
    let mut roots = f.roots.clone();
    roots.hdd_space = alias.join("missing");
    assert!(prepare(&roots, f.id, true).is_err());
    assert!(!parent.join("missing").exists());
    assert!(!f.roots.ssd_space.exists());
}

#[test]
fn roles_cannot_overlap_and_first_setup_never_creates_a_database() {
    let f = Fixture::new();
    let mut roots = f.roots.clone();
    roots.state = roots.ssd_space.clone();
    assert!(prepare(&roots, f.id, true).is_err());
    roots.state = roots.ssd_space.join("state");
    assert!(prepare(&roots, f.id, true).is_err());
    assert!(!f.roots.ssd_space.exists());
    drop(prepare(&f.roots, f.id, true).unwrap());
    assert!(!f.roots.state.join("system").exists());
    assert!(!f.starts(false));
    assert!(!f.starts(true));
    for (role, root) in ["ssd-space", "hdd-space", "app-state"]
        .into_iter()
        .zip(f.paths())
    {
        assert_eq!(
            std::fs::read_to_string(root.join(MARKER)).unwrap(),
            format!("{}\n{role}\n", f.id)
        );
    }
}

#[test]
fn explicit_repair_cannot_bless_swapped_roles_missing_state_or_busy_roots() {
    let f = Fixture::new();
    f.setup();
    f.clear_locks();
    let mut swapped = f.roots.clone();
    std::mem::swap(&mut swapped.ssd_space, &mut swapped.hdd_space);
    assert!(re_pair(&swapped, f.id, true).is_err());
    f.no_locks();
    for root in f.paths() {
        let held = DataLock::acquire_existing(root).unwrap();
        assert!(re_pair(&f.roots, f.id, true).is_err());
        drop(held);
    }
    f.clear_locks();
    std::fs::remove_file(f.roots.state.join("system/auth.db")).unwrap();
    let before = f.paths().map(|p| std::fs::read(p.join(MARKER)).unwrap());
    assert!(re_pair(&f.roots, Uuid::new_v4(), true).is_err());
    f.no_locks();
    for (path, bytes) in f.paths().into_iter().zip(before) {
        assert_eq!(std::fs::read(path.join(MARKER)).unwrap(), bytes);
    }
}

#[test]
fn invalid_lock_entry_is_checked_on_every_role_before_any_lock_creation() {
    for role in 0..3 {
        let f = Fixture::new();
        f.setup();
        f.clear_locks();
        let lock = f.paths()[role].join(".lock");
        std::fs::create_dir(&lock).unwrap();
        for create in [false, true] {
            assert!(!f.starts(create));
        }
        for (i, root) in f.paths().into_iter().enumerate() {
            if i != role {
                assert!(!root.join(".lock").exists());
            }
        }
        assert!(lock.is_dir());
    }
}
