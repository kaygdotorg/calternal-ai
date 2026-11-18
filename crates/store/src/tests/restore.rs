//! The restore drill (#918): every way the journal goes back in time must
//! end with devices re-reading, and no honest token may ever cause it.

use super::*;
use crate::ErrorCode;
use std::os::unix::fs::FileExt;
use std::path::Path;

#[path = "restore_failures.rs"]
mod failures;

/// A data folder (`spaces/<space-id>`) and a secrets root beside it, like a server.
struct Site {
    _root: TempDir,
    data: PathBuf,
    secrets: PathBuf,
}

impl Site {
    fn new() -> Self {
        let root = TempDir::new();
        let data = root.path().join("data").join("u1");
        let secrets = root.path().join("secrets");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&secrets).unwrap();
        Self {
            _root: root,
            data,
            secrets,
        }
    }

    async fn open(&self) -> SpaceStore {
        SpaceStore::open_dir_secured(&self.data, Options::default(), Some(&self.secrets))
            .await
            .unwrap()
    }

    fn head_file(&self) -> PathBuf {
        self.secrets
            .join("spaces")
            .join(test_space().to_string())
            .join("head")
    }

    fn secrets_user(&self) -> PathBuf {
        self.secrets.join("spaces").join(test_space().to_string())
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let _ = std::fs::remove_dir_all(to);
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

/// Lets go of the store and waits until its writer closed the database.
async fn stop(s: SpaceStore, site: &Site) {
    drop(s);
    for _ in 0..200 {
        if !site.data.join("index.db-wal").exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the writer did not close the database");
}

/// Appends test events inside the same activity scope used by engine saves.
async fn put(s: &SpaceStore, tag: &str, n: usize) -> Vec<JournalEntry> {
    let mut out = Vec::new();
    for i in 0..n {
        let _save_activity = s.begin_save_activity();
        let e = s
            .commit(Commit {
                events: vec![ev("t.a", &format!("{tag}{i}"))],
                idempotency: None,
                durable: false,
            })
            .await
            .unwrap();
        out.extend(e);
    }
    out
}

fn epoch(s: &SpaceStore) -> String {
    s.head().epoch
}

fn restored(s: &SpaceStore) -> u64 {
    *s.restored().borrow()
}

async fn expired(s: &SpaceStore, token: &str) -> String {
    let e = s.position(token).await.unwrap_err();
    assert_eq!(e.code, ErrorCode::SyncTokenExpired, "{token}");
    e.to_string()
}

/// A token with a good tag for any epoch, seq and check, as only the
/// server's key can make.
fn token(s: &SpaceStore, epoch: &str, seq: i64, check: u32) -> String {
    s.inner.shared.sign(epoch, seq, check)
}

#[tokio::test]
async fn signed_tokens_are_issued_everywhere_and_accepted() {
    let site = Site::new();
    let s = site.open().await;
    let a = put(&s, "a", 3).await;
    for e in &a {
        assert_eq!(e.token.split('-').count(), 4, "{}", e.token);
        assert_eq!(s.position(&e.token).await.unwrap(), e.seq);
    }
    assert_eq!(s.since(0, 10).await.unwrap(), a);
    assert_eq!(s.token_at(3).await.unwrap(), a[2].token);
    assert_eq!(s.token_at(2).await.unwrap(), a[1].token);
    let zero = s.token_at(0).await.unwrap();
    assert_eq!(zero.split('-').count(), 4, "zero cursors are signed too");
    assert_eq!(s.position(&zero).await.unwrap(), 0);
    // A token from before signing is accepted while it is not past the head.
    assert_eq!(s.position(&format!("{}-2", epoch(&s))).await.unwrap(), 2);
    assert_eq!(s.position(&format!("{}-0", epoch(&s))).await.unwrap(), 0);
    expired(&s, &format!("{}-99", epoch(&s))).await;
    assert_eq!(
        restored(&s),
        0,
        "an unsigned token past the head proves nothing"
    );
}

/// Repeated catch-up reuses tokens for an older page that collided with newer cursors.
#[tokio::test]
async fn catch_up_reuses_recent_tokens_with_bounded_cache() {
    let site = Site::new();
    let s = site.open().await;
    let written = put(&s, "cache", 256).await;
    let before = s.inner.shared.mint_calls();

    let first = s.since(0, 256).await.unwrap();
    assert_eq!(first, written);
    assert_eq!(
        s.inner.shared.mint_calls() - before,
        128,
        "the 128 older tokens miss while the bounded recent window is reused"
    );

    let after_first = s.inner.shared.mint_calls();
    let recent = s.since(128, 128).await.unwrap();
    assert_eq!(recent, written[128..]);
    assert_eq!(
        s.inner.shared.mint_calls(),
        after_first,
        "a repeated read of the recent window does not sign its entries again"
    );

    let old_page = s.since(0, 100).await.unwrap();
    assert_eq!(old_page, written[..100]);
    let after_old_page = s.inner.shared.mint_calls();
    let old_page_again = s.since(0, 100).await.unwrap();
    assert_eq!(old_page_again, written[..100]);
    assert_eq!(
        s.inner.shared.mint_calls(),
        after_old_page,
        "a repeated older page does not re-sign entries that collide with newer cursors"
    );
}

#[tokio::test]
async fn a_garbled_or_hand_edited_token_is_invalid_and_never_rotates() {
    let site = Site::new();
    let s = site.open().await;
    let a = put(&s, "a", 3).await;
    let e0 = epoch(&s);
    let good = a[2].token.clone();
    let flip = |t: &str, at: usize| {
        let mut b = t.as_bytes().to_vec();
        b[at] = if b[at] == b'0' { b'1' } else { b'0' };
        String::from_utf8(b).unwrap()
    };
    let bad_tag = flip(&good, good.len() - 1);
    let bad_check = flip(&good, good.len() - 10);
    let bad_seq = good.replacen("-3-", "-2-", 1);
    let past_head = good.replacen("-3-", "-30-", 1);
    for t in [bad_tag, bad_check, bad_seq, past_head] {
        assert!(expired(&s, &t).await.contains("invalid token"), "{t}");
    }
    assert_eq!(epoch(&s), e0);
    assert_eq!(restored(&s), 0);
    assert_eq!(s.position(&good).await.unwrap(), 3);
}

#[tokio::test]
async fn a_token_from_another_epoch_is_expired_not_a_rotation() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    let e0 = epoch(&s);
    let old = token(&s, "abc123abc123", 2, 7);
    let why = expired(&s, &old).await;
    assert!(why.contains("other epoch"), "{why}");
    assert_eq!(epoch(&s), e0);
    assert_eq!(restored(&s), 0);
}

#[tokio::test]
async fn first_deploy_rotates_nobody() {
    let site = Site::new();
    // Before this feature: no secrets root.
    let plain = SpaceStore::open_dir(&site.data).await.unwrap();
    let a = put(&plain, "a", 3).await;
    let e0 = epoch(&plain);
    assert_eq!(a[2].token, format!("{e0}-3"));
    stop(plain, &site).await;

    let s = site.open().await;
    assert_eq!(epoch(&s), e0, "nobody rotates");
    assert_eq!(restored(&s), 0);
    assert_eq!(
        s.position(&a[2].token).await.unwrap(),
        3,
        "old token still works"
    );
    assert!(site.head_file().exists(), "the file is made");
    let more = put(&s, "b", 1).await;
    assert_eq!(
        more[0].token.split('-').count(),
        4,
        "new replies are signed"
    );
}

/// Reopening an unchanged store must reuse its durable witness instead of
/// fsyncing and replacing the same high-water file on every start.
#[tokio::test]
async fn opening_with_a_current_witness_does_not_replace_it() {
    use std::os::unix::fs::MetadataExt;

    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 1).await;
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;

    let inode = std::fs::metadata(site.head_file()).unwrap().ino();
    let s = site.open().await;
    assert_eq!(restored(&s), 0);
    assert_eq!(std::fs::metadata(site.head_file()).unwrap().ino(), inode);
    stop(s, &site).await;
}

