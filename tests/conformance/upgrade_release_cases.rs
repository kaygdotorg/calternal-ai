//! Cross-release upgrade proof over data written through the live service.
use super::*;

mod fixture;
use fixture::*;

#[tokio::test]
async fn release_upgrade_proves_compatible_handoff_and_refuses_an_incompatible_downgrade() {
    let previous = binary("CALTERNAL_RELEASE_N_BIN");
    let candidate = binary("CALTERNAL_RELEASE_NEXT_BIN");
    let fixture = std::env::var_os("CALTERNAL_RELEASE_FIXTURE_OUT").map(PathBuf::from);
    let mut seeded = Fixture::empty();
    let mut shape = seed(&mut seeded, &previous).await;
    shape["revision"] = json!(std::env::var("CALTERNAL_RELEASE_N_REVISION").unwrap());
    if let Some(directory) = &fixture {
        export_seed(&seeded, &shape, directory);
    }
    let archive = tempfile::tempdir().unwrap();
    export_seed(&seeded, &shape, archive.path());
    let installed = archive.path().join("release-n.tar.gz");
    let old_schema = stored_machine_schema(&seeded);

    for point in [
        "upgrade_locked",
        "upgrade_versions_checked",
        "upgrade_copied",
        "upgrade_intent",
        "upgrade_migrated",
        "upgrade_verified",
    ] {
        let mut f = load_seed(&installed, seeded.cookies.clone());
        let evidence = proof(&f);
        let failed = prepare(&f, &candidate, &evidence, Some(point));
        assert!(!failed.status.success(), "{point}");
        assert!(
            String::from_utf8_lossy(&failed.stderr).contains(point),
            "{point}"
        );
        let restored = answer(upgrade(&f, &previous, &["recover"], None));
        if ["upgrade_intent", "upgrade_migrated", "upgrade_verified"].contains(&point) {
            assert_eq!(restored["restored_previous"], PREVIOUS, "{point}");
        }
        assert_eq!(stored_machine_schema(&f), old_schema, "{point}");
        prove_visible(&mut f, &previous, &shape).await;
    }

    for point in ["upgrade_restore_file", "upgrade_restored"] {
        let mut f = load_seed(&installed, seeded.cookies.clone());
        let evidence = proof(&f);
        answer(prepare(&f, &candidate, &evidence, None));
        let failed = upgrade(&f, &previous, &["recover"], Some(point));
        assert!(!failed.status.success());
        assert!(String::from_utf8_lossy(&failed.stderr).contains(point));
        answer(upgrade(&f, &previous, &["recover"], None));
        assert_eq!(stored_machine_schema(&f), old_schema);
        prove_visible(&mut f, &previous, &shape).await;
    }

    let mut compatible = load_seed(&installed, seeded.cookies.clone());
    let evidence = proof(&compatible);
    answer(prepare(&compatible, &candidate, &evidence, None));
    assert_eq!(stored_machine_schema(&compatible), old_schema + 1);
    let committed = commit(&compatible, &candidate, Some("upgrade_committed"));
    assert!(!committed.status.success());
    assert!(String::from_utf8_lossy(&committed.stderr).contains("upgrade_committed"));
    assert!(
        answer(upgrade(&compatible, &previous, &["recover"], None))["restored_previous"].is_null()
    );
    answer(upgrade(&compatible, &previous, &["verify"], None));
    prove_visible(&mut compatible, &candidate, &shape).await;
    compatible.release_binary = Some(candidate.clone());
    compatible.start().await;
    let new = compatible.op(0, "notes.create", json!({
        "space_id":shape["space"],"title":"After cutover","markdown":"Saved by the candidate.\n"
    })).await;
    new.ok();
    let new_id = new.body["data"]["note"]["id"].clone();
    compatible.stop();
    prove_visible(&mut compatible, &previous, &shape).await;
    compatible.start().await;
    let preserved = compatible.op(0, "notes.get", json!({"id":new_id})).await;
    preserved.ok();
    assert_eq!(
        preserved.body["data"]["note"]["markdown"],
        "Saved by the candidate.\n"
    );
    compatible.stop();

    let mut incompatible = load_seed(&installed, seeded.cookies.clone());
    answer(upgrade(
        &incompatible,
        &candidate,
        &[
            "prepare",
            "--previous",
            PREVIOUS,
            "--candidate",
            CANDIDATE,
            "--incompatible",
        ],
        None,
    ));
    answer(commit(&incompatible, &candidate, None));
    assert_eq!(stored_machine_schema(&incompatible), old_schema + 1);
    let before = std::fs::read(incompatible.appdata().join("system/machines.db")).unwrap();
    let refused = upgrade(&incompatible, &previous, &["verify"], None);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stdout).contains("supported range"));
    incompatible.release_binary = Some(previous);
    incompatible.start_checked(true, true).await;
    incompatible.stop();
    assert_eq!(
        before,
        std::fs::read(incompatible.appdata().join("system/machines.db")).unwrap()
    );
    prove_visible(&mut incompatible, &candidate, &shape).await;
}

