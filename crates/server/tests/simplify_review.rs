//! Independent regression probes for the simplify lane; failures block integration.
mod support;

use calternal_content::Request;
use calternal_server::{machines::admin, store::Store};
use calternal_types::{AccountId, SpaceId};
use rusqlite::Connection;
use serde_json::json;

#[test]
fn machine_cli_refuses_missing_roots_before_creating_appdata() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("unmounted-appdata");
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_calternal"))
        .args(["machines", "list", "--appdata"])
        .arg(&missing)
        .arg("--json")
        .output()
        .unwrap();
    assert!(
        !missing.exists(),
        "machines list created an unpaired appdata root before validating storage"
    );
    assert!(!result.status.success(), "unguarded root must be refused");
}

#[test]
fn expired_control_receipt_allows_a_new_request_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let roots = support::roots(dir.path());
    let installation = uuid::Uuid::new_v4();
    let account = AccountId::new();
    let space = SpaceId::new();
    let store = Store::open(
        &roots,
        installation,
        true,
        std::time::Duration::from_secs(7 * 86400),
    )
    .unwrap();
    store
        .content()
        .record_receipt(
            space,
            Request {
                account,
                id: "reusable",
                operation: "plugins.enable",
                fingerprint: "old-input",
            },
            json!({"old":true}),
        )
        .unwrap();
    // A real installation has auth.db before its next guarded open.
    drop(calternal_auth::db::open_conn(&roots.appdata.join("system/auth.db")).unwrap());
    drop(store);
    // Inject an old receipt timestamp, with no wall-clock waits. Startup's normal
    // prune removes it; permanent publication receipts deliberately remain.
    let journal = roots
        .appdata
        .join("spaces")
        .join(space.to_string())
        .join("index.db");
    Connection::open(journal)
        .unwrap()
        .execute("UPDATE idempotency SET at_ms=0", [])
        .unwrap();
    let store = Store::open(
        &roots,
        installation,
        false,
        std::time::Duration::from_secs(7 * 86400),
    )
    .unwrap();
    assert!(store.receipt(account, space, "reusable").unwrap().is_none());
    let result = store.content().record_receipt(
        space,
        Request {
            account,
            id: "reusable",
            operation: "plugins.disable",
            fingerprint: "new-input",
        },
        json!({"new":true}),
    );
    assert!(
        result.is_ok(),
        "expired receipt must permit a new request: {result:?}"
    );
}

