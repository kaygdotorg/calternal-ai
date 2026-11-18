//! A pending local upgrade fences the public server until recovery or cutover.
use super::*;

const INSTALLATION: &str = "00000000-0000-0000-0000-000000000001";
const PREVIOUS: &str = "1111111111111111111111111111111111111111111111111111111111111111";
const CANDIDATE: &str = "2222222222222222222222222222222222222222222222222222222222222222";

fn upgrade(f: &Fixture, args: &[&str]) -> std::process::Output {
    Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap())
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
        .args(args)
        .output()
        .unwrap()
}

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
    assert!(!upgrade(&f, &prepare).status.success());
    assert_eq!(ok(upgrade(&f, &["status"]))["phase"], "none");

    f.stop();
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
