//! Disposable stage adapter and crash probes against the real Content engine.
#[path = "v2_step0_save/fence.rs"]
mod fence;

use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_content::{Content, FaultPoint, FileFormat, MAX_DOCUMENT_BYTES, Placement, Request};
use calternal_roots::Roots;
use calternal_types::{AccountId, SpaceId};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, exit},
    sync::Mutex,
    time::{Duration, Instant},
};
use uuid::Uuid;

const CHUNK: usize = 64 * 1024;
const N: usize = MAX_DOCUMENT_BYTES / CHUNK;
const PIN: Duration = Duration::from_secs(60);
const RUNS: usize = 100;
const SAFETY_RUNS: usize = 10;
const FRAME_CAP: usize = 256 * 1024;

struct Stage {
    path: PathBuf,
    receipt: Mutex<Option<[u8; 32]>>,
}

impl Stage {
    fn begin(&self, digest: [u8; 32]) -> Result<bool, ()> {
        let mut hold = self.receipt.lock().unwrap();
        if let Some(existing) = *hold {
            return if digest == existing {
                Ok(false)
            } else {
                Err(())
            };
        }
        calternal_fsio::private::directory(&self.path).unwrap();
        *hold = Some(digest);
        Ok(true)
    }

    fn chunk(&self, index: usize, bytes: &[u8]) -> bool {
        assert!(index < N && bytes.len() == CHUNK);
        let hold = self.receipt.lock().unwrap();
        if hold.is_none() {
            return false;
        }
        let file = self.path.join(index.to_string());
        match fs::read(&file) {
            Ok(existing) => existing == bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Chunks are provisional. A restarted invocation restages
                // instead of relying on an acknowledged chunk surviving a crash.
                let mut target = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&file)
                    .unwrap();
                calternal_fsio::paced::write_all(&mut target, bytes).unwrap();
                true
            }
            Err(e) => panic!("{e}"),
        }
    }

    fn finish(&self) -> Vec<u8> {
        let hold = self.receipt.lock().unwrap();
        let mut bytes = Vec::with_capacity(MAX_DOCUMENT_BYTES);
        for index in 0..N {
            bytes.extend(fs::read(self.path.join(index.to_string())).unwrap());
        }
        assert_eq!(bytes.len(), MAX_DOCUMENT_BYTES);
        assert_eq!(*hold, Some(*blake3::hash(&bytes).as_bytes()));
        bytes
    }

    fn cancel(&self) {
        let mut hold = self.receipt.lock().unwrap();
        if hold.take().is_some() {
            fs::remove_dir_all(&self.path).unwrap();
        }
    }
}

fn body() -> Vec<u8> {
    [b'"', b'\\', b'\t', b'\n'].repeat(MAX_DOCUMENT_BYTES / 4)
}

fn open(root: &Path, fault: fn(FaultPoint)) -> Content {
    Content::open(
        Roots {
            appdata: root.join("appdata"),
            userdata_ssd: root.join("ssd"),
            userdata_hdd: root.join("hdd"),
        },
        root.join("secrets"),
        Duration::ZERO,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        fault,
    )
    .unwrap()
}

fn request(account: AccountId, id: &str) -> Request<'_> {
    Request {
        account,
        via: None,
        id,
        operation: "spike.save",
        fingerprint: id,
    }
}

fn put(
    content: &Content,
    space: SpaceId,
    account: AccountId,
    previous: Option<(Uuid, &str)>,
    key: &str,
    bytes: &[u8],
) -> calternal_content::Item {
    content
        .write(
            space,
            previous.map(|p| p.0),
            "notes",
            previous.map(|p| p.1),
            json!({"title":"Spike"}),
            bytes,
            request(account, key),
            |item| {
                if key == "save" && std::env::var("STEP0_CRASH").as_deref() == Ok("before-content")
                {
                    exit(79);
                }
                json!({"id":item.id,"revision":item.revision})
            },
        )
        .unwrap()
}

