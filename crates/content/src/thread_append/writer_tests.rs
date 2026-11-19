use super::*;
use crate::{FileFormat, Placement, Roots};
use calternal_types::AccountId;
use std::{
    fs::OpenOptions,
    io::Write,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};

const RECORD: &[u8] = b"\n## Assistant\n\nComplete content.\n";

fn eventually<T>(run: impl Fn() -> Result<T>) -> Result<T> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        match run() {
            Err(e) if proof::pending(&e) && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

#[test]
fn reopening_repairs_each_durable_torn_suffix_before_reading() {
    static CRASH: AtomicBool = AtomicBool::new(false);
    for cut in [0, 1, 7, RECORD.len() / 2, RECORD.len() - 1, RECORD.len()] {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots {
            appdata: dir.path().join("appdata"),
            userdata_ssd: dir.path().join("ssd"),
            userdata_hdd: dir.path().join("hdd"),
        };
        let open = |fault| {
            Content::open(
                roots.clone(),
                dir.path().join("secrets"),
                Duration::from_secs(60),
                crate::DEFAULT_RECEIPT_RETENTION,
                fault,
            )
            .unwrap()
        };
        let content = open(|point| {
            if matches!(point, FaultPoint::AfterCommit) && CRASH.swap(false, Ordering::SeqCst) {
                panic!("simulate process death after queuing the append");
            }
        });
        let space = SpaceId::new();
        let account = AccountId::new();
        let req = |id| Request {
            account,
            via: None,
            id,
            operation: "threads.checkpoint",
            fingerprint: id,
        };
        content
            .register_kind(
                space,
                "threads",
                Placement::Visible {
                    folder: "Chats".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        let original = b"---\nformat: calternal-thread/v2\n---\n\n# Talk\n\nOld bytes.\n";
        let item = content
            .write(
                space,
                None,
                "threads",
                None,
                json!({"title":"Talk"}),
                original,
                req("old"),
                |_| Value::Null,
            )
            .unwrap();
        content.materialize(space, item.id).unwrap();
        let (head, _) = content.thread_head(space, item.id).unwrap();
        let path = roots
            .userdata_ssd
            .join("spaces")
            .join(space.to_string())
            .join(head.path);
        let physical = std::fs::read(&path).unwrap();
        CRASH.store(true, Ordering::SeqCst);
        let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = eventually(|| {
                content.append_thread_record(
                    space,
                    item.id,
                    &item.revision,
                    json!({"title":"Talk"}),
                    RECORD,
                    req("new"),
                    |_| json!({"saved":true}),
                )
            });
        }));
        assert!(crashed.is_err());
        drop(content);

        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(&RECORD[..cut])
            .unwrap();
        let restarted = open(|_| {});
        let saved = eventually(|| restarted.get(space, item.id)).unwrap();
        assert_eq!(
            saved.bytes,
            [original.as_slice(), RECORD].concat(),
            "cut {cut}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            [physical.as_slice(), RECORD].concat(),
            "cut {cut}"
        );
        let receipt = restarted.receipt(space, account, "new").unwrap().unwrap();
        assert_eq!(receipt.result, json!({"saved":true}));
        let old = restarted.history(space, item.id, 10, None).unwrap();
        assert!(
            old.0
                .iter()
                .any(|v| v.revision == item.revision && v.bytes == original)
        );
    }
}

const FIRST: &[u8] = b"\n## First\n\nOne.\n";
const SECOND: &[u8] = b"\n## Second\n\nTwo.\n";

struct BatchFixture {
    _dir: tempfile::TempDir,
    roots: Roots,
    secrets: std::path::PathBuf,
    space: SpaceId,
    account: AccountId,
    item: Item,
    file: std::path::PathBuf,
}