#[tokio::test]
async fn a_backup_taken_after_the_last_start_is_found() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 3).await;
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;

    // Monday's start is over. Tuesday: back up the data folder only.
    let backup = site.data.with_file_name("backup");
    copy_dir(&site.data, &backup);

    // Devices move ahead on Wednesday...
    let s = site.open().await;
    let ahead = put(&s, "b", 2).await;
    let e0 = epoch(&s);
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;

    // ...and on Thursday the backup comes back. The secrets do not.
    copy_dir(&backup, &site.data);
    let s = site.open().await;
    assert_eq!(
        restored(&s),
        1,
        "plugins hear it before their workers start"
    );
    assert_ne!(epoch(&s), e0);
    assert_eq!(s.head().seq, 3);
    let why = expired(&s, &ahead[1].token).await;
    assert!(why.contains("other epoch"), "{why}");
    assert_eq!(restored(&s), 1, "reading again is not another rotation");
    assert_eq!(s.epoch_head(), 3);
}

#[tokio::test]
async fn a_backup_from_before_an_earlier_rotation_is_found() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    stop(s, &site).await;
    let backup = site.data.with_file_name("backup");
    copy_dir(&site.data, &backup);

    // A rotation happens (any proof of a rollback will do).
    let s = site.open().await;
    let e1 = epoch(&s);
    expired(&s, &token(&s, &e1, 99, 5)).await;
    let e2 = epoch(&s);
    assert_ne!(e1, e2);
    stop(s, &site).await;

    // The backup predates it: same seqs, an epoch the server has retired.
    copy_dir(&backup, &site.data);
    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    let e3 = epoch(&s);
    assert!(e3 != e1 && e3 != e2);
}