#[test]
fn machine_pool_grant_requires_a_working_recovery_log() {
    let dir = tempfile::tempdir().unwrap();
    let roots = support::roots(dir.path());
    let installation = uuid::Uuid::new_v4();
    let _store = Store::open(
        &roots,
        installation,
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let log = roots.userdata_ssd.join(".calternal/recovery");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    // A regular file cannot be opened as a recovery directory. There is no
    // permissions or root-user dependence in this failure injection.
    std::fs::write(&log, b"unavailable recovery directory").unwrap();
    let account = AccountId::new();
    // Adapted call (simplify lane): machine administration takes the guarded
    // store, since an appdata path alone cannot reach the recovery log.
    let result = admin::service_for(&_store).and_then(|svc| svc.set_pool_grant(account, true));
    assert!(
        result.is_err(),
        "pool authority was acknowledged without a usable recovery log"
    );
}

#[test]
fn cancelled_worker_cold_start_releases_capacity_for_other_spaces() {
    use calternal_server::plugins::{
        PreviewHost, WorkerPool,
        worker::{Call, NodeRuntime, Outcome},
    };
    use std::{
        future::Future,
        sync::Arc,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .max_blocking_threads(1)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let bundle = support::worker(dir.path(), "done(f,{})", "");
        let plugin = Arc::new(support::plugin(
            support::insights_manifest(),
            Some(&bundle),
            true,
        ));
        // Spawn failure is intentional: capacity must be released regardless of
        // outcome. No real Node executable or timing-sensitive process is needed.
        let pool = WorkerPool::with_capacity(
            Some(NodeRuntime {
                path: dir.path().join("nonexistent-node"),
                version: "v22.13.0".into(),
            }),
            1,
        );
        let call = || Call {
            operation: "notes-insights.summarize".into(),
            input: json!({}),
            account: AccountId::new(),
            host: Arc::new(PreviewHost {
                kind: "notes".into(),
            }),
            timeout: Duration::from_secs(3),
            cancel: None,
            job: None,
        };
        let (release, wait) = std::sync::mpsc::channel();
        let (entered, started) = tokio::sync::oneshot::channel();
        let blocker = tokio::task::spawn_blocking(move || {
            entered.send(()).unwrap();
            wait.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        started.await.unwrap();
        let space = SpaceId::new();
        let mut first = Box::pin(pool.invoke(&plugin, space, call()));
        assert!(matches!(
            first.as_mut().poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        // Cancellation happens after reservation but before the cold-start closure
        // can execute. Its detached blocking task still completes after release.
        drop(first);
        release.send(()).unwrap();
        blocker.await.unwrap();
        tokio::task::spawn_blocking(|| ()).await.unwrap();
        let next = pool.invoke(&plugin, SpaceId::new(), call()).await;
        let Outcome::Failed(error) = next else {
            panic!("missing executable should fail");
        };
        assert_ne!(
            error
                .details
                .as_ref()
                .and_then(|d| d.get("reason"))
                .and_then(|r| r.as_str()),
            Some("worker_capacity"),
            "a cancelled cold start permanently consumed the only pool slot"
        );
    });
}

#[test]
fn held_space_write_does_not_stall_another_space_and_stale_base_loses() {
    use calternal_content::{FileFormat, Placement};
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(
        &support::roots(dir.path()),
        uuid::Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let content = Arc::clone(&store.shared_content());
    let a = SpaceId::new();
    let b = SpaceId::new();
    let account = AccountId::new();
    for space in [a, b] {
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
    let request = |id| Request {
        account,
        id,
        operation: "notes.create",
        fingerprint: id,
    };
    let initial = content
        .write(
            a,
            None,
            "notes",
            None,
            json!({"title":"A"}),
            b"initial",
            request("initial"),
            |_| json!({}),
        )
        .unwrap();
    let (entered, ready) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (finished, other_done) = mpsc::channel();
    std::thread::scope(|scope| {
        let first_content = &content;
        let original = &initial;
        let first = scope.spawn(move || {
            first_content.write(
                a,
                Some(original.id),
                "notes",
                Some(&original.revision),
                json!({"title":"A"}),
                b"first",
                request("first"),
                |_| {
                    entered.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(10)).unwrap();
                    json!({})
                },
            )
        });
        ready.recv_timeout(Duration::from_secs(10)).unwrap();
        let second = scope.spawn(|| {
            content.write(
                a,
                Some(initial.id),
                "notes",
                Some(&initial.revision),
                json!({"title":"A"}),
                b"stale",
                request("stale"),
                |_| json!({}),
            )
        });
        scope.spawn(|| {
            finished
                .send(content.write(
                    b,
                    None,
                    "notes",
                    None,
                    json!({"title":"B"}),
                    b"other",
                    request("other"),
                    |_| json!({}),
                ))
                .unwrap();
        });
        let progressed = other_done.recv_timeout(Duration::from_secs(5));
        release.send(()).unwrap();
        let first = first.join().unwrap().unwrap();
        let second = second.join().unwrap().unwrap_err();
        assert!(
            progressed
                .expect("space B waited for space A's held write")
                .is_ok()
        );
        assert_eq!(second.code, calternal_types::ErrorCode::Conflict);
        assert_eq!(content.get(a, initial.id).unwrap().revision, first.revision);
        assert_eq!(content.journal(a).unwrap().len(), 2);
        assert_eq!(content.journal(b).unwrap().len(), 1);
    });
}

#[test]
fn concurrent_first_open_and_cache_pressure_preserve_a_pinned_journal() {
    use calternal_content::{FileFormat, Placement};
    use std::sync::{Arc, Barrier};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(
        &support::roots(dir.path()),
        uuid::Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let content = store.shared_content();
    let spaces = [SpaceId::new(), SpaceId::new()];
    let account = AccountId::new();
    let barrier = Barrier::new(16);
    std::thread::scope(|scope| {
        let mut writes = Vec::new();
        for n in 0..16 {
            let content = &content;
            let barrier = &barrier;
            let space = spaces[n % 2];
            writes.push(scope.spawn(move || {
                barrier.wait();
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
                let id = format!("request-{}", n / 2);
                content
                    .write(
                        space,
                        None,
                        "notes",
                        None,
                        json!({"title":id}),
                        id.as_bytes(),
                        Request {
                            account,
                            id: &id,
                            operation: "notes.create",
                            fingerprint: &id,
                        },
                        |_| json!({}),
                    )
                    .unwrap();
            }));
        }
        for write in writes {
            write.join().unwrap();
        }
    });
    for space in spaces {
        assert_eq!(content.list(space, "notes", 100).unwrap().len(), 8);
        assert_eq!(content.journal(space).unwrap().len(), 8);
    }
    let pinned = content.live_journal(spaces[0]).unwrap();
    let follower = pinned.subscribe_bounded(1).unwrap();
    let head = pinned.head_cell();
    for _ in 0..70 {
        content.live_journal(SpaceId::new()).unwrap();
    }
    content.sweep();
    let again = content.live_journal(spaces[0]).unwrap();
    assert!(
        Arc::ptr_eq(&head, &again.head_cell()),
        "cache pressure opened a second writer for a pinned journal"
    );
    assert_eq!(again.listeners(), 1);
    drop(follower);
}

#[test]
fn held_plugin_toggle_does_not_block_another_space() {
    use calternal_server::store::PluginRow;
    use std::{sync::mpsc, time::Duration};
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(
        &support::roots(dir.path()),
        uuid::Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let (entered, started) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let (progress, other_started) = mpsc::channel();
    let account = AccountId::new();
    std::thread::scope(|scope| {
        let st = &store;
        let first = scope.spawn(move || {
            st.set_plugin_enabled(
                account,
                "one",
                "plugins.enable",
                "one",
                SpaceId::new(),
                "fixture",
                PluginRow {
                    enabled: false,
                    generation: 0,
                },
                true,
                |_| {
                    // The result encoder runs after the row read, inside the writer
                    // transaction. Holding it models a stalled transaction/fsync.
                    entered.send(()).unwrap();
                    wait.recv_timeout(Duration::from_secs(10)).unwrap();
                    json!({})
                },
            )
        });
        started.recv_timeout(Duration::from_secs(10)).unwrap();
        let second = scope.spawn(|| {
            store.set_plugin_enabled(
                account,
                "two",
                "plugins.enable",
                "two",
                SpaceId::new(),
                "fixture",
                PluginRow {
                    enabled: false,
                    generation: 0,
                },
                true,
                |_| {
                    progress.send(()).unwrap();
                    json!({})
                },
            )
        });
        let independent = other_started.recv_timeout(Duration::from_secs(1));
        release.send(()).unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
        assert!(
            independent.is_ok(),
            "space B's plugin transaction waited for space A's installation-wide writer"
        );
    });
}

#[test]
fn manager_can_cancel_another_accounts_job_without_blocking_the_async_worker() {
    use calternal_server::{
        api::{AppState, allowed_hosts, dispatch},
        auth::StoredDirectory,
        plugins::{Jobs, Registry, Session, WorkerPool, jobs::Admission},
    };
    use std::sync::Arc;
    let dir = tempfile::tempdir().unwrap();
    let roots = support::roots(dir.path());
    let store = Store::open(
        &roots,
        uuid::Uuid::new_v4(),
        true,
        calternal_content::DEFAULT_RECEIPT_RETENTION,
    )
    .unwrap();
    let directory = StoredDirectory::load_or_create(&roots.appdata).unwrap();
    let ids = support::dev(&directory);
    directory
        .spaces
        .set_role(ids.owner.id, ids.shared, ids.guest.id, "manager")
        .unwrap();
    let jobs = Jobs::open(&roots.appdata).unwrap();
    let id = jobs
        .admit(Admission {
            space: ids.shared.to_string(),
            plugin: "fixture".into(),
            account: ids.owner.id,
            session: Session::Static,
            operation: "fixture.run".into(),
            input: json!({}),
            parent: None,
            generation: 0,
            request_key: "one-job".into(),
        })
        .unwrap();
    let state = Arc::new(AppState {
        inputs: store.account_inputs(),
        store,
        directory,
        registry: Registry::from_plugins(vec![]),
        workers: WorkerPool::new(None),
        jobs,
        allowed_hosts: allowed_hosts(0),
    });
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let task = tokio::spawn(async move {
            dispatch(
                state,
                ids.guest,
                Session::Static,
                "cancel",
                "jobs.cancel",
                serde_json::from_value(json!({"job_id":id})).unwrap(),
            )
            .await
        });
        let result = task
            .await
            .expect("manager cancellation called a blocking directory method on an async worker")
            .unwrap();
        assert_eq!(result["job"]["state"], "cancelled");
    });
}