fn setup(content: &Content, space: SpaceId) {
    content
        .register_kind(
            space,
            "notes",
            Placement::Visible {
                folder: "Notes".into(),
            },
            FileFormat::Markdown,
        )
        .unwrap();
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkFrame {
    offset: usize,
    bytes: String,
}

fn wire_chunk(offset: usize, bytes: &[u8]) -> (usize, Vec<u8>) {
    let frame = serde_json::to_vec(&ChunkFrame {
        offset,
        bytes: STANDARD.encode(bytes),
    })
    .unwrap();
    assert!(frame.len() < FRAME_CAP);
    let decoded: ChunkFrame = serde_json::from_slice(&frame).unwrap();
    (decoded.offset, STANDARD.decode(decoded.bytes).unwrap())
}

fn fill(stage: &Stage, body: &[u8]) {
    for (index, bytes) in body.chunks_exact(CHUNK).enumerate() {
        let (offset, decoded) = wire_chunk(index * CHUNK, bytes);
        assert_eq!(offset, index * CHUNK);
        assert!(stage.chunk(offset / CHUNK, &decoded));
    }
}

fn read_pin(
    content: &Content,
    space: SpaceId,
    item: Uuid,
    base: &str,
    rev: &str,
    offset: usize,
    until: Instant,
    authorized: bool,
) -> Option<Vec<u8>> {
    if !authorized || Instant::now() >= until {
        return None;
    }
    content
        .revision_slice(space, item, "notes", base, rev, offset..offset + CHUNK)
        .unwrap()
        .map(|(_, bytes)| wire_chunk(offset, &bytes).1)
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn staged_round_trip() {
    let body = body();
    let escaped = serde_json::to_vec(std::str::from_utf8(&body).unwrap())
        .unwrap()
        .len();
    let digest = *blake3::hash(&body).as_bytes();
    let mut durations = Vec::new();
    for _ in 0..RUNS {
        let root = tempfile::tempdir().unwrap();
        let content = open(root.path(), |_| {});
        let space = SpaceId::new();
        let account = AccountId::new();
        setup(&content, space);
        let initial = put(&content, space, account, None, "initial", b"initial");
        let stage = Stage {
            path: root.path().join("stage"),
            receipt: Mutex::new(None),
        };
        let start = Instant::now();
        let wins = std::thread::scope(|s| {
            let one = s.spawn(|| stage.begin(digest).unwrap());
            let two = s.spawn(|| stage.begin(digest).unwrap());
            u8::from(one.join().unwrap()) + u8::from(two.join().unwrap())
        });
        assert_eq!(wins, 1);
        assert!(stage.begin([0; 32]).is_err());
        fill(&stage, &body);
        // Resend a chunk whose ACK was lost, then try different bytes at that offset.
        assert!(stage.chunk(0, &body[..CHUNK]));
        assert!(!stage.chunk(0, &vec![b'x'; CHUNK]));
        let saved = put(
            &content,
            space,
            account,
            Some((initial.id, &initial.revision)),
            "save",
            &stage.finish(),
        );
        let until = Instant::now() + PIN;
        let advanced = put(
            &content,
            space,
            account,
            Some((saved.id, &saved.revision)),
            "advance",
            b"head advanced",
        );
        let mut reconstructed = Vec::with_capacity(body.len());
        for offset in (0..body.len()).step_by(CHUNK) {
            reconstructed.extend(
                read_pin(
                    &content,
                    space,
                    saved.id,
                    &initial.revision,
                    &saved.revision,
                    offset,
                    until,
                    true,
                )
                .unwrap(),
            );
        }
        assert_eq!(body, reconstructed);
        assert!(
            read_pin(
                &content,
                space,
                saved.id,
                &initial.revision,
                &saved.revision,
                0,
                until,
                false
            )
            .is_none()
        );
        assert!(
            read_pin(
                &content,
                space,
                saved.id,
                &initial.revision,
                &saved.revision,
                0,
                Instant::now(),
                true
            )
            .is_none()
        );
        assert_eq!(
            content.get(space, saved.id).unwrap().revision,
            advanced.revision
        );
        durations.push(start.elapsed().as_micros());
        stage.cancel();
        assert!(stage.begin(digest).unwrap());
        assert!(stage.chunk(0, &body[..CHUNK]));
        std::thread::scope(|scope| {
            let cancel = scope.spawn(|| stage.cancel());
            let chunk = scope.spawn(|| stage.chunk(1, &body[..CHUNK]));
            let _ = chunk.join().unwrap();
            cancel.join().unwrap();
        });
        assert!(!stage.chunk(1, &body[..CHUNK]));
        assert!(!stage.path.exists());
        // Retry stages at a fresh path while stale cleanup still owns the old
        // handle. The collector cannot remove the retry's provisional bytes.
        let retry = Stage {
            path: root.path().join("retry-stage"),
            receipt: Mutex::new(None),
        };
        std::thread::scope(|scope| {
            let collector = scope.spawn(|| stage.cancel());
            let retry = scope.spawn(|| {
                assert!(retry.begin(digest).unwrap());
                assert!(retry.chunk(0, &body[..CHUNK]));
            });
            retry.join().unwrap();
            collector.join().unwrap();
        });
        assert!(retry.path.join("0").is_file());
        retry.cancel();
        assert_eq!(content.journal(space).unwrap().len(), 3);
        content
            .trash(
                &advanced,
                advanced.metadata.clone(),
                &advanced.bytes,
                request(account, "trash"),
                |_| json!({}),
                || Ok(()),
            )
            .unwrap();
        content
            .purge(space, "notes", saved.id, request(account, "purge"))
            .unwrap();
        assert!(Instant::now() < until);
        let conn = rusqlite::Connection::open(
            root.path()
                .join(format!("appdata/spaces/{space}/content.db")),
        )
        .unwrap();
        let versions: i64 = conn
            .query_row(
                "SELECT count(*) FROM versions WHERE item=?1",
                [saved.id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(versions, 0);
        assert!(
            read_pin(
                &content,
                space,
                saved.id,
                &initial.revision,
                &saved.revision,
                0,
                until,
                true
            )
            .is_none()
        );
    }
    durations.sort_unstable();
    println!(
        "round_trip n={RUNS} decoded_bytes={} escaped_json_bytes={escaped} chunks={N} p50_us={} p95_us={} max_us={} duplicate_reservations=0 conflicting_replacements=0 post_cancel_chunks=0 retry_stage_losses=0 revoked_ranges=0 expired_ranges=0 purged_versions=0",
        body.len(),
        durations[RUNS.div_ceil(2) - 1],
        durations[(RUNS * 95).div_ceil(100) - 1],
        durations[RUNS - 1]
    );
}

fn die(point: FaultPoint) {
    match (std::env::var("STEP0_CRASH").as_deref(), point) {
        (Ok("after-content"), FaultPoint::AfterCommit) => exit(79),
        _ => {}
    }
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("STEP0_ROOT") else {
        return;
    };
    let root = Path::new(&root);
    let space = SpaceId::parse(&std::env::var("STEP0_SPACE").unwrap()).unwrap();
    let actor = AccountId::parse(&std::env::var("STEP0_ACTOR").unwrap()).unwrap();
    let item = Uuid::parse_str(&std::env::var("STEP0_ITEM").unwrap()).unwrap();
    let content = open(root, die);
    let base = content.live_revision(space, "notes", item).unwrap();
    let body = body();
    let stage = Stage {
        path: root.join("stage"),
        receipt: Mutex::new(None),
    };
    assert!(stage.begin(*blake3::hash(&body).as_bytes()).unwrap());
    if std::env::var("STEP0_CRASH").as_deref() == Ok("orphan") {
        assert!(stage.chunk(0, &body[..CHUNK]));
        exit(79);
    }
    fill(&stage, &body);
    put(
        &content,
        space,
        actor,
        Some((item, &base)),
        "save",
        &stage.finish(),
    );
    if std::env::var("STEP0_CRASH").as_deref() == Ok("after-publish-before-reply") {
        exit(79);
    }
    panic!("fault point did not fire");
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn restart_after_process_exit() {
    for point in [
        "orphan",
        "before-content",
        "after-content",
        "after-publish-before-reply",
    ] {
        for _ in 0..SAFETY_RUNS {
            let root = tempfile::tempdir().unwrap();
            let space = SpaceId::new();
            let actor = AccountId::new();
            let content = open(root.path(), |_| {});
            setup(&content, space);
            let initial = put(&content, space, actor, None, "initial", b"initial");
            drop(content);
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "crash_child"])
                .env("STEP0_ROOT", root.path())
                .env("STEP0_CRASH", point)
                .env("STEP0_SPACE", space.to_string())
                .env("STEP0_ACTOR", actor.to_string())
                .env("STEP0_ITEM", initial.id.to_string())
                .output()
                .unwrap();
            assert_eq!(
                status.status.code(),
                Some(79),
                "{point}: {}",
                String::from_utf8_lossy(&status.stdout)
            );
            // Explicit fixture restart cleanup. Content recovery knows only about
            // its own commit/outbox, not uncommitted files in this stage directory.
            fs::remove_dir_all(root.path().join("stage")).unwrap();
            let recovered = open(root.path(), |_| {});
            let receipt = recovered.receipt(space, actor, "save").unwrap();
            let journal = recovered.journal(space).unwrap();
            let head = recovered.get(space, initial.id).unwrap();
            let committed = point.starts_with("after");
            assert_eq!(receipt.is_some(), committed);
            assert_eq!(head.revision != initial.revision, committed);
            assert_eq!(journal.len(), if committed { 2 } else { 1 });
            assert!(
                recovered
                    .receipt(space, AccountId::new(), "save")
                    .unwrap()
                    .is_none()
            );
            if let Some(receipt) = receipt {
                assert_eq!(receipt.result["revision"], head.revision);
                assert_eq!(receipt.fingerprint, "save");
                assert_eq!(head.bytes, body());
                let replay = recovered.receipt(space, actor, "save").unwrap().unwrap();
                assert_eq!(replay.result, receipt.result);
            }
            let db = rusqlite::Connection::open_with_flags(
                root.path()
                    .join(format!("appdata/spaces/{space}/content.db")),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            let versions: i64 = db
                .query_row(
                    "SELECT count(*) FROM versions WHERE item=?1",
                    [initial.id.to_string()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(versions, if committed { 2 } else { 1 });
            let pending: i64 = db
                .query_row("SELECT count(*) FROM outbox", [], |row| row.get(0))
                .unwrap();
            assert_eq!(pending, 0);
        }
        println!(
            "restart point={point} n={SAFETY_RUNS} correct={SAFETY_RUNS} extra_revisions=0 extra_events=0 foreign_receipts=0"
        );
    }
}
