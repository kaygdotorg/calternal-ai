//! Updater fence (qualification case E) and Sends straddling a restart
//! (case B) through the real broker Send path and per-space store.
use super::*;
use calternal_content::execution::{PinnedExecution, Released};
use calternal_types::{ContributionId, ContributionIdentity, PackageId, PlacementRef};
use uuid::Uuid;

struct Fixture {
    dir: tempfile::TempDir,
    space: SpaceId,
    item: ItemRef,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let fixture = Self {
            dir,
            space: SpaceId::new(),
            item: ItemRef(String::new()),
        };
        let content = fixture.open();
        content
            .register_kind(
                fixture.space,
                "example",
                calternal_content::Placement::Visible {
                    folder: "Examples".into(),
                },
                calternal_content::FileFormat::Markdown,
            )
            .unwrap();
        let item = content
            .write(
                fixture.space,
                None,
                "example",
                None,
                json!({"title": "Example"}),
                b"initial",
                Request {
                    account: AccountId::new(),
                    via: None,
                    id: "create",
                    operation: "item_create",
                    fingerprint: "create",
                },
                |item| json!({"item": item.id}),
            )
            .unwrap();
        Self {
            item: ItemRef(item.id.to_string()),
            ..fixture
        }
    }

    fn open(&self) -> Content {
        let root = self.dir.path();
        Content::open(
            calternal_roots::Roots {
                appdata: root.join("appdata"),
                userdata_ssd: root.join("ssd"),
                userdata_hdd: root.join("hdd"),
            },
            root.join("secrets"),
            std::time::Duration::from_secs(60),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap()
    }

    fn send(&self, content: &Content, gate: &UpdateGate, actor: AccountId, id: &str) -> JobId {
        let admission = CheckedAdmission {
            space: self.space,
            target_field: "target".into(),
            pin: PinnedExecution {
                item: self.item.clone(),
                kind: "example".into(),
                actor,
                installation: InstallationId::new(),
                installation_generation: 1,
                installation_expiry_ms: i64::MAX,
                operation: "example.run".into(),
                declaration: "example-job".into(),
                package: PackageId("example".into()),
                package_digest: "a".repeat(64),
                harness: HarnessRef("selected-harness".into()),
                contribution: ContributionIdentity {
                    package: PackageId("contributor".into()),
                    contribution: ContributionId("run".into()),
                },
                contribution_digest: "b".repeat(64),
                entry_digest: "c".repeat(64),
                image_digest: "d".repeat(64),
                placement: Some(PlacementRef("selected-machine".into())),
                machine: Uuid::nil(),
                profile: "profile".into(),
                profile_generation: 1,
                audience_generation: 1,
                effect_ceiling: json!({}),
                input: Value::Null,
                contribution_input: Value::Null,
            },
        };
        let input = JsonObject::from([
            ("job".into(), json!("example-job")),
            ("item".into(), json!(self.item)),
            ("input".into(), json!({"text": id})),
            ("harness".into(), json!("selected-harness")),
            ("placement".into(), json!("selected-machine")),
            ("contribution_input".into(), json!({"text": id})),
        ]);
        let receipt = admission
            .start_job(
                content,
                gate,
                &RequestId(id.into()),
                input,
                |_| Ok(()),
                |_| Ok(()),
                |_| Ok(()),
            )
            .unwrap();
        serde_json::from_value(receipt["job_id"].clone()).unwrap()
    }
}

fn observe(gate: &UpdateGate) -> FenceState {
    gate.request(&json!({"mode": "observe"}), 0).unwrap()
}

fn acquire(gate: &UpdateGate, id: &str, now: i64) -> FenceState {
    gate.request(&json!({"mode": "acquire", "request_id": id}), now)
        .unwrap()
}

fn open_gate() -> UpdateGate {
    let gate = UpdateGate::recovering(None);
    gate.finish_recovery();
    assert!(gate.reopen().is_empty());
    gate
}

#[test]
fn fence_requests_follow_the_schema() {
    let gate = open_gate();
    for bad in [
        json!({"mode": "observe", "request_id": "x"}),
        json!({"mode": "acquire"}),
        json!({"mode": "acquire", "request_id": ""}),
        json!({"mode": "steal"}),
        json!([]),
    ] {
        assert!(gate.request(&bad, 0).is_err(), "{bad}");
    }
    assert_eq!(
        FenceState::Held { expires_at_ms: 0 }.to_json(),
        json!({"state": "held", "expires_at": "1970-01-01T00:00:00.000Z"})
    );
}

