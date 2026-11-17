//! Failure branches in the restore witness: missing entries, changed entries
//! and unavailable storage must never let a device silently trust a lost future.

use super::*;

/// Opens a store with long flush deadlines so these tests control every write.
async fn open_with_quiet_witness(site: &Site) -> SpaceStore {
    SpaceStore::open_dir_secured(
        &site.data,
        Options {
            highwater_quiet: std::time::Duration::from_secs(60),
            highwater_max_staleness: std::time::Duration::from_secs(120),
            ..Options::default()
        },
        Some(&site.secrets),
    )
    .await
    .unwrap()
}

/// Opening a valid store with an older witness leaves the refresh to the worker.
#[tokio::test]
async fn opening_a_lagging_witness_does_not_sync_during_startup() {
    let site = Site::new();
    let first = open_with_quiet_witness(&site).await;
    put(&first, "before-reopen", 1).await;
    first.flush_highwater().await.unwrap();
    let durable = crate::highwater::read(&site.head_file()).unwrap();
    stop(first, &site).await;

    let conn = rusqlite::Connection::open(site.data.join("index.db")).unwrap();
    conn.execute(
        "INSERT INTO journal (at_ms, kind, item, data, author) VALUES (1, 'notes.edited', 'after-reopen', '{}', '00000000-0000-7000-8000-000000000002')",
        [],
    )
    .unwrap();
    drop(conn);

    let reopened = open_with_quiet_witness(&site).await;

    assert_eq!(reopened.head().seq, durable.seq + 1);
    assert_ne!(
        reopened
            .inner
            .shared
            .flush_activity
            .snapshot()
            .dirty_since_ns,
        0,
        "the flush worker must keep the routine head advance pending"
    );
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap(),
        durable,
        "startup must not sync a routine high-water advance"
    );

    reopened.flush_highwater().await.unwrap();
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap().seq,
        reopened.head().seq
    );
    stop(reopened, &site).await;
}

/// Checks that a changed witness path blocks one rotation and a retry completes it.
async fn assert_replacement_waits_for_retry(
    store: &SpaceStore,
    site: &Site,
    old_epoch: &str,
    proof: &str,
) {
    let error = store.position(proof).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(
        epoch(store),
        old_epoch,
        "stale inode cannot publish an epoch"
    );
    assert_eq!(restored(store), 0, "stale inode cannot publish a restore");

    store.flush_highwater().await.unwrap();

    assert_eq!(
        store.position(proof).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired,
        "retry publishes only after the path has a durable witness"
    );
    assert_ne!(epoch(store), old_epoch);
    assert_eq!(restored(store), 1);
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap().epoch,
        epoch(store)
    );
}

/// A hole below the head is proof even when later entries still exist.
#[tokio::test]
async fn a_signed_token_for_a_missing_entry_rotates() {
    let site = Site::new();
    let s = site.open().await;
    let entries = put(&s, "a", 3).await;
    let old_epoch = epoch(&s);
    let conn = rusqlite::Connection::open(site.data.join("index.db")).unwrap();
    conn.execute("DELETE FROM journal WHERE seq = 2", [])
        .unwrap();
    expired(&s, &entries[1].token).await;
    assert_ne!(epoch(&s), old_epoch);
    assert_eq!(restored(&s), 1);
}

/// A witness at the same seq must compare the entry, not just its position.
#[tokio::test]
async fn a_same_epoch_highwater_with_a_different_entry_rotates_at_open() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 3).await;
    let old_epoch = epoch(&s);
    stop(s, &site).await;
    let conn = rusqlite::Connection::open(site.data.join("index.db")).unwrap();
    conn.execute("UPDATE journal SET item = 'replacement' WHERE seq = 3", [])
        .unwrap();
    drop(conn);
    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), old_epoch);
}

/// A deleted witness is different from the first deploy because its flag remains.
#[tokio::test]
async fn a_missing_highwater_after_initialization_rotates_at_open() {
    let site = Site::new();
    let s = site.open().await;
    put(&s, "a", 2).await;
    let old_epoch = epoch(&s);
    stop(s, &site).await;
    std::fs::remove_file(site.head_file()).unwrap();
    let s = site.open().await;
    assert_eq!(restored(&s), 1);
    assert_ne!(epoch(&s), old_epoch);
}

