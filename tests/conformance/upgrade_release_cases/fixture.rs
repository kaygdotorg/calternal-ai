//! Provision and reopen disposable installed data through the actual release.
use super::*;

const INSTALLATION: &str = "00000000-0000-0000-0000-000000000001";
pub(super) const PREVIOUS: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";
pub(super) const CANDIDATE: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";
const MARKDOWN: &str = "# Upgrade fixture\n\nSaved before the service handoff.\n";
const FINAL_MARKDOWN: &str =
    "# Upgrade fixture\n\nSaved before the service handoff.\n\nA subsequent saved revision.\n";
const NOTE_REQUEST: &str = "release-n-note-create";

pub(super) fn binary(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("Set {name} to the pinned executable"))
}

fn cli(f: &Fixture, binary: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .arg("--json")
        .env("CALTERNAL_INSTALLATION_ID", INSTALLATION)
        .current_dir(f.dir.path());
    command
}

pub(super) fn answer(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

pub(super) async fn seed(f: &mut Fixture, previous: &Path) -> Value {
    f.release_binary = Some(previous.to_path_buf());
    f.start().await;
    let setup = answer(
        cli(f, previous)
            .args(["setup-code", "--appdata"])
            .arg(f.appdata())
            .output()
            .unwrap(),
    );
    f.enroll(setup["code"].as_str().unwrap()).await;
    let created = f
        .op(0, "spaces.create", json!({"name":"Release fixture"}))
        .await;
    created.ok();
    let space = created.body["data"]["space"]["id"].as_str().unwrap();
    let note_input = json!({"space_id":space,"title":"Upgrade fixture","markdown":MARKDOWN});
    let saved = f
        .op_id(0, "notes.create", note_input.clone(), NOTE_REQUEST)
        .await;
    saved.ok();
    let note = saved.body["data"]["note"]["id"].as_str().unwrap();
    let updated = f
        .op_id(
            0,
            "notes.update",
            json!({
                "id":note,
                "base_revision":saved.body["data"]["note"]["revision"],
                "markdown":FINAL_MARKDOWN
            }),
            "release-n-note-update",
        )
        .await;
    updated.ok();

    let trash = f
        .op(
            0,
            "notes.create",
            json!({
                "space_id":space,"title":"Before deletion","markdown":"Kept in Trash.\n"
            }),
        )
        .await;
    trash.ok();
    let trashed = trash.body["data"]["note"]["id"].as_str().unwrap();
    f.op(0, "notes.delete", json!({"id":trashed})).await.ok();

    let invite = f
        .op(
            0,
            "invites.create",
            json!({
                "space_id":space, "role":"reader", "expires_at":null
            }),
        )
        .await;
    invite.ok();
    let invites = f.op(0, "invites.list", json!({"space_id":space})).await;
    invites.ok();
    let link = invites.body["data"]["invites"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["invite_id"] == invite.body["data"]["invite_id"])
        .unwrap()["link"]
        .as_str()
        .unwrap();
    f.enroll_named(link.rsplit('/').next().unwrap(), "Upgrade fixture reader")
        .await;
    let thread = f
        .op(
            0,
            "threads.create",
            json!({"space_id":space,"title":"Saved thread"}),
        )
        .await;
    thread.ok();
    let secret = f.op(0, "secrets.request", json!({
        "space_id":space, "name":"release-fixture", "label":"Fixture token", "reason":"Upgrade proof"
    })).await;
    secret.ok();
    f.auth(
        "secrets/provide",
        Some(&f.cookies[0]),
        json!({
            "space_id":space,
            "request_id":secret.body["data"]["request_id"],
            "value":"calternal-disposable-fixture-value"
        }),
    )
    .await
    .ok();
    let code = f
        .req(
            "POST",
            "/api/v1/machines/codes",
            Some(&f.cookies[0]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    code.ok();
    let machine_dir = f.dir.path().join("machine");
    let machine = answer(
        cli(f, previous)
            .args(["machine", "enroll", "--server", &f.base, "--dir"])
            .arg(&machine_dir)
            .env("CALTERNAL_ENROLL_CODE", code.body["code"].as_str().unwrap())
            .output()
            .unwrap(),
    );
    let relative_work = format!(
        "sandboxes/accounts/{}/threads/{}/work/Saved.md",
        f.ids[0],
        thread.body["data"]["thread"]["id"].as_str().unwrap(),
    );
    let work = machine_dir.join(&relative_work);
    std::fs::create_dir_all(work.parent().unwrap()).unwrap();
    std::fs::write(&work, "A user file in the durable machine workspace.").unwrap();
    let shape = json!({
        "format":"calternal-upgrade-fixture/v1",
        "space":space,
        "owner":f.ids[0],
        "note":note,
        "trashed":trashed,
        "thread":thread.body["data"]["thread"]["id"],
        "reader":f.ids[1],
        "machine":machine["machine_id"],
        "workspace":relative_work,
        "note_input":note_input,
    });
    f.stop();
    shape
}

pub(super) fn upgrade(
    f: &Fixture,
    executable: &Path,
    step: &[&str],
    fault: Option<&str>,
) -> std::process::Output {
    let mut command = cli(f, executable);
    command
        .args(["upgrade", "--appdata"])
        .arg(f.appdata())
        .arg("--userdata-ssd")
        .arg(f.dir.path().join("userdata-ssd"))
        .arg("--userdata-hdd")
        .arg(f.dir.path().join("userdata-hdd"))
        .arg("--machine-dir")
        .arg(f.dir.path().join("machine"))
        .args(step);
    if let Some(fault) = fault {
        command.env("CALTERNAL_FAULT", fault);
    }
    command.output().unwrap()
}

pub(super) fn proof(f: &Fixture) -> PathBuf {
    let proof = f.dir.path().join("pair-proof.json");
    std::fs::write(
        &proof,
        json!({
            "format":"calternal-upgrade-proof/v1",
            "previous":PREVIOUS,
            "candidate":CANDIDATE
        })
        .to_string(),
    )
    .unwrap();
    proof
}

pub(super) async fn prove_visible(f: &mut Fixture, executable: &Path, shape: &Value) {
    f.release_binary = Some(executable.to_path_buf());
    f.start().await;
    let note = f.op(0, "notes.get", json!({"id":shape["note"]})).await;
    note.ok();
    assert_eq!(note.body["data"]["note"]["markdown"], FINAL_MARKDOWN);
    let shared = f.op(1, "notes.get", json!({"id":shape["note"]})).await;
    shared.ok();
    assert_eq!(shared.body["data"]["note"]["markdown"], FINAL_MARKDOWN);
    let history = f.op(0, "notes.history", json!({"id":shape["note"]})).await;
    history.ok();
    assert!(
        !history.body["data"]["versions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let replay = f
        .op_id(0, "notes.create", shape["note_input"].clone(), NOTE_REQUEST)
        .await;
    replay.ok();
    assert_eq!(replay.body["data"]["note"]["id"], shape["note"]);
    let trash = f
        .op(0, "trash.list", json!({"space_id":shape["space"]}))
        .await;
    trash.ok();
    assert!(
        trash.body["data"]["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note["id"] == shape["trashed"])
    );
    let thread = f.op(0, "threads.get", json!({"id":shape["thread"]})).await;
    thread.ok();
    assert_eq!(
        thread.body["data"]["thread"]["metadata"]["title"],
        "Saved thread"
    );
    let secret = f
        .op(0, "secrets.list", json!({"space_id":shape["space"]}))
        .await;
    secret.ok();
    assert_eq!(secret.body["data"]["secrets"][0]["name"], "release-fixture");
    let credentials =
        calternal_machine::identity::Credentials::load(&f.dir.path().join("machine")).unwrap();
    assert_eq!(
        credentials.machine_id.to_string(),
        shape["machine"].as_str().unwrap()
    );
    let workspace = f
        .dir
        .path()
        .join("machine")
        .join(shape["workspace"].as_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(workspace).unwrap(),
        "A user file in the durable machine workspace."
    );
    f.stop();
}

pub(super) fn export_seed(f: &Fixture, shape: &Value, directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    std::fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(shape).unwrap(),
    )
    .unwrap();
    let status = Command::new("tar")
        .args([
            "--exclude=appdata/setup.token",
            "--exclude=appdata/startup.json",
            "--exclude=.lock",
            "--exclude=*.sock",
            "--exclude=*-shm",
            "--sort=name",
        ])
        .arg("-C")
        .arg(f.dir.path())
        .arg("-czf")
        .arg(directory.join("release-n.tar.gz"))
        .args(["appdata", "userdata-ssd", "userdata-hdd", "machine"])
        .status()
        .unwrap();
    assert!(status.success());
}

pub(super) fn load_seed(archive: &Path, cookies: Vec<String>) -> Fixture {
    let mut f = Fixture::empty();
    let status = Command::new("tar")
        .arg("-C")
        .arg(f.dir.path())
        .arg("-xzf")
        .arg(archive)
        .status()
        .unwrap();
    assert!(status.success());
    f.cookies = cookies;
    f
}

pub(super) fn stored_machine_schema(f: &Fixture) -> i64 {
    let path = f.appdata().join("system/machines.db");
    Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

pub(super) fn prepare(
    f: &Fixture,
    next: &Path,
    evidence: &Path,
    fault: Option<&str>,
) -> std::process::Output {
    upgrade(
        f,
        next,
        &[
            "prepare",
            "--previous",
            PREVIOUS,
            "--candidate",
            CANDIDATE,
            "--proof",
            evidence.to_str().unwrap(),
        ],
        fault,
    )
}

pub(super) fn commit(f: &Fixture, next: &Path, fault: Option<&str>) -> std::process::Output {
    upgrade(f, next, &["commit", "--candidate", CANDIDATE], fault)
}
