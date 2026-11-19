//! Exercise the real content outbox and Pin inverse across process loss.
use crate::{authority, fixture};
use calternal_content::{Content, FaultPoint, FileFormat, Placement, Request};
use calternal_types::{AccountId, SpaceId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use uuid::Uuid;

#[derive(Clone, Copy, Deserialize, Serialize)]
enum Cut {
    Before,
    After,
    Reply,
}
#[derive(Deserialize, Serialize)]
struct Fixture {
    root: PathBuf,
    space: SpaceId,
    actor: AccountId,
    row: Uuid,
    activation: Uuid,
    cut: Cut,
}

fn disk(path: &Path, hook: fn(FaultPoint)) -> Content {
    let roots = fixture::roots(path);
    let guards = calternal_roots::prepare(&roots, Uuid::from_u128(1), true).unwrap();
    if !roots.appdata.join("system/auth.db").exists() {
        fixture::spaces(&roots).seed_development().unwrap();
    }
    let secrets = roots.appdata.join("secrets");
    let content = Content::open(
        roots,
        secrets,
        Duration::from_secs(120),
        calternal_content::DEFAULT_RECEIPT_RETENTION,
        hook,
    )
    .unwrap();
    content.retain_root_guard(std::sync::Arc::new(guards));
    content
}

fn hook(point: FaultPoint) {
    if point == FaultPoint::AfterCommit && std::env::var_os("STEP0_PIN_EXIT_AFTER").is_some() {
        std::process::exit(82);
    }
}

#[test]
#[ignore = "release process crash helper for Pin Undo"]
fn pin_crash_child() {
    let Ok(raw) = std::env::var("STEP0_PIN_CRASH") else {
        return;
    };
    let f: Fixture = serde_json::from_str(&raw).unwrap();
    let content = disk(&f.root, hook);
    let base = content.get(f.space, f.row).unwrap();
    let key = format!("step0/{}", f.activation);
    let mut metadata = base.metadata.clone();
    metadata["pinned"] = json!(true);
    let signing_key = authority::load_undo_key(&fixture::roots(&f.root).appdata);
    let token = authority::undo_token(&signing_key, f.actor, f.space, f.activation);
    let outcome = authority::pin_outcome(f.activation, &token);
    content
        .write(
            f.space,
            Some(f.row),
            "notes",
            Some(&base.revision),
            metadata,
            &base.bytes,
            Request {
                account: f.actor,
                via: None,
                id: &key,
                operation: "pin",
                fingerprint: &key,
            },
            |after| {
                if matches!(f.cut, Cut::Before) {
                    std::process::exit(81);
                }
                json!({"outcome":outcome,"target":f.row,"post_revision":after.revision,
                   "inverse":{"pinned":base.metadata["pinned"]}})
            },
        )
        .unwrap();
    if matches!(f.cut, Cut::Reply) {
        let mut out = std::io::stdout().lock();
        write!(out, r#"{{"state":"ok","undo":"#).unwrap();
        out.flush().unwrap();
        std::process::exit(83);
    }
    panic!("test did not cut");
}

#[test]
#[ignore = "disposable Step 0 Pin crash matrix"]
fn sealed_pin_crashes() {
    let runs: usize = std::env::var("STEP0_CUT_RUNS").map_or(10, |s| s.parse().unwrap());
    for _ in 0..runs {
        for (cut, code) in [(Cut::Before, 81), (Cut::After, 82), (Cut::Reply, 83)] {
            let dir = tempfile::tempdir().unwrap();
            let disk_before = disk(dir.path(), |_| {});
            let (space, actor) = (SpaceId::new(), AccountId::new());
            disk_before
                .register_kind(
                    space,
                    "notes",
                    Placement::Visible {
                        folder: "Notes".into(),
                    },
                    FileFormat::Markdown,
                )
                .unwrap();
            let base = disk_before
                .write(
                    space,
                    None,
                    "notes",
                    None,
                    json!({"title":"Cut","pinned":false}),
                    b"immutable body",
                    Request {
                        account: actor,
                        via: None,
                        id: "seed",
                        operation: "seed",
                        fingerprint: "seed",
                    },
                    |_| json!({}),
                )
                .unwrap();
            disk_before.materialize(space, base.id).unwrap();
            drop(disk_before);
            let activation = Uuid::new_v4();
            let config = Fixture {
                root: dir.path().to_owned(),
                space,
                actor,
                row: base.id,
                activation,
                cut,
            };
            let mut worker = Command::new(std::env::current_exe().unwrap());
            worker
                .args([
                    "--ignored",
                    "--exact",
                    "crash::pin_crash_child",
                    "--nocapture",
                ])
                .env("STEP0_PIN_CRASH", serde_json::to_string(&config).unwrap());
            if matches!(cut, Cut::After) {
                worker.env("STEP0_PIN_EXIT_AFTER", "1");
            }
            let result = worker.output().unwrap();
            assert_eq!(
                result.status.code(),
                Some(code),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            if matches!(cut, Cut::Reply) {
                assert!(
                    String::from_utf8_lossy(&result.stdout).ends_with(r#"{"state":"ok","undo":"#)
                );
            }
            let recovered = disk(dir.path(), |_| {});
            let current = recovered.get(space, base.id).unwrap();
            let receipt = recovered
                .receipt(space, actor, &format!("step0/{activation}"))
                .unwrap();
            if matches!(cut, Cut::Before) {
                assert_eq!(current.revision, base.revision);
                assert_eq!(current.metadata["pinned"], false);
                assert!(receipt.is_none());
            } else {
                let receipt = receipt.unwrap();
                assert_eq!(current.metadata["pinned"], true);
                assert_eq!(receipt.result["post_revision"], current.revision);
                assert_eq!(receipt.result["inverse"]["pinned"], false);
                let key = authority::load_undo_key(&fixture::roots(dir.path()).appdata);
                let token = authority::undo_token(&key, actor, space, activation);
                assert_eq!(receipt.result["outcome"]["undo"]["ref"], token);
            }
        }
    }
    eprintln!("pin process cuts per boundary={runs}");
}