#[tokio::test]
async fn release_artifacts_preserve_live_data_in_both_directions() {
    let previous = binary("CALTERNAL_RELEASE_N_BIN");
    let candidate = binary("CALTERNAL_RELEASE_NEXT_BIN");
    let previous_id = std::env::var("CALTERNAL_RELEASE_N_IMAGE").unwrap();
    let candidate_id = std::env::var("CALTERNAL_RELEASE_NEXT_IMAGE").unwrap();
    let mut f = Fixture::empty();
    let shape = seed(&mut f, &previous).await;
    let evidence = f.dir.path().join("artifact-proof.json");
    std::fs::write(
        &evidence,
        json!({
            "format":"calternal-upgrade-proof/v1",
            "previous":previous_id,
            "candidate":candidate_id
        })
        .to_string(),
    )
    .unwrap();

    answer(upgrade(
        &f,
        &candidate,
        &[
            "prepare",
            "--previous",
            &previous_id,
            "--candidate",
            &candidate_id,
            "--proof",
            evidence.to_str().unwrap(),
        ],
        None,
    ));
    answer(upgrade(
        &f,
        &candidate,
        &["commit", "--candidate", &candidate_id],
        None,
    ));
    prove_visible(&mut f, &candidate, &shape).await;
    f.release_binary = Some(candidate.clone());
    f.start().await;
    let new = f.op(0, "notes.create", json!({
        "space_id":shape["space"],"title":"After cutover","markdown":"Candidate acknowledged this.\n"
    })).await;
    new.ok();
    let id = new.body["data"]["note"]["id"].clone();

    // The candidate adds this store at version 1. The previous release must
    // still run and make writes with the candidate's store and file present.
    let space = &shape["space"];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let minute = (now / 60 + 720) % 1440;
    let words = format!("daily at {}:{:02}", minute / 60, minute % 60);
    let preview = f
        .op(
            0,
            "automations.preview_schedule",
            json!({"space_id":space,"schedule_text":words}),
        )
        .await;
    preview.ok();
    let timing = &preview.body["data"];
    let automation = f
        .op(
            0,
            "automations.create",
            json!({
                "space_id":space,"name":"Upgrade smoke test","prompt":"Verify the handoff.",
                "schedule_text":words,"schedule":timing["schedule"],"timezone":timing["timezone"],
            }),
        )
        .await;
    automation.ok();
    let automation = &automation.body["data"]["automation"];
    f.stop();
    let store = f.appdata().join("system/automations.db");
    let db =
        rusqlite::Connection::open_with_flags(&store, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let version: i64 = db
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap();
    assert_eq!(version, 1);
    drop(db);
    answer(upgrade(&f, &previous, &["verify"], None));
    prove_visible(&mut f, &previous, &shape).await;
    f.start().await;
    let restored = f.op(0, "notes.get", json!({"id":id})).await;
    restored.ok();
    assert_eq!(
        restored.body["data"]["note"]["markdown"],
        "Candidate acknowledged this.\n"
    );
    let written_by_n = f.op(0, "notes.create", json!({
        "space_id":shape["space"],"title":"After rollback","markdown":"Previous release acknowledged this.\n"
    })).await;
    written_by_n.ok();
    f.stop();
    f.release_binary = Some(candidate);
    f.start().await;
    let newer_reopened = f
        .op(
            0,
            "notes.get",
            json!({"id":written_by_n.body["data"]["note"]["id"]}),
        )
        .await;
    newer_reopened.ok();
    assert_eq!(
        newer_reopened.body["data"]["note"]["markdown"],
        "Previous release acknowledged this.\n"
    );
    let reopened = f
        .op(
            0,
            "automations.read",
            json!({"space_id": space,"id":automation["id"]}),
        )
        .await;
    reopened.ok();
    let reopened = &reopened.body["data"]["automation"];
    for field in [
        "id",
        "revision",
        "schedule",
        "schedule_text",
        "timezone",
        "next_run_ms",
    ] {
        assert_eq!(reopened[field], automation[field], "{field}");
    }
}

/// Measure from the last acknowledged write on N to the first on N+1. Run only
/// with prebuilt artifacts under the perf lock. The process test excludes Quadlet
/// and proxy overhead and reports each sample rather than setting a product SLA.
#[tokio::test]
#[ignore = "run prebuilt on perf-01 under its perf lock"]
async fn release_handoff_socket_write_gap() {
    assert_eq!(std::env::var("CALTERNAL_PERF_HELD").as_deref(), Ok("1"));
    let previous = binary("CALTERNAL_RELEASE_N_BIN");
    let candidate = binary("CALTERNAL_RELEASE_NEXT_BIN");
    for index in 0..10 {
        let mut f = Fixture::empty();
        let shape = seed(&mut f, &previous).await;
        let evidence = proof(&f);
        f.start().await;
        let write =
            |step| json!({"space_id":shape["space"],"title":step,"markdown":"Write probe.\n"});
        f.op(0, "notes.create", write("Before handoff")).await.ok();
        let start = Instant::now();
        let mut outgoing = f.child.take().unwrap();
        assert!(
            Command::new("kill")
                .args(["-TERM", &outgoing.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        assert!(outgoing.wait().unwrap().success());
        answer(prepare(&f, &candidate, &evidence, None));
        answer(commit(&f, &candidate, None));
        f.release_binary = Some(candidate.clone());
        f.start().await;
        f.op(0, "notes.create", write("After handoff")).await.ok();
        println!(
            "upgrade_write_gap_ms sample={} elapsed={:.3}",
            index + 1,
            start.elapsed().as_secs_f64() * 1000.0
        );
    }
}

#[tokio::test]
async fn checked_in_release_fixture_opens_after_upgrade() {
    let candidate = binary("CALTERNAL_RELEASE_NEXT_BIN");
    let root = source_root().join("tests/conformance/fixtures/upgrades");
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["format"], "calternal-upgrade-fixture/v1");
    let mut f = load_seed(&root.join("release-n.tar.gz"), Vec::new());
    let before = stored_machine_schema(&f);
    let evidence = proof(&f);
    answer(prepare(&f, &candidate, &evidence, None));
    answer(commit(&f, &candidate, None));
    assert_eq!(stored_machine_schema(&f), before + 1);
    f.release_binary = Some(candidate);
    f.start().await;
    let status = f
        .req("GET", "/api/v1/auth/status", None, json!({}), Some(ORIGIN))
        .await;
    status.ok();
    assert_eq!(status.body["setup"], false);
    f.stop();

    // Published fixtures intentionally have no live session material.
    let space = manifest["space"].as_str().unwrap();
    let path = f.appdata().join("spaces").join(space).join("content.db");
    let connection =
        Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let count = |id: &Value| {
        connection
            .query_row(
                "SELECT count(*) FROM versions WHERE item=?1",
                [id.as_str().unwrap()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap()
    };
    assert!(count(&manifest["note"]) >= 2);
    assert!(count(&manifest["thread"]) >= 1);
    let workspace = f
        .dir
        .path()
        .join("machine")
        .join(manifest["workspace"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(workspace).unwrap(),
        "A user file in the durable machine workspace."
    );
}