/// A directory at the witness path must not let the stale fd publish an epoch.
#[tokio::test]
async fn witness_directory_replaced_does_not_publish_epoch_and_recovers() {
    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    let old_epoch = epoch(&s);
    s.flush_highwater().await.unwrap();
    let old_mark = crate::highwater::read(&site.head_file()).unwrap();
    let saved = site.secrets_user().join("head.saved");
    std::fs::copy(site.head_file(), &saved).unwrap();
    std::fs::remove_file(site.head_file()).unwrap();
    std::fs::create_dir(site.head_file()).unwrap();
    let proof = token(&s, &old_epoch, 99, 1);
    let error = s.position(&proof).await.unwrap_err();
    assert_eq!(error.code, ErrorCode::Internal);
    assert_eq!(crate::highwater::read(&saved).unwrap(), old_mark);
    assert_eq!(epoch(&s), old_epoch, "no epoch before its durable witness");
    assert_eq!(restored(&s), 0, "failed rotation is not published");
    std::fs::remove_dir(site.head_file()).unwrap();
    std::fs::copy(&saved, site.head_file()).unwrap();
    s.flush_highwater().await.unwrap();
    assert_eq!(
        s.position(&proof).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    assert_ne!(epoch(&s), old_epoch);
    assert_eq!(restored(&s), 1);
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap().epoch,
        epoch(&s)
    );
    std::fs::remove_file(saved).unwrap();
}

/// Replacing the witness's parent directory cannot leave its old fd authoritative.
#[tokio::test]
async fn witness_parent_directory_replaced_does_not_publish_epoch_and_recovers() {
    use std::os::unix::fs::MetadataExt;

    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    let proof = token(&s, &old_epoch, 99, 1);
    let user_dir = site.secrets_user();
    let moved_dir = user_dir.with_file_name("user.saved");
    let old_head = site.head_file();
    let old_bytes = std::fs::read(&old_head).unwrap();

    calternal_fsio::rename_safely(&user_dir, &moved_dir).unwrap();
    std::fs::create_dir(&user_dir).unwrap();
    std::fs::copy(moved_dir.join("token.key"), user_dir.join("token.key")).unwrap();
    calternal_fsio::write_atomic_mode(&old_head, b"different replacement", 0o600).unwrap();
    assert_eq!(std::fs::read(moved_dir.join("head")).unwrap(), old_bytes);
    assert_ne!(
        std::fs::metadata(moved_dir.join("head")).unwrap().ino(),
        std::fs::metadata(&old_head).unwrap().ino(),
        "the replacement directory must name a different witness inode"
    );

    assert_replacement_waits_for_retry(&s, &site, &old_epoch, &proof).await;
}