/// Case E, live arm without restart: busy while work or a permit exists,
/// one bounded grant at idle, Sends held durably and released once on
/// expiry in original order, and a retry after release is `released`.
#[test]
fn fence_is_granted_only_at_idle_and_releases_held_sends_once_on_expiry() {
    let fixture = Fixture::new();
    let content = fixture.open();
    let space = fixture.space;
    let gate = UpdateGate::recovering(None);
    assert_eq!(observe(&gate), FenceState::Recovering);
    assert_eq!(acquire(&gate, "early", 0), FenceState::Recovering);
    gate.recover_space(&content, space).unwrap();
    assert!(gate.finish_recovery().is_empty());
    assert!(gate.reopen().is_empty());
    assert_eq!(observe(&gate), FenceState::Idle);

    // A Send racing the grant holds a permit: the grant must wait.
    let permit = gate.admission();
    assert!(matches!(permit, Admission::Open(_)));
    assert_eq!(acquire(&gate, "update-1", 1_000), FenceState::Busy);
    drop(permit);
    let held = FenceState::Held {
        expires_at_ms: 1_000 + MAX_FENCE_MS,
    };
    assert_eq!(acquire(&gate, "update-1", 1_000), held);
    assert_eq!(acquire(&gate, "update-1", 20_000), held, "no extension");
    assert_eq!(acquire(&gate, "update-2", 20_000), held);
    assert_eq!(observe(&gate), held);

    let a = AccountId::new();
    let b = AccountId::new();
    let first = fixture.send(&content, &gate, a, "send-a");
    assert_eq!(fixture.send(&content, &gate, a, "send-a"), first);
    let second = fixture.send(&content, &gate, b, "send-b");
    assert!(
        claim_job(&content, &gate, space, first.clone(), |_| panic!(
            "launch past the fence"
        ))
        .unwrap()
        .is_none()
    );
    assert_eq!(gate.expire(1_000 + MAX_FENCE_MS - 1), None);
    assert_eq!(gate.expire(1_000 + MAX_FENCE_MS), Some(vec![space]));
    assert_eq!(gate.expire(1_000 + MAX_FENCE_MS + 1), None);
    assert_eq!(acquire(&gate, "update-1", 40_000), FenceState::Released);
    assert_eq!(acquire(&gate, "update-3", 40_000), FenceState::Busy);
    let released = gate.release_space(&content, space, |_| Ok(true)).unwrap();
    assert!(matches!(&released[..],
        [Released::Admitted(x), Released::Admitted(y)] if x.job == first && y.job == second));
    assert!(
        gate.release_space(&content, space, |_| panic!("twice"))
            .unwrap()
            .is_empty()
    );
    assert!(gate.reopen().is_empty());
    assert_eq!(
        fixture.send(&content, &gate, a, "send-a"),
        first,
        "receipt replay"
    );
    assert_eq!(observe(&gate), FenceState::Busy);
    assert_eq!(acquire(&gate, "update-1", 40_000), FenceState::Released);
    let head = claim_job(&content, &gate, space, first, |_| Ok(true)).unwrap();
    assert!(head.is_some());
}

/// Cases B and E across an applied update: a Send held under the fence and
/// one sent while the new process is still recovering are both delivered
/// exactly once, in order, after recovery; the old grant reads `released`.
#[test]
fn restart_recovery_releases_held_sends_once_and_closes_the_old_grant() {
    let fixture = Fixture::new();
    let space = fixture.space;
    let a = AccountId::new();
    let b = AccountId::new();
    let before = {
        let content = fixture.open();
        let gate = open_gate();
        assert!(matches!(
            acquire(&gate, "update-1", 0),
            FenceState::Held { .. }
        ));
        fixture.send(&content, &gate, a, "before-restart")
    };

    // The updater restarts the server. The operator socket persisted the
    // granted request ID; startup begins in `recovering`.
    let content = fixture.open();
    let gate = UpdateGate::recovering(Some("update-1".into()));
    let during = fixture.send(&content, &gate, b, "during-recovery");
    assert_eq!(observe(&gate), FenceState::Recovering);
    gate.recover_space(&content, space).unwrap();
    assert_eq!(gate.finish_recovery(), vec![space]);
    assert_eq!(observe(&gate), FenceState::Busy);
    let released = gate.release_space(&content, space, |_| Ok(true)).unwrap();
    assert!(matches!(&released[..],
        [Released::Admitted(x), Released::Admitted(y)] if x.job == before && y.job == during));
    assert!(gate.reopen().is_empty());
    assert_eq!(acquire(&gate, "update-1", 0), FenceState::Released);
    assert_eq!(fixture.send(&content, &gate, a, "before-restart"), before);
    assert!(
        claim_job(&content, &gate, space, before, |_| Ok(true))
            .unwrap()
            .is_some()
    );
    assert!(
        claim_job(&content, &gate, space, during, |_| Ok(true))
            .unwrap()
            .is_none()
    );
}
