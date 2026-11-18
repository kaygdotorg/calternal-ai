//! Cross-release upgrade proof over data written through the live service.
use super::*;

mod fixture;
use fixture::*;

#[tokio::test]
async fn release_artifacts_migrate_security_receipts_from_v3_to_v4() {
    let previous = binary("CALTERNAL_RELEASE_N_BIN");
    let candidate = binary("CALTERNAL_RELEASE_NEXT_BIN");
    let old_id = std::env::var("CALTERNAL_RELEASE_N_IMAGE").unwrap();
    let new_id = std::env::var("CALTERNAL_RELEASE_NEXT_IMAGE").unwrap();
    let mut f = Fixture::empty();
    let shape = seed(&mut f, &previous).await;
    let input = json!({"name":"Receipt proof"});
    let request = "upgrade:receipt/東京";
    f.start().await;
    let original = f.op_id(0, "spaces.create", input.clone(), request).await;
    original.ok();
    let receipt = original.body["data"].clone();
    let disposable = f
        .op(0, "spaces.create", json!({"name":"Deleted before upgrade"}))
        .await;
    disposable.ok();
    let delete_input = json!({"space_id":disposable.body["data"]["space"]["id"]});
    let deleted = f
        .op_id(0, "spaces.delete", delete_input.clone(), "delete-on-v3")
        .await;
    deleted.ok();
    f.stop();
    let root = f.dir.path().join("userdata-ssd/.calternal/recovery");
    let snapshot = root.join("snapshot.json");
    let version = || {
        serde_json::from_slice::<Value>(&std::fs::read(&snapshot).unwrap()[32..]).unwrap()["version"]
            .as_u64().unwrap()
    };
    let prepare = |f: &Fixture| {
        upgrade(
            f,
            &candidate,
            &[
                "prepare",
                "--previous",
                &old_id,
                "--candidate",
                &new_id,
                "--incompatible",
            ],
            None,
        )
    };

    assert_eq!(version(), 3);
    let baseline_snapshot = std::fs::read(&snapshot).unwrap();
    let evidence = proof(&f);
    let additive = upgrade(
        &f,
        &candidate,
        &[
            "prepare",
            "--previous",
            PREVIOUS,
            "--candidate",
            CANDIDATE,
            "--proof",
            evidence.to_str().unwrap(),
        ],
        None,
    );
    assert!(!additive.status.success());
    assert!(String::from_utf8_lossy(&additive.stdout).contains("requires an incompatible upgrade"));
    assert_eq!(std::fs::read(&snapshot).unwrap(), baseline_snapshot);

    answer(prepare(&f));
    assert_eq!(version(), 4);
    let new_snapshot: Value =
        serde_json::from_slice(&std::fs::read(&snapshot).unwrap()[32..]).unwrap();
    let temporary_segment = root.join(new_snapshot["segment"].as_str().unwrap());
    assert!(temporary_segment.is_file());
    assert_eq!(
        answer(upgrade(&f, &previous, &["recover"], None))["restored_previous"],
        old_id
    );
    assert_eq!(std::fs::read(&snapshot).unwrap(), baseline_snapshot);
    assert!(!temporary_segment.exists());
    prove_visible(&mut f, &previous, &shape).await;

    answer(prepare(&f));
    answer(upgrade(
        &f,
        &candidate,
        &["commit", "--candidate", &new_id],
        None,
    ));
    prove_visible(&mut f, &candidate, &shape).await;
    f.start().await;
    let replay = f.op_id(0, "spaces.create", input, request).await;
    replay.ok();
    assert_eq!(replay.body["data"], receipt);
    let replay_delete = f
        .op_id(0, "spaces.delete", delete_input, "delete-on-v3")
        .await;
    replay_delete.ok();
    assert_eq!(replay_delete.body["data"], deleted.body["data"]);
    let next = f
        .op_id(
            0,
            "spaces.create",
            json!({"name":"Candidate authority write"}),
            "after-upgrade",
        )
        .await;
    next.ok();
    f.stop();
    let committed = std::fs::read(&snapshot).unwrap();
    assert!(!upgrade(&f, &previous, &["verify"], None).status.success());
    f.release_binary = Some(previous);
    f.start_checked(true, true).await;
    f.stop();
    assert_eq!(std::fs::read(&snapshot).unwrap(), committed);
    f.release_binary = Some(candidate);
    f.start().await;
    let replay_next = f
        .op_id(
            0,
            "spaces.create",
            json!({"name":"Candidate authority write"}),
            "after-upgrade",
        )
        .await;
    replay_next.ok();
    assert_eq!(replay_next.body["data"], next.body["data"]);
}

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
    f.stop();
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