#[tokio::test]
async fn a_host_snapshot_rollback_is_found_by_the_first_newer_token() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 3).await;
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;
    // Snapshot the whole host: data and secrets.
    let (snap_d, snap_s) = (
        site.data.with_file_name("snap-d"),
        site.secrets.with_file_name("snap-s"),
    );
    copy_dir(&site.data, &snap_d);
    copy_dir(&site.secrets, &snap_s);

    let s = site.open().await;
    let device = put(&s, "lost", 2).await; // seqs 4, 5
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;

    copy_dir(&snap_d, &site.data);
    copy_dir(&snap_s, &site.secrets);
    let s = site.open().await;
    assert_eq!(
        restored(&s),
        0,
        "the file rolled back too: nothing to see at open"
    );
    let e0 = epoch(&s);

    // The device's token points past the head: proof.
    let why = expired(&s, &device[1].token).await;
    assert!(why.contains("went back in time"), "{why}");
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), e0);
}

#[tokio::test]
async fn a_reused_seq_with_a_different_entry_is_found() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 3).await;
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;
    let (snap_d, snap_s) = (
        site.data.with_file_name("snap-d"),
        site.secrets.with_file_name("snap-s"),
    );
    copy_dir(&site.data, &snap_d);
    copy_dir(&site.secrets, &snap_s);

    let s = site.open().await;
    let lost = put(&s, "lost", 2).await; // seqs 4, 5
    stop(s, &site).await;

    copy_dir(&snap_d, &site.data);
    copy_dir(&snap_s, &site.secrets);
    let s = site.open().await;
    // The server writes past the device's seq with *different* entries.
    put(&s, "other", 3).await; // seqs 4, 5, 6
    assert_eq!(s.head().seq, 6);
    let e0 = epoch(&s);
    // Without the check, the device (at 5) would skip entry 6 forever.
    let why = expired(&s, &lost[1].token).await;
    assert!(why.contains("went back in time"), "{why}");
    assert_ne!(epoch(&s), e0);
    assert_eq!(restored(&s), 1);
}

#[tokio::test]
async fn a_lost_log_tail_is_found_at_open() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 3).await;
    // Power-loss stand-in: the database files as they were durable (with
    // their log), while the high-water file, fsynced on its own, kept up.
    let live = site.data.with_file_name("live");
    copy_dir(&site.data, &live);
    let tail = put(&s, "b", 2).await;
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;
    copy_dir(&live, &site.data);

    let s = site.open().await;
    assert_eq!(s.head().seq, 3, "the tail is gone");
    assert_eq!(restored(&s), 1);
    expired(&s, &tail[1].token).await;
    // The reused seqs get new tokens in the new epoch.
    let again = put(&s, "c", 2).await;
    assert_eq!(s.position(&again[1].token).await.unwrap(), 5);
}

#[tokio::test]
async fn a_zero_length_or_torn_highwater_file_counts_as_missing() {
    for contents in [
        &b""[..],
        &b"calternal-head-1 abc"[..],
        &b"calternal-head-1 abc 3 0000"[..],
    ] {
        let site = Site::new();
        let s = site.open().await;
        put(&s, "a", 2).await;
        let e0 = epoch(&s);
        stop(s, &site).await;
        calternal_fsio::write_atomic(&site.head_file(), contents).unwrap();

        // Missing, and the flag says we had one: restored.
        let s = site.open().await;
        assert_eq!(restored(&s), 1, "{contents:?}");
        assert_ne!(epoch(&s), e0);
        // The file is whole again.
        assert!(crate::highwater::read(&site.head_file()).is_some());
    }
}

