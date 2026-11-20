use super::broker::bind_target;
use super::*;
use calternal_content::execution::{ArchiveTarget, PinnedExecution};
use calternal_types::{ContributionId, ContributionIdentity, PackageId, PlacementRef};
use uuid::Uuid;

#[test]
fn checked_admission_rejects_a_second_target_even_when_equal() {
    let input = json!({"item": "checked-item"});
    let result = bind_target(input, "item", &ItemRef("checked-item".into()));
    assert!(result.is_err());
}

#[test]
fn checked_admission_injects_the_target_at_the_declared_field() {
    let result = bind_target(
        json!({"text": "hello"}),
        "target",
        &ItemRef("checked-item".into()),
    )
    .unwrap();
    assert_eq!(result, json!({"text": "hello", "target": "checked-item"}));
}

#[test]
fn stop_context_is_only_redeemable_by_its_original_scope() {
    let scope = ControlScope {
        space: SpaceId::new(),
        actor: AccountId::new(),
        installation: InstallationId::new(),
        lease: LeaseRef("current".into()),
        component: "item-view".into(),
        item: ItemRef("item".into()),
    };
    let target = ArchiveTarget {
        intent: calternal_types::ExecutionIntentRef("intent".into()),
        attempt: None,
        starter: scope.actor,
        queued: false,
    };
    let context = StopContext {
        scope: scope.clone(),
        target,
    };
    let mut other = scope.clone();
    other.lease = LeaseRef("replacement".into());
    assert!(context.require_scope(&other).is_err());
    other = scope.clone();
    other.installation = InstallationId::new();
    assert!(context.require_scope(&other).is_err());
    other = scope.clone();
    other.component = "other-component".into();
    assert!(context.require_scope(&other).is_err());
    assert!(context.require_scope(&scope).is_ok());
}

#[test]
fn send_start_and_confirmed_stop_share_the_durable_job() {
    let dir = tempfile::tempdir().unwrap();
    let roots = calternal_roots::Roots {
        appdata: dir.path().join("appdata"),
        userdata_ssd: dir.path().join("ssd"),
        userdata_hdd: dir.path().join("hdd"),
    };
    let open = || {
        Content::open(
            roots.clone(),
            dir.path().join("secrets"),
            std::time::Duration::from_secs(60),
            calternal_content::DEFAULT_RECEIPT_RETENTION,
            |_| {},
        )
        .unwrap()
    };
    let space = SpaceId::new();
    let actor = AccountId::new();
    let installation = InstallationId::new();
    let content = open();
    content
        .register_kind(
            space,
            "example",
            calternal_content::Placement::Visible {
                folder: "Examples".into(),
            },
            calternal_content::FileFormat::Markdown,
        )
        .unwrap();
    let item = content
        .write(
            space,
            None,
            "example",
            None,
            json!({"title": "Example"}),
            b"initial",
            Request {
                account: actor,
                via: None,
                id: "create",
                operation: "item_create",
                fingerprint: "create",
            },
            |item| json!({"item": item.id}),
        )
        .unwrap();
    let item = ItemRef(item.id.to_string());
    drop(content);
    let db = rusqlite::Connection::open(
        roots
            .appdata
            .join("spaces")
            .join(space.to_string())
            .join("content.db"),
    )
    .unwrap();
    let prior = calternal_content::SPACE_SCHEMA.version();
    let changes = [(
        prior + 1,
        include_str!("../../../../content/src/execution/schema.sql"),
    )];
    calternal_store::migrations::migrate_release(
        &db,
        calternal_store::migrations::Schema::new("space content.db", prior, &[], &changes),
        true,
    )
    .unwrap();
    drop(db);
    let content = open();
    let scope = || CheckedAdmission {
        space,
        target_field: "target".into(),
        pin: PinnedExecution {
            item: item.clone(),
            kind: "example".into(),
            actor,
            installation,
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
    let send = |text: &str| {
        JsonObject::from([
            ("job".into(), json!("example-job")),
            ("item".into(), json!(item)),
            ("input".into(), json!({"text": text})),
            ("harness".into(), json!("selected-harness")),
            ("placement".into(), json!("selected-machine")),
            ("contribution_input".into(), json!({"text":text})),
        ])
    };
    let request = RequestId("checked-send".into());
    let do_send = |text| {
        scope().start_job(
            &content,
            &request,
            send(text),
            |input| {
                assert_eq!(input, &json!({"target":item, "text":text}));
                Ok(())
            },
            |input| {
                assert_eq!(input, &json!({"text":text}));
                Ok(())
            },
            |_| Ok(()),
        )
    };
    let admission = do_send("hello").unwrap();
    assert_eq!(do_send("hello").unwrap(), admission);
    assert_eq!(do_send("changed").unwrap_err(), Error::request_reused());
    let job: JobId = serde_json::from_value(admission["job_id"].clone()).unwrap();
    let claimed = claim_job(&content, space, job, |_| Ok(true))
        .unwrap()
        .unwrap();
    assert_eq!(claimed.operation(), "example.run");
    assert_eq!(
        claimed.context()["contribution_input"],
        json!({"text":"hello"})
    );
    let start = JsonObject::from([
        ("request_id".into(), json!("start")),
        ("harness".into(), json!("selected-harness")),
    ]);
    let mut resume = start.clone();
    resume.insert("continuation".into(), json!("unqualified-continuation"));
    assert!(claimed.start_attempt(&content, resume, |_| Ok(())).is_err());
    let attempt = claimed
        .start_attempt(&content, start.clone(), |_| Ok(()))
        .unwrap();
    assert_eq!(
        claimed.start_attempt(&content, start, |_| Ok(())).unwrap(),
        attempt
    );
    let controls = ControlScope {
        space,
        actor,
        installation,
        lease: LeaseRef("live-view".into()),
        component: "item-view".into(),
        item,
    };
    let mut peer = controls.clone();
    peer.actor = AccountId::new();
    assert!(
        stop_controls(&content, &peer, || Ok(false))
            .unwrap()
            .is_empty()
    );
    let context = stop_controls(&content, &controls, || Ok(false))
        .unwrap()
        .pop()
        .unwrap();
    context.resolve(&content, &controls, || Ok(false)).unwrap();
    let confirmed = context
        .confirm(&controls, RequestId("confirmed-stop".into()), |request| {
            assert_eq!(request.0, "confirmed-stop");
            Ok(())
        })
        .unwrap();
    let stopped = confirmed
        .dispatch(&content, &controls, || Ok(false))
        .unwrap();
    assert_eq!(stopped["execution"], attempt["execution"]);
    assert_eq!(stopped["phase"], "stopped");
    assert_eq!(
        confirmed
            .dispatch(&content, &controls, || Ok(false))
            .unwrap(),
        stopped
    );
    assert!(
        stop_controls(&content, &controls, || Ok(false))
            .unwrap()
            .is_empty()
    );
}
