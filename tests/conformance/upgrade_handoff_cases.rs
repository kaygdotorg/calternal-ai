//! A pending local upgrade fences the public server until recovery or cutover.
use super::*;

const INSTALLATION: &str = "00000000-0000-0000-0000-000000000001";
const PREVIOUS: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const CANDIDATE: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn upgrade(f: &Fixture, args: &[&str]) -> std::process::Output {
    upgrade_command(f, args).output().unwrap()
}

fn upgrade_command(f: &Fixture, args: &[&str]) -> Command {
    let mut command = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap());
    command
        .args([
            "--json",
            "upgrade",
            "--installation-id",
            INSTALLATION,
            "--appdata",
        ])
        .arg(f.appdata())
        .arg("--userdata-ssd")
        .arg(f.dir.path().join("userdata-ssd"))
        .arg("--userdata-hdd")
        .arg(f.dir.path().join("userdata-hdd"))
        .args(args);
    command
}

#[track_caller]
fn ok(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

#[tokio::test]
async fn a_pending_handoff_fences_startup_until_it_is_recovered_or_committed() {
    let mut f = Fixture::new().await;
    let space = f.spaces[3].clone();
    let note = f.note(1, &space).await;
    let proof = f.dir.path().join("proof.json");
    std::fs::write(
        &proof,
        json!({
            "format": "calternal-upgrade-proof/v1",
            "previous": PREVIOUS,
            "candidate": CANDIDATE
        })
        .to_string(),
    )
    .unwrap();
    let prepare = [
        "prepare",
        "--previous",
        PREVIOUS,
        "--candidate",
        CANDIDATE,
        "--proof",
        proof.to_str().unwrap(),
    ];
    let mut preflight = prepare;
    preflight[0] = "preflight";
    let valid = std::fs::read(&proof).unwrap();
    std::fs::write(&proof, b"malformed").unwrap();
    assert!(!upgrade(&f, &preflight).status.success());
    f.op(1, "notes.get", json!({"id":note["id"]})).await.ok();
    std::fs::write(&proof, &valid).unwrap();
    assert_eq!(ok(upgrade(&f, &preflight))["verified"], true);
    assert!(!upgrade(&f, &prepare).status.success());
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");

    f.stop();
    // The file may change after preflight. Locked preparation must reread it
    // before publishing a restore point or opening a migration transaction.
    std::fs::write(&proof, b"malformed").unwrap();
    assert!(!upgrade(&f, &prepare).status.success());
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");
    std::fs::write(&proof, valid).unwrap();
    assert!(!upgrade(&f, &prepare[..5]).status.success());
    assert_eq!(ok(upgrade(&f, &prepare))["phase"], "preparing");
    f.start_checked(true, true).await;
    f.stop();
    let releases = f.dir.path().join("releases/current");
    std::fs::create_dir_all(&releases).unwrap();
    std::os::unix::fs::symlink(
        std::env::var_os("CALTERNAL_SERVER_BIN").unwrap(),
        releases.join("calternal"),
    )
    .unwrap();
    std::fs::write(
        f.dir.path().join("server.env"),
        format!("CALTERNAL_INSTALLATION_ID={INSTALLATION}\n"),
    )
    .unwrap();
    let recovery = Command::new("bash")
        .arg(source_root().join("deploy/recover-start.sh"))
        .arg(f.dir.path())
        .arg(f.dir.path().join("userdata-hdd"))
        .output()
        .unwrap();
    assert!(
        recovery.status.success(),
        "{}",
        String::from_utf8_lossy(&recovery.stderr)
    );
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");
    f.start().await;
    f.op(1, "notes.get", json!({"id":note["id"]})).await.ok();

    f.stop();
    ok(upgrade(&f, &prepare));
    assert!(
        !upgrade(&f, &["commit", "--candidate", PREVIOUS])
            .status
            .success()
    );
    assert_eq!(
        ok(upgrade(&f, &["commit", "--candidate", CANDIDATE]))["phase"],
        "committed"
    );
    assert!(ok(upgrade(&f, &["recover"]))["restored_previous"].is_null());
    f.start().await;
    f.op(1, "notes.get", json!({"id":note["id"]})).await.ok();
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn full_preflight_refusal_and_locked_recheck_leave_no_intent() {
    let mut f = Fixture::new().await;
    let ssd = f.dir.path().join("userdata-ssd");
    let outside = f.dir.path().join("outside");
    std::fs::write(&outside, b"external inode").unwrap();
    let link = ssd.join("outside-alias");
    std::fs::hard_link(&outside, &link).unwrap();
    let mut command = [
        "preflight",
        "--previous",
        PREVIOUS,
        "--candidate",
        CANDIDATE,
        "--incompatible",
    ];
    let refused = upgrade(&f, &command);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stdout).contains("outside-alias"));
    let space = f.spaces[3].clone();
    f.note(1, &space).await;
    std::fs::remove_file(&link).unwrap();
    assert_eq!(ok(upgrade(&f, &command))["verified"], true);

    f.stop();
    std::fs::hard_link(&outside, &link).unwrap();
    command[0] = "prepare";
    let refused = upgrade(&f, &command);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stdout).contains("outside-alias"));
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");
    assert_eq!(std::fs::read(&outside).unwrap(), b"external inode");
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn full_intent_replays_both_sides_of_cutover_through_the_pinned_binary() {
    let mut f = Fixture::new().await;
    f.stop();
    let root = f.dir.path();
    let ssd = root.join("userdata-ssd");
    let path = ssd.join("upgrade-link");
    let target = "/usr/bin/python3";
    std::os::unix::fs::symlink(target, &path).unwrap();
    let prepare = [
        "prepare",
        "--previous",
        PREVIOUS,
        "--candidate",
        CANDIDATE,
        "--incompatible",
    ];
    let commit = ["commit", "--candidate", CANDIDATE];
    let crash = |args: &[&str], fault: &str| {
        let output = upgrade_command(&f, args)
            .env("CALTERNAL_FAULT", fault)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&format!("injected fault at {fault}")),
            "{:?}",
            output
        );
    };
    let releases = root.join("releases");
    let old = releases.join("release.old");
    let staged = releases.join("release.candidate");
    std::fs::create_dir_all(&old).unwrap();
    std::fs::create_dir(&staged).unwrap();
    std::fs::write(old.join("image.id"), PREVIOUS).unwrap();
    std::fs::write(staged.join("image.id"), CANDIDATE).unwrap();
    std::fs::write(
        staged.join("probe.name"),
        format!("calternal-ai-{}-upgrade-probe", uuid::Uuid::now_v7()),
    )
    .unwrap();
    // Only the pinned candidate has the v2 reader. A hook that runs the
    // current binary first cannot recover the v2 pending intent.
    std::os::unix::fs::symlink(
        std::env::var_os("CALTERNAL_SERVER_BIN").unwrap(),
        staged.join("calternal"),
    )
    .unwrap();
    std::os::unix::fs::symlink("release.old", releases.join("current")).unwrap();
    std::os::unix::fs::symlink("release.candidate", releases.join("pending-recovery")).unwrap();
    std::fs::write(
        root.join("server.env"),
        format!("CALTERNAL_INSTALLATION_ID={INSTALLATION}\n"),
    )
    .unwrap();

    crash(&prepare, "upgrade_copied");
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "copying");
    ok(upgrade(&f, &["recover"]));
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");
    assert_eq!(
        std::fs::read_link(&path).unwrap(),
        std::path::Path::new(target)
    );

    crash(&prepare, "upgrade_intent");
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "preparing");
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"candidate replacement").unwrap();
    crash(&["recover"], "upgrade_restore_file");
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "preparing");
    let recovery = Command::new("bash")
        .arg(source_root().join("deploy/recover-start.sh"))
        .arg(root)
        .arg(root.join("userdata-hdd"))
        .output()
        .unwrap();
    assert!(recovery.status.success(), "{:?}", recovery);
    assert_eq!(
        std::fs::read_link(&path).unwrap(),
        std::path::Path::new(target)
    );

    ok(upgrade(&f, &prepare));
    crash(&commit, "upgrade_committed");
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "committed");
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"live candidate data").unwrap();
    ok(upgrade(&f, &["recover"]));
    assert_eq!(std::fs::read(path).unwrap(), b"live candidate data");
    let intent: Value = serde_json::from_slice(
        &std::fs::read(f.appdata().join("system/upgrade-intent.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(intent["format"], "calternal-upgrade-intent/v1");
    assert!(intent.get("entries").is_none());
    assert_eq!(ok(upgrade(&f, &["status"]))["needs_recovery"], false);
}

#[tokio::test]
async fn upgrade_account_count_checks_stopped_auth_without_disclosing_records() {
    let mut fresh = Fixture::empty();
    fresh.start().await;
    assert!(!upgrade(&fresh, &["account-count"]).status.success());
    fresh.stop();
    assert_eq!(ok(upgrade(&fresh, &["account-count"])), json!({"accounts":0}));

    let mut enrolled = Fixture::new().await;
    enrolled.stop();
    assert_eq!(
        ok(upgrade(&enrolled, &["account-count"])),
        json!({"accounts":enrolled.ids.len()})
    );
}