/// An unlinked witness is replaced durably before the pending epoch is published.
#[tokio::test]
async fn witness_path_unlinked_does_not_publish_epoch_and_recovers() {
    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    std::fs::remove_file(site.head_file()).unwrap();
    let proof = token(&s, &old_epoch, 99, 1);

    assert_eq!(
        s.position(&proof).await.unwrap_err().code,
        ErrorCode::Internal
    );
    assert_eq!(epoch(&s), old_epoch, "an unwitnessed epoch stays private");
    let conn = rusqlite::Connection::open(site.data.join("index.db")).unwrap();
    let pending_epoch: String = conn
        .query_row("SELECT value FROM meta WHERE key = 'epoch'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_ne!(pending_epoch, old_epoch);
    drop(conn);

    s.flush_highwater().await.unwrap();
    assert_eq!(
        s.position(&proof).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    assert_eq!(epoch(&s), pending_epoch);
    assert_eq!(restored(&s), 1);
    assert_eq!(
        crate::highwater::read(&site.head_file()).unwrap().epoch,
        pending_epoch
    );
}

/// An atomic replacement between flushes must be detected before rotation writes.
#[tokio::test]
async fn witness_path_replaced_between_flushes_does_not_publish_epoch() {
    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    let proof = token(&s, &old_epoch, 99, 1);
    calternal_fsio::write_atomic_mode(&site.head_file(), b"different replacement", 0o600).unwrap();

    assert_replacement_waits_for_retry(&s, &site, &old_epoch, &proof).await;
}

/// A fresh inode with the same durable mark is still valid witness evidence.
#[tokio::test]
async fn a_copied_durable_witness_can_authorize_rotation() {
    use std::os::unix::fs::MetadataExt;

    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    let proof = token(&s, &old_epoch, 99, 1);
    let head = site.head_file();
    let old_inode = std::fs::metadata(&head).unwrap().ino();
    let replacement = site.secrets_user().join("head.copy");
    std::fs::copy(&head, &replacement).unwrap();
    let copied_mark = std::fs::read(&replacement).unwrap();
    calternal_fsio::write_atomic_mode(&head, &copied_mark, 0o600).unwrap();
    assert_ne!(
        std::fs::metadata(&head).unwrap().ino(),
        old_inode,
        "the current path must name a newly copied inode"
    );

    assert_eq!(
        s.position(&proof).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    assert_ne!(epoch(&s), old_epoch);
    assert_eq!(restored(&s), 1);
    assert_eq!(
        crate::highwater::read(&head).unwrap().epoch,
        epoch(&s),
        "the current path contains the newly durable epoch"
    );
}

/// A no-op flush reports path repair before another rotation can use it.
#[tokio::test]
async fn a_noop_flush_repairs_a_replaced_witness_and_reports_internal() {
    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    let proof = token(&s, &old_epoch, 99, 1);
    let old_mark = crate::highwater::read(&site.head_file()).unwrap();
    calternal_fsio::write_atomic_mode(&site.head_file(), b"different replacement", 0o600).unwrap();

    assert_eq!(
        s.flush_highwater().await.unwrap_err().code,
        ErrorCode::Internal
    );
    assert_eq!(crate::highwater::read(&site.head_file()).unwrap(), old_mark);
    assert_eq!(epoch(&s), old_epoch, "repair alone cannot publish an epoch");

    assert_eq!(
        s.position(&proof).await.unwrap_err().code,
        ErrorCode::SyncTokenExpired
    );
    assert_ne!(epoch(&s), old_epoch);
    assert_eq!(restored(&s), 1);
}

/// A renamed old witness and a different replacement must not share authority.
#[tokio::test]
async fn witness_path_renamed_and_replaced_does_not_publish_epoch() {
    let site = Site::new();
    let s = open_with_quiet_witness(&site).await;
    put(&s, "a", 2).await;
    s.flush_highwater().await.unwrap();
    let old_epoch = epoch(&s);
    let proof = token(&s, &old_epoch, 99, 1);
    let head = site.head_file();
    let moved = site.secrets_user().join("head.renamed");
    calternal_fsio::rename_safely(&head, &moved).unwrap();
    calternal_fsio::write_atomic_mode(&head, b"different replacement", 0o600).unwrap();

    assert_replacement_waits_for_retry(&s, &site, &old_epoch, &proof).await;
}

/// A failed first-deploy write has never established a witness. Retrying
/// after storage is repaired must not turn initialization into a restore.
#[tokio::test]
async fn a_failed_first_deploy_can_retry_without_rotating() {
    let site = Site::new();
    let plain = SpaceStore::open_dir(&site.data).await.unwrap();
    put(&plain, "existing", 2).await;
    let old_epoch = epoch(&plain);
    stop(plain, &site).await;
    std::fs::create_dir_all(site.secrets_user()).unwrap();
    std::fs::create_dir(site.head_file()).unwrap();
    let attempt =
        SpaceStore::open_dir_secured(&site.data, Options::default(), Some(&site.secrets)).await;
    assert!(
        attempt.is_err(),
        "the witness write must report its failure"
    );
    std::fs::remove_dir(site.head_file()).unwrap();
    let s = site.open().await;
    assert_eq!(epoch(&s), old_epoch, "initialization rotates nobody");
    assert_eq!(restored(&s), 0);
}