/// Two invalid slots are unknown state and trigger the existing safe restore check.
#[tokio::test]
async fn both_invalid_highwater_slots_trigger_the_safe_restore_check() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    let old_epoch = epoch(&s);
    stop(s, &site).await;
    assert_eq!(std::fs::metadata(site.head_file()).unwrap().len(), 4096);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(site.head_file())
        .unwrap();
    file.write_at(&[0u8; 4096], 0).unwrap();
    file.sync_data().unwrap();

    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), old_epoch);
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap().epoch,
        epoch(&s)
    );
}

#[tokio::test]
async fn an_ahead_file_from_the_same_epoch_is_a_restore() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    stop(s, &site).await;
    let e0 = {
        let m = crate::highwater::read(&site.head_file()).unwrap();
        crate::highwater::write(
            &site.head_file(),
            &crate::highwater::Mark {
                seq: 9,
                ..m.clone()
            },
        )
        .unwrap();
        m.epoch
    };
    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), e0);
}

#[tokio::test]
async fn rotation_is_durable_before_new_tokens_and_survives_power_loss() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    let e0 = epoch(&s);
    expired(&s, &token(&s, &e0, 50, 1)).await;
    let e1 = epoch(&s);
    assert_ne!(e0, e1);

    // Look at the files the way a restart would, without closing anything.
    let conn = rusqlite::Connection::open_with_flags(
        site.data.join("index.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let in_db: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'epoch'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(
        in_db, e1,
        "the database already names the epoch we hand out"
    );
    assert_eq!(crate::highwater::read(&site.head_file()).unwrap().epoch, e1);
    drop(conn);

    // The first token of the new epoch is valid after a restart.
    let t = put(&s, "b", 1).await;
    stop(s, &site).await;
    let s = site.open().await;
    assert_eq!(epoch(&s), e1);
    assert_eq!(restored(&s), 0, "no second rotation");
    assert_eq!(s.position(&t[0].token).await.unwrap(), t[0].seq);
}

#[tokio::test]
async fn two_proofs_at_once_rotate_once() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    let e0 = epoch(&s);
    let (a, b) = (token(&s, &e0, 70, 1), token(&s, &e0, 71, 2));
    let (ra, rb) = tokio::join!(s.position(&a), s.position(&b));
    assert!(ra.is_err() && rb.is_err());
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), e0);
}

/// Waits for the idle threshold instead of the removed fixed flush interval.
#[tokio::test]
async fn the_witness_does_not_flush_on_the_old_fixed_interval() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_millis(100),
            highwater_max_staleness: Duration::from_secs(2),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();
    let a = put(&s, "a", 4).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let m = crate::highwater::read(&site.head_file()).unwrap();
    assert_eq!((m.seq, m.epoch), (0, epoch(&s)));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let m = crate::highwater::read(&site.head_file()).unwrap();
    assert_eq!((m.seq, m.epoch), (4, epoch(&s)));
    assert_eq!(
        a[3].token.split('-').nth(2).unwrap(),
        format!("{:08x}", m.check)
    );
}

/// A clean shutdown waits for queued writes and leaves their final head durable.
#[tokio::test]
async fn clean_shutdown_flushes_the_latest_head() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_secs(60),
            highwater_max_staleness: Duration::from_secs(120),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();
    let entries = put(&s, "shutdown", 3).await;
    let head = s.head().seq;
    let (_, proof) = crate::SyncToken::parse_signed(&entries.last().unwrap().token).unwrap();
    let check = proof.unwrap().check;

    stop(s, &site).await;

    let mark = crate::highwater::read(&site.head_file()).unwrap();
    assert_eq!((mark.seq, mark.check), (head, check));
}

/// A caller can hold activity across file work that happens before a commit.
#[tokio::test]
async fn save_activity_stays_active_until_the_full_operation_returns() {
    let site = Site::new();
    let s = site.open().await;

    let activity = s.begin_save_activity().unwrap();
    assert_eq!(s.inner.shared.flush_activity.snapshot().active_saves, 1);
    drop(activity);
    assert_eq!(s.inner.shared.flush_activity.snapshot().active_saves, 0);
}

