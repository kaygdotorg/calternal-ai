//! Live local setup-code admission and first-owner registration.
use super::*;

const INSTALLATION: &str = "00000000-0000-0000-0000-000000000001";

fn operator(f: &Fixture, installation: &str) -> Command {
    let mut command = Command::new(std::env::var_os("CALTERNAL_SERVER_BIN").unwrap());
    command
        .args(["setup-code", "--json", "--appdata"])
        .arg(f.appdata())
        .args(["--installation-id", installation])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn code(output: std::process::Output) -> String {
    assert!(output.status.success(), "setup-code failed");
    assert!(output.stderr.is_empty(), "setup-code wrote diagnostics");
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
    let data = &reply["data"];
    assert_eq!(data["url"], format!("{ORIGIN}/"));
    assert!(data["expires_at"].as_str().is_some_and(|s| s.contains('T')));
    data["code"].as_str().unwrap().to_owned()
}

async fn refuses(f: &Fixture, token: &str) {
    let result = f
        .auth(
            "setup/start",
            None,
            json!({"token":token,"name":"Expired owner"}),
        )
        .await;
    assert!(result.status >= 400, "an invalidated code was accepted");
}

#[tokio::test]
async fn setup_code_rotates_live_and_startup_preserves_the_latest_valid_file() {
    let mut f = Fixture::empty();
    std::fs::write(f.dir.path().join("capture-setup-logs"), []).unwrap();
    f.start().await;
    let path = f.appdata().join("setup.token");
    let startup = std::fs::read_to_string(&path).unwrap();
    let wrong = operator(&f, "00000000-0000-0000-0000-000000000002")
        .output()
        .unwrap();
    assert!(!wrong.status.success());
    assert!(std::fs::read_to_string(&path).unwrap() == startup);
    let public = f
        .req("POST", "/api/v1/setup-code", None, json!({}), Some(ORIGIN))
        .await;
    assert_eq!(public.status, 401);

    let first = operator(&f, INSTALLATION).spawn().unwrap();
    let second = operator(&f, INSTALLATION).spawn().unwrap();
    let first = code(first.wait_with_output().unwrap());
    let second = code(second.wait_with_output().unwrap());
    assert!(first != second && first != startup && second != startup);
    let current = std::fs::read_to_string(&path).unwrap();
    assert!(current == first || current == second);
    let replaced = if current == first { &second } else { &first };
    refuses(&f, &startup).await;
    refuses(&f, replaced).await;

    use std::os::unix::fs::MetadataExt;
    let inode = std::fs::metadata(&path).unwrap().ino();
    f.stop();
    f.start().await;
    assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
    assert!(std::fs::read_to_string(&path).unwrap() == current);
    f.enroll(&current).await;
    let public = f
        .req(
            "POST",
            "/api/v1/setup-code",
            Some(&f.cookies[0]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(public.status, 404);
    let done = operator(&f, INSTALLATION).output().unwrap();
    assert_eq!(done.status.code(), Some(1));
    let reply: Value = serde_json::from_slice(&done.stdout).unwrap();
    assert_eq!(reply["error"]["code"], "setup_complete");
    assert!(reply["data"].is_null());
    assert!(done.stderr.is_empty());
    assert!(std::fs::read_to_string(&path).unwrap() == current);

    f.stop();
    for name in ["server.stdout", "server.stderr"] {
        let log = std::fs::read_to_string(f.dir.path().join(name)).unwrap();
        for secret in [&startup, &first, &second] {
            assert!(
                !log.contains(secret),
                "a credential appeared in server logs"
            );
        }
    }
    let offline = operator(&f, INSTALLATION).output().unwrap();
    assert!(!offline.status.success());
    let reply: Value = serde_json::from_slice(&offline.stdout).unwrap();
    assert_eq!(reply["error"]["code"], "server_unavailable");
}