impl BatchFixture {
    fn new() -> (Self, Content) {
        let dir = tempfile::tempdir().unwrap();
        let roots = Roots {
            appdata: dir.path().join("appdata"),
            userdata_ssd: dir.path().join("ssd"),
            userdata_hdd: dir.path().join("hdd"),
        };
        let secrets = dir.path().join("secrets");
        let content = Content::open(
            roots.clone(),
            secrets.clone(),
            Duration::from_secs(60),
            crate::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap();
        let space = SpaceId::new();
        let account = AccountId::new();
        content
            .register_kind(
                space,
                "threads",
                Placement::Visible {
                    folder: "Chats".into(),
                },
                FileFormat::Markdown,
            )
            .unwrap();
        let item = content
            .write(
                space,
                None,
                "threads",
                None,
                json!({"title":"Talk"}),
                b"---\nformat: calternal-thread/v2\n---\n\n# Talk\n\nOld bytes.\n",
                Request {
                    account,
                    via: None,
                    id: "base",
                    operation: "threads.create",
                    fingerprint: "base",
                },
                |_| Value::Null,
            )
            .unwrap();
        content.materialize(space, item.id).unwrap();
        let (head, _) = content.thread_head(space, item.id).unwrap();
        let file = roots
            .userdata_ssd
            .join("spaces")
            .join(space.to_string())
            .join(head.path);
        (
            Self {
                _dir: dir,
                roots,
                secrets,
                space,
                account,
                item,
                file,
            },
            content,
        )
    }

    fn reopen(&self, fault: fn(FaultPoint)) -> Content {
        Content::open(
            self.roots.clone(),
            self.secrets.clone(),
            Duration::from_secs(60),
            crate::DEFAULT_RECEIPT_RETENTION,
            fault,
        )
        .unwrap()
    }

    fn prepare_pair(&self, content: &Content) {
        let stage = |base, request, record| {
            content.stage_thread_record(
                self.space,
                self.item.id,
                base,
                self.item.metadata.clone(),
                record,
                Request {
                    account: self.account,
                    via: None,
                    id: request,
                    operation: "threads.checkpoint",
                    fingerprint: request,
                },
                |_| json!({"id":request}),
            )
        };
        let (first, _) = eventually(|| stage(&self.item.revision, "first", FIRST)).unwrap();
        // Hold the same queue window open deterministically while the second
        // caller stages against the next revision. Neither caller has acked.
        let slot = crate::journal::lock(&content.spaces)
            .get(&self.space)
            .unwrap()
            .clone();
        crate::journal::lock(&slot.space)
            .as_mut()
            .unwrap()
            .append_group
            .as_mut()
            .expect("first record is grouped")
            .closes = Instant::now() + Duration::from_secs(10);
        let (next, _) = content
            .thread_head_for_append(self.space, self.item.id)
            .unwrap();
        assert_eq!(next.revision, first);
        stage(&next.revision, "second", SECOND).unwrap();
        content
            .with_space_access(self.space, Some(self.item.id), |s| {
                let group = s.append_group.as_ref().unwrap();
                assert_eq!(group.last - group.first, 1);
                for request in ["first", "second"] {
                    assert!(
                        content
                            .journals
                            .receipt(&s.journal, self.account, request)?
                            .is_none()
                    );
                }
                Ok(())
            })
            .unwrap();
    }

    fn check_receipts(&self, content: &Content) {
        for request in ["first", "second"] {
            assert_eq!(
                content
                    .receipt(self.space, self.account, request)
                    .unwrap()
                    .unwrap()
                    .result,
                json!({"id":request}),
            );
        }
    }
}

#[test]
fn two_pending_records_share_one_placement_sync_and_retire_together() {
    use std::sync::atomic::AtomicUsize;
    static PLACED: AtomicUsize = AtomicUsize::new(0);
    let (f, initial) = BatchFixture::new();
    drop(initial);
    let content = f.reopen(|point| {
        if point == FaultPoint::AfterPlacement {
            PLACED.fetch_add(1, Ordering::SeqCst);
        }
    });
    f.prepare_pair(&content);
    let rows = content
        .with_space(f.space, |s| {
            // The first ordinary read seals the group, but both intent pins
            // are still durable until the retirement worker runs.
            let rows: u32 = s
                .conn
                .query_row("SELECT count(*) FROM outbox", [], |r| r.get(0))
                .map_err(internal)?;
            assert!(s.published_append_cutoff() > 0);
            Ok(rows)
        })
        .unwrap();
    assert_eq!(rows, 2);
    assert_eq!(PLACED.load(Ordering::SeqCst), 1);
    f.check_receipts(&content);
    content
        .with_space(f.space, |s| {
            s.published_appends.as_mut().unwrap().1 = Instant::now() - Duration::from_secs(2);
            assert!(!s.retire_appends_if_due());
            let rows: u32 = s
                .conn
                .query_row("SELECT count(*) FROM outbox", [], |r| r.get(0))
                .map_err(internal)?;
            assert_eq!(rows, 0);
            Ok(())
        })
        .unwrap();
    f.check_receipts(&content);
}

#[test]
fn restart_replays_a_group_torn_in_either_record_before_serving_it() {
    let both = [FIRST, SECOND].concat();
    for cut in [
        0,
        1,
        FIRST.len() - 1,
        FIRST.len(),
        FIRST.len() + 3,
        both.len(),
    ] {
        let (f, content) = BatchFixture::new();
        let prefix = std::fs::read(&f.file).unwrap();
        f.prepare_pair(&content);
        // Drop the process-local file lease without acknowledging. Preserve
        // only the part of the dirty suffix that made it to disk.
        drop(content);
        OpenOptions::new()
            .write(true)
            .open(&f.file)
            .unwrap()
            .set_len((prefix.len() + cut) as u64)
            .unwrap();
        let restarted = f.reopen(|_| {});
        let saved = eventually(|| restarted.get(f.space, f.item.id)).unwrap();
        assert_eq!(
            saved.bytes,
            [f.item.bytes.as_slice(), both.as_slice()].concat()
        );
        assert_eq!(
            std::fs::read(&f.file).unwrap(),
            [prefix, both.clone()].concat()
        );
        f.check_receipts(&restarted);
    }
}

#[test]
fn first_journal_receipt_recovers_a_lost_group_placement_mark() {
    static CRASH: AtomicBool = AtomicBool::new(false);
    let (f, initial) = BatchFixture::new();
    drop(initial);
    let content = f.reopen(|point| {
        if point == FaultPoint::AfterJournalCommit && CRASH.swap(false, Ordering::SeqCst) {
            panic!("exit after the first append receipt");
        }
    });
    f.prepare_pair(&content);
    CRASH.store(true, Ordering::SeqCst);
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = content.get(f.space, f.item.id);
    }));
    assert!(stopped.is_err());
    drop(content);
    // A power loss can discard the NORMAL placement mark even when the
    // Markdown sync and the first journal receipt are already durable.
    let db = rusqlite::Connection::open(crate::db::space_path(&f.roots.appdata, f.space)).unwrap();
    db.execute(
        "UPDATE items SET mat_state='pending' WHERE id=?1",
        [f.item.id.to_string()],
    )
    .unwrap();
    drop(db);
    OpenOptions::new()
        .append(true)
        .open(&f.file)
        .unwrap()
        .write_all(b"Outside edit.\n")
        .unwrap();
    let physical = std::fs::read(&f.file).unwrap();
    let restarted = f.reopen(|_| {});
    let saved = restarted.get(f.space, f.item.id).unwrap();
    assert_eq!(
        saved.bytes,
        [f.item.bytes.as_slice(), FIRST, SECOND].concat()
    );
    assert_eq!(std::fs::read(&f.file).unwrap(), physical);
    f.check_receipts(&restarted);
}