/// The owned guard counts a save in flight exactly like the borrowed one.
#[tokio::test]
async fn owned_save_activity_counts_until_dropped() {
    let site = Site::new();
    let s = site.open().await;
    let owned = s.begin_owned_save_activity().unwrap();
    let both = s.begin_save_activity().unwrap();
    assert_eq!(s.inner.shared.flush_activity.snapshot().active_saves, 2);
    drop(owned);
    assert_eq!(s.inner.shared.flush_activity.snapshot().active_saves, 1);
    drop(both);
    let idle = s.inner.shared.flush_activity.snapshot();
    assert_eq!(idle.active_saves, 0);
    assert_ne!(idle.dirty_since_ns, 0);
}

/// A save scope marks the witness dirty outside the journal writer thread.
#[tokio::test]
async fn save_activity_marks_the_witness_dirty() {
    let site = Site::new();
    let s = site.open().await;

    let activity = s.begin_save_activity().unwrap();
    let active = s.inner.shared.flush_activity.snapshot();
    assert_eq!(active.active_saves, 1);
    assert_ne!(active.dirty_since_ns, 0);
    drop(activity);

    let idle = s.inner.shared.flush_activity.snapshot();
    assert_eq!(idle.active_saves, 0);
    assert_ne!(idle.dirty_since_ns, 0);
    assert_ne!(idle.last_save_ns, 0);
}

