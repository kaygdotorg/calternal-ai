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
        let original = b"---\nformat: calternal-thread/v1\n---\n\n# Talk\n\nOld bytes.\n";
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
