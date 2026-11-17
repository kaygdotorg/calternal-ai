//! Golden tests of the `calternal` binary's own commands: human and `--json`
//! output, one error case each, JSON checked against `docs/cli-shapes.json`.
//! Snapshots live in `tests/golden/cli/`; `UPDATE_GOLDEN=1` rewrites them.
//! Volatile values (codes, ids, temp paths, times, ports) are replaced by
//! placeholders first; the time zone is forced to UTC+1 to prove local rendering.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use calternal_cli::testing::{assert_golden, assert_shape, transcript};
use serde_json::Value;

const MACHINE_A: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/cli")
        .join(name)
}

fn command(args: &[&str]) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_calternal"));
    c.args(args)
        .env("TZ", "CET-1")
        .env("NO_COLOR", "1")
        .env("RUST_LOG", "off")
        .env_remove("COLUMNS")
        .env_remove("CALTERNAL_FAULT");
    c
}

struct Ran {
    exit: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Ran {
    let o = command(args).output().unwrap();
    Ran {
        exit: o.status.code(),
        stdout: String::from_utf8(o.stdout).unwrap(),
        stderr: String::from_utf8(o.stderr).unwrap(),
    }
}

/// Run `args` in human mode and with `--json`, snapshot both as `<name>.txt` /
/// `<name>.json`, check the JSON shape (`shape` is the command's key, or `error`),
/// and return both runs. `clean` removes volatile text from a snapshot.
fn case(name: &str, args: &[&str], shape: &str, clean: &dyn Fn(&str) -> String) -> (Ran, Ran) {
    let human = run(args);
    assert_golden(
        &golden(&format!("{name}.txt")),
        &clean(&transcript(human.exit, &human.stdout, &human.stderr)),
    );
    let mut with_json = args.to_vec();
    with_json.push("--json");
    let json = run(&with_json);
    let doc: Value = serde_json::from_str(&json.stdout).expect("--json prints one JSON document");
    assert_shape(shape, &doc);
    assert!(
        json.stderr.is_empty(),
        "--json keeps stderr empty: {}",
        json.stderr
    );
    assert_eq!(doc["ok"].as_bool(), Some(json.exit == Some(0)));
    assert_golden(
        &golden(&format!("{name}.json")),
        &clean(&transcript(json.exit, &json.stdout, &json.stderr)),
    );
    (human, json)
}

fn data(dir: &Path) -> String {
    dir.to_str().unwrap().to_owned()
}

#[test]
fn machines_code_prints_the_code_once_with_a_local_expiry() {
    let d = tempfile::tempdir().unwrap();
    let dir = data(d.path());
    let (human, json) = {
        let clean = |s: &str| {
            let mut out = String::new();
            for line in s.lines() {
                if let Some(rest) = line.strip_prefix("code:") {
                    assert!(rest.trim().len() >= 8, "a code was issued");
                    out.push_str("code:     <CODE>\n");
                } else if line.starts_with("expires:") {
                    assert!(line.ends_with(" +01:00"), "local zone: {line}");
                    out.push_str("expires:  <TIME>\n");
                } else if line.trim_start().starts_with("\"code\"") {
                    out.push_str("    \"code\": \"<CODE>\",\n");
                } else if line.trim_start().starts_with("\"expires_at\"") {
                    assert!(line.contains("Z\""), "UTC in JSON: {line}");
                    out.push_str("    \"expires_at\": \"<TIME>\",\n");
                } else {
                    out.push_str(line);
                    out.push('\n');
                }
            }
            out
        };
        case(
            "machines-code-pool",
            &["machines", "code", "--pool", "--appdata", &dir],
            "calternal machines code",
            &clean,
        )
    };
    // Secrets: the code appears once per run, nowhere else.
    assert_eq!(human.stdout.matches("code:").count(), 1);
    let code = serde_json::from_str::<Value>(&json.stdout).unwrap()["data"]["code"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(json.stdout.matches(&code).count(), 1);
    // Error: neither --pool nor --account.
    case(
        "machines-code-error",
        &["machines", "code", "--appdata", &dir],
        "error",
        &|s| s.to_owned(),
    );
}

fn enrolled_data_dir() -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    let path = d.path().join("system/machines.db");
    let mut c = calternal_server::machines::db::open(&path).unwrap();
    for (id, name, os, arch, owner) in [
        (
            MACHINE_A,
            "build-box",
            "linux",
            "x86_64",
            calternal_machine::protocol::Owner::Pool,
        ),
        (
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
            "laptop",
            "macos",
            "aarch64",
            calternal_machine::protocol::Owner::Account {
                id: calternal_types::AccountId::parse("cccccccc-cccc-7ccc-8ccc-cccccccccccc")
                    .unwrap(),
            },
        ),
    ] {
        let (code, _) =
            calternal_server::machines::db::new_code(&c, owner, Duration::from_secs(60)).unwrap();
        let canon = calternal_auth::token::normalize_setup_token(&code).unwrap();
        let row = calternal_server::machines::db::redeem_code(
            &mut c,
            &calternal_auth::token::hash32(&canon),
            &[7; 32],
            name,
            os,
            arch,
        )
        .unwrap()
        .unwrap();
        // Give the rows fixed ids so the snapshots are stable.
        c.execute(
            "UPDATE machines SET id = ?1 WHERE id = ?2",
            [id, &row.id.to_string()],
        )
        .unwrap();
    }
    let dir = data(d.path());
    (d, dir)
}

#[test]
fn machines_list_revoke_and_pool_grant() {
    let (_keep, dir) = enrolled_data_dir();
    case(
        "machines-list",
        &["machines", "list", "--appdata", &dir],
        "calternal machines list",
        &|s| s.to_owned(),
    );
    case(
        "machines-revoke",
        &["machines", "revoke", MACHINE_A, "--appdata", &dir],
        "calternal machines revoke",
        &|s| s.to_owned(),
    );
    // The revoked machine now shows its state.
    case(
        "machines-list-after-revoke",
        &["machines", "list", "--appdata", &dir],
        "calternal machines list",
        &|s| s.to_owned(),
    );
    case(
        "machines-revoke-unknown",
        &[
            "machines",
            "revoke",
            "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
            "--appdata",
            &dir,
        ],
        "error",
        &|s| s.to_owned(),
    );
    case(
        "machines-revoke-bad-id",
        &["machines", "revoke", "nope", "--appdata", &dir],
        "error",
        &|s| s.to_owned(),
    );
    let account = "cccccccc-cccc-7ccc-8ccc-cccccccccccc";
    case(
        "machines-pool-grant",
        &["machines", "pool-grant", account, "--appdata", &dir],
        "calternal machines pool-grant",
        &|s| s.to_owned(),
    );
    case(
        "machines-pool-revoke",
        &[
            "machines",
            "pool-grant",
            account,
            "--remove",
            "--appdata",
            &dir,
        ],
        "calternal machines pool-grant",
        &|s| s.to_owned(),
    );
    case(
        "machines-pool-grant-error",
        &["machines", "pool-grant", "nope", "--appdata", &dir],
        "error",
        &|s| s.to_owned(),
    );
}

#[test]
fn machines_list_empty_and_unusable_data_dir() {
    let d = tempfile::tempdir().unwrap();
    let dir = data(d.path());
    case(
        "machines-list-empty",
        &["machines", "list", "--appdata", &dir],
        "calternal machines list",
        &|s| s.to_owned(),
    );
    // A regular file cannot be a data directory.
    let file = d.path().join("file");
    std::fs::write(&file, b"x").unwrap();
    let clean = |s: &str| s.replace(file.to_str().unwrap(), "<FILE>");
    case(
        "machines-list-error",
        &["machines", "list", "--appdata", file.to_str().unwrap()],
        "error",
        &clean,
    );
}

#[test]
fn plugin_digest_lists_workers_and_reports_usage_errors() {
    case(
        "plugin-digest",
        &["plugin-digest", "--plugin-root", "/nonexistent-plugins"],
        "calternal plugin-digest",
        &|s| s.to_owned(),
    );
    case(
        "plugin-digest-usage",
        &["plugin-digest", "--plugin-root"],
        "error",
        &|s| s.to_owned(),
    );
}

#[test]
fn machine_status_and_enroll_failures_say_what_to_do() {
    let d = tempfile::tempdir().unwrap();
    let dir = data(d.path());
    let clean = |s: &str| s.replace(&dir, "<DIR>");
    case(
        "machine-status-error",
        &["machine", "status", "--dir", &dir],
        "error",
        &clean,
    );
    case(
        "machine-enroll-error",
        &[
            "machine",
            "enroll",
            "--server",
            "http://127.0.0.1:1",
            "--code",
            "AAAA-BBBB",
            "--dir",
            &dir,
        ],
        "error",
        &clean,
    );
    // The retired spelling is gone, not aliased.
    let old = run(&["machine", "enrol", "--server", "x", "--code", "y"]);
    assert_eq!(old.exit, Some(2));
}

fn serve_start(extra: &[&str], json: bool) -> (String, Option<i32>) {
    let d = tempfile::tempdir().unwrap();
    let dir = data(&d.path().join("appdata"));
    let (ssd, hdd) = (
        data(&d.path().join("userdata-ssd")),
        data(&d.path().join("userdata-hdd")),
    );
    let mut args = vec![
        "serve",
        "--development",
        "--port",
        "0",
        "--plugin-root",
        "/nonexistent-plugins",
        "--appdata",
        &dir,
        "--userdata-ssd",
        &ssd,
        "--userdata-hdd",
        &hdd,
        "--create",
        "--installation-id",
        "00000000-0000-0000-0000-000000000001",
    ];
    args.extend_from_slice(extra);
    if json {
        args.push("--json");
    }
    let mut child = command(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut text = String::new();
    let started = std::time::Instant::now();
    loop {
        let mut line = String::new();
        if out.read_line(&mut line).unwrap() == 0 || started.elapsed() > Duration::from_secs(20) {
            break;
        }
        text.push_str(&line);
        let done = if json {
            serde_json::from_str::<Value>(&text).is_ok()
        } else {
            line.starts_with("startup info:")
        };
        if done {
            break;
        }
    }
    let exit = if text.is_empty() {
        child.wait().ok().and_then(|s| s.code())
    } else {
        let _ = child.kill();
        let _ = child.wait();
        Some(0)
    };
    let mut err = String::new();
    let _ = child.stderr.take().map(|mut e| e.read_to_string(&mut err));
    let text = text.replace(&dir, "<DATA>");
    let text = match text.split("127.0.0.1:").nth(1) {
        Some(rest) => {
            let port: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            text.replace(&format!("127.0.0.1:{port}"), "127.0.0.1:<PORT>")
        }
        None => text,
    };
    (transcript(exit, &text, &err), exit)
}

#[test]
fn serve_prints_one_startup_block() {
    let (human, _) = serve_start(&[], false);
    assert_golden(&golden("serve.txt"), &human);
    let (json, _) = serve_start(&[], true);
    assert_golden(&golden("serve.json"), &json);
    let body = json
        .split("--- stdout ---\n")
        .nth(1)
        .unwrap()
        .split("--- stderr ---")
        .next()
        .unwrap();
    assert_shape("calternal serve", &serde_json::from_str(body).unwrap());
}

#[test]
fn serve_failures_name_the_flag_to_fix() {
    let (human, exit) = serve_start(&["--origin", "not an origin"], false);
    assert_eq!(exit, Some(1));
    assert_golden(&golden("serve-error.txt"), &human);
    let (json, _) = serve_start(&["--origin", "not an origin"], true);
    assert_golden(&golden("serve-error.json"), &json);
    let body = json
        .split("--- stdout ---\n")
        .nth(1)
        .unwrap()
        .split("--- stderr ---")
        .next()
        .unwrap();
    assert_shape("error", &serde_json::from_str(body).unwrap());
}

#[test]
fn usage_errors_are_json_with_json() {
    case("usage-unknown-command", &["frobnicate"], "error", &|s| {
        s.to_owned()
    });
}