/// A commit opens no save scope. Only the flusher stamps the quiet window, when it
/// next observes the new head, so wait for that stamp rather than asserting it has
/// not happened yet, which would race the flusher's poll.
async fn assert_flusher_alone_stamps(s: &SpaceStore, before: crate::shared::FlushSnapshot) {
    let active = || s.inner.shared.flush_activity.snapshot().active_saves;
    assert_eq!(active(), before.active_saves);
    let mut stamped = false;
    for _ in 0..2000 {
        if s.inner.shared.flush_activity.snapshot().last_save_ns != before.last_save_ns {
            stamped = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(stamped, "the flusher never observed the committed head");
    assert_eq!(active(), before.active_saves);
}

/// A direct journal commit only publishes the head: it opens no save scope, and the
/// flusher alone stamps the quiet window once it observes the new head.
#[tokio::test]
async fn a_store_commit_does_not_update_flush_activity() {
    let site = Site::new();
    let s = site.open().await;
    let before = s.inner.shared.flush_activity.snapshot();

    s.commit(Commit {
        events: vec![ev("t.a", "one")],
        idempotency: None,
        durable: false,
    })
    .await
    .unwrap();

    assert_flusher_alone_stamps(&s, before).await;
}

/// A durable commit does not do witness bookkeeping in the commit path.
#[tokio::test]
async fn a_durable_commit_does_not_update_flush_activity() {
    let site = Site::new();
    let s = site.open().await;
    let before = s.inner.shared.flush_activity.snapshot();

    s.commit(Commit {
        events: vec![ev("t.a", "one")],
        idempotency: None,
        durable: true,
    })
    .await
    .unwrap();

    assert_flusher_alone_stamps(&s, before).await;
}

/// The flush worker, rather than a journal commit, publishes the cached check used by the witness.
#[tokio::test]
async fn commits_leave_the_witness_check_cache_for_the_flush_worker() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_secs(60),
            highwater_max_staleness: Duration::from_secs(120),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();

    let entries = s
        .commit(Commit {
            events: vec![ev("t.a", "direct")],
            idempotency: None,
            durable: false,
        })
        .await
        .unwrap();
    let once = s
        .commit(Commit {
            events: vec![ev("t.a", "once")],
            idempotency: None,
            durable: true,
        })
        .await
        .unwrap()
        .remove(0);

    assert_eq!(s.inner.shared.last_check(entries[0].seq), None);
    assert_eq!(s.inner.shared.last_check(once.seq), None);

    s.flush_highwater().await.unwrap();

    let mark = crate::highwater::read(&site.head_file()).unwrap();
    assert_eq!(mark.seq, once.seq);
    assert_eq!(s.inner.shared.last_check(mark.seq), Some(mark.check));
}

/// A direct commit is observed by the flush worker without commit-path activity hooks.
#[tokio::test]
async fn direct_commit_without_activity_is_flushed_by_the_worker() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_secs(1),
            highwater_max_staleness: Duration::from_millis(100),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();

    s.commit(Commit {
        events: vec![ev("t.a", "direct")],
        idempotency: None,
        durable: false,
    })
    .await
    .unwrap();

    for _ in 0..100 {
        if crate::highwater::read(&site.head_file()).is_some_and(|mark| mark.seq == 1) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the worker did not witness a commit without a save activity scope");
}

/// The maximum age flushes a dirty witness even when the quiet window is longer.
#[tokio::test]
async fn maximum_staleness_flushes_during_a_continuous_save_stream() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_secs(10),
            highwater_max_staleness: Duration::from_millis(100),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();

    for _ in 0..5 {
        put(&s, "stream", 1).await;
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    for _ in 0..40 {
        if crate::highwater::read(&site.head_file()).is_some_and(|m| m.seq > 0) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        crate::highwater::read(&site.head_file()).is_some_and(|m| m.seq > 0),
        "the maximum-age deadline must write even when saves are not quiet"
    );
}

/// The flush worker reads the head's entry check from the token the writer
/// cached, so it matches the database without opening SQLite.
#[tokio::test]
async fn the_cached_token_gives_the_flush_the_same_check_as_the_database() {
    let site = Site::new();
    let s = site.open().await;
    let entries = put(&s, "a", 3).await;
    let shared = &s.inner.shared;
    let epoch = shared.epoch().to_string();
    for e in &entries {
        let from_db =
            crate::token::check_at(&crate::schema::open_reader(&s.inner.path).unwrap(), e.seq)
                .unwrap();
        assert_eq!(shared.issued_check(&epoch, e.seq), from_db);
    }
    assert_eq!(shared.issued_check("other", entries[2].seq), None);
}

/// The maximum-age flush never runs while a save is in flight: it waits for
/// the save to end, then writes in the gap before the next save.
#[tokio::test]
async fn maximum_age_flush_waits_for_the_save_in_flight_to_end() {
    let site = Site::new();
    let s = SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: Duration::from_secs(10),
            highwater_max_staleness: Duration::from_millis(400),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap();
    let witnessed = || crate::highwater::read(&site.head_file()).is_some_and(|m| m.seq > 0);

    let save = s.begin_save_activity().unwrap();
    put(&s, "held", 1).await;
    // The flusher treats 300 ms as stale (400 ms minus one 100 ms poll) and
    // forces a flush only at twice that. 450 ms is between the two.
    tokio::time::sleep(Duration::from_millis(450)).await;
    assert!(!witnessed(), "a flush started while a save was in flight");

    drop(save);
    for _ in 0..50 {
        if witnessed() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("the flush did not follow the end of the save");
}

#[tokio::test]
async fn the_key_stays_in_the_secrets_root_and_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 1).await;
    let key = site.secrets_user().join("token.key");
    assert_eq!(
        std::fs::metadata(&key).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!site.data.join("token.key").exists());
    let again = std::fs::read(&key).unwrap();
    stop(s, &site).await;
    let s = site.open().await;
    assert_eq!(std::fs::read(&key).unwrap(), again, "the key is kept");
    drop(s);
}

/// A power cut under `synchronous=NORMAL`: `index.db` returns to its last
/// durable state, with no log tail, while the high-water file and the key,
/// each fsynced on their own, keep up. The server then writes *past* the
/// device's seq with different entries before the device reconnects; the
/// device must still be told to read again (#918, the 77762 drill).
#[tokio::test]
async fn every_file_rolled_back_to_its_last_fsync_never_hides_reused_seqs() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "durable", 3).await;
    stop(s, &site).await; // checkpointed: this is what survived
    let durable = site.data.with_file_name("durable");
    copy_dir(&site.data, &durable);

    let s = site.open().await;
    let device = put(&s, "acked", 4).await; // seqs 4..=7, tokens handed out
    s.flush_highwater().await.unwrap();
    stop(s, &site).await;
    copy_dir(&durable, &site.data);
    assert!(!site.data.join("index.db-wal").exists());

    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    // Other devices push the head past the lost device's seq first.
    put(&s, "after", 10).await;
    assert!(s.head().seq > 7);
    for e in &device {
        expired(&s, &e.token).await;
    }
    assert_eq!(restored(&s), 1, "no second rotation for the same loss");
}
