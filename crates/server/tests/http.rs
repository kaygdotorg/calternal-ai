//! Black-box tests against the real `calternal` binary on loopback.
//! Debug builds honor CALTERNAL_FAULT for crash-point tests.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b).unwrap();
        let p = std::env::temp_dir().join(format!("calternal-test-{tag}-{}", hex::encode(b)));
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Server {
    child: Child,
    port: u16,
    owner: String,
    guest: String,
    /// The owner's space the guest cannot see, read from `describe`.
    personal: String,
    /// The space both fixture accounts belong to, read from `describe`.
    shared: String,
}

/// Space UUIDs listed by `describe` for one bearer token.
fn described_spaces(port: u16, token: &str) -> Vec<String> {
    let auth = format!("Bearer {token}");
    let r = raw(
        port,
        "GET",
        "/api/v1/describe",
        &[("Authorization", &auth)],
        None,
    )
    .unwrap();
    assert_eq!(r.status, 200, "{}", r.body);
    r.body["principal"]["spaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["id"].as_str().unwrap().to_owned())
        .collect()
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start(data: &Path, fault: Option<&str>) -> Server {
    start_with(data, fault, None)
}

fn start_with(data: &Path, fault: Option<&str>, web: Option<&Path>) -> Server {
    start_full(data, fault, web, None)
}

fn start_full(
    data: &Path,
    fault: Option<&str>,
    web: Option<&Path>,
    plugins: Option<&Path>,
) -> Server {
    let startup = data.join("appdata/startup.json");
    let _ = std::fs::remove_file(&startup);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_calternal"));
    cmd.args(["serve", "--development", "--port", "0"])
        .args(roots(data))
        .arg("--plugin-root")
        .arg(plugins.unwrap_or(Path::new("/nonexistent-test-plugins")))
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(w) = web {
        cmd.arg("--web-dir").arg(w);
    }
    match fault {
        Some(f) => cmd.env("CALTERNAL_FAULT", f),
        None => cmd.env_remove("CALTERNAL_FAULT"),
    };
    let mut child = cmd.spawn().unwrap();
    let t0 = Instant::now();
    loop {
        if let Ok(bytes) = std::fs::read(&startup)
            && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
            && v["pid"].as_u64() == Some(child.id() as u64)
        {
            {
                {
                    let url = v["base_url"].as_str().unwrap();
                    let port = url.rsplit(':').next().unwrap().parse().unwrap();
                    let owner =
                        std::fs::read_to_string(v["token_files"]["owner"].as_str().unwrap())
                            .unwrap();
                    let guest =
                        std::fs::read_to_string(v["token_files"]["guest"].as_str().unwrap())
                            .unwrap();
                    let shared = described_spaces(port, &guest).remove(0);
                    let personal = described_spaces(port, &owner)
                        .into_iter()
                        .find(|s| *s != shared)
                        .unwrap();
                    return Server {
                        child,
                        port,
                        owner,
                        guest,
                        personal,
                        shared,
                    };
                }
            }
        }
        if let Ok(Some(st)) = child.try_wait() {
            panic!("server exited early: {st}");
        }
        assert!(
            t0.elapsed() < Duration::from_secs(20),
            "server did not start"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Serve arguments for three sibling roots under `data`; `--create` pairs them on
/// first start and is harmless afterwards.
fn roots(data: &Path) -> Vec<std::ffi::OsString> {
    let mut args = Vec::new();
    for (flag, dir) in [
        ("--appdata", "appdata"),
        ("--userdata-ssd", "userdata-ssd"),
        ("--userdata-hdd", "userdata-hdd"),
    ] {
        args.push(flag.into());
        args.push(data.join(dir).into_os_string());
    }
    args.push("--create".into());
    args.push("--installation-id".into());
    args.push(INSTALLATION.into());
    args
}

/// The installation ID every fixture's server configuration pins.
const INSTALLATION: &str = "00000000-0000-0000-0000-000000000001";

struct Resp {
    status: u16,
    headers: String,
    body: Value,
}

/// Minimal HTTP/1.1 client so tests can set arbitrary Host/Origin headers.
fn raw(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&[u8]>,
) -> Option<Resp> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut req = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    let has_host = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("host"));
    if !has_host {
        req += &format!("Host: 127.0.0.1:{port}\r\n");
    }
    for (k, v) in headers {
        req += &format!("{k}: {v}\r\n");
    }
    let body = body.unwrap_or(b"");
    req += &format!("Content-Length: {}\r\n\r\n", body.len());
    s.write_all(req.as_bytes()).ok()?;
    s.write_all(body).ok()?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).ok()?;
    if out.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&out).to_string();
    let (head, rest) = text.split_once("\r\n\r\n")?;
    let status = head.split(' ').nth(1)?.parse().ok()?;
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(rest)
    } else {
        rest.to_string()
    };
    Some(Resp {
        status,
        headers: head.to_ascii_lowercase(),
        body: serde_json::from_str(&body).unwrap_or(Value::Null),
    })
}

fn dechunk(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some((len, after)) = rest.split_once("\r\n") {
        let n = usize::from_str_radix(len.trim(), 16).unwrap_or(0);
        if n == 0 {
            break;
        }
        out += &after[..n];
        rest = &after[n + 2..];
    }
    out
}

fn op(srv: &Server, token: &str, rid: &str, operation: &str, input: Value) -> Resp {
    try_op(srv, token, rid, operation, input).expect("response")
}

fn try_op(srv: &Server, token: &str, rid: &str, operation: &str, input: Value) -> Option<Resp> {
    let body = json!({"request_id": rid, "operation": operation, "input": input}).to_string();
    let auth = format!("Bearer {token}");
    raw(
        srv.port,
        "POST",
        "/api/v1/operations",
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
        ],
        Some(body.as_bytes()),
    )
}

fn create(srv: &Server, rid: &str, space: &str, md: &str) -> Value {
    let r = op(
        srv,
        &srv.owner,
        rid,
        "notes.create",
        json!({"space_id": space, "title": "T", "markdown": md}),
    );
    assert_eq!(r.status, 200, "{}", r.body);
    r.body["data"]["note"].clone()
}

fn note_dir(data: &Path, space: &str) -> PathBuf {
    data.join("userdata-ssd/spaces").join(space).join("Notes")
}
fn note_file(data: &Path, space: &str, id: &str) -> PathBuf {
    let conn = rusqlite::Connection::open_with_flags(
        data.join("appdata/spaces").join(&space).join("content.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    let relative = loop {
        let (relative, state): (String, String) = conn
            .query_row("SELECT path,mat_state FROM items WHERE id=?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        if state != "pending" {
            break relative;
        }
        assert!(
            Instant::now() < deadline,
            "userdata placement exceeded one second for {id}"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    data.join("userdata-ssd/spaces").join(space).join(relative)
}
fn note_body(path: PathBuf) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    let (_, body) = text
        .strip_prefix("---\n")
        .unwrap()
        .split_once("\n---\n")
        .unwrap();
    body.into()
}

#[test]
fn permissions_two_principals() {
    let d = TempDir::new("perm");
    let srv = start(&d.0, None);
    let personal = create(&srv, "c1", &srv.personal, "secret\n");
    let shared = create(&srv, "c2", &srv.shared, "hello\n");
    // Guest cannot touch personal.
    let r = op(
        &srv,
        &srv.guest,
        "g1",
        "notes.list",
        json!({"space_id": srv.personal}),
    );
    assert_eq!(
        (r.status, r.body["error"]["code"].as_str()),
        (404, Some("not_found"))
    );
    let r = op(
        &srv,
        &srv.guest,
        "g2",
        "notes.get",
        json!({"space_id": srv.personal, "id": personal["id"]}),
    );
    assert_eq!(r.status, 404);
    // Cross-space id binding: personal id via shared space is not found.
    let r = op(
        &srv,
        &srv.guest,
        "g3",
        "notes.get",
        json!({"space_id": srv.shared, "id": personal["id"]}),
    );
    assert_eq!(r.status, 404);
    // Guest reads shared, cannot write it.
    let r = op(
        &srv,
        &srv.guest,
        "g4",
        "notes.get",
        json!({"space_id": srv.shared, "id": shared["id"]}),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.body["request_id"], "g4");
    let r = op(
        &srv,
        &srv.guest,
        "g5",
        "notes.create",
        json!({"space_id": srv.shared, "title": "x", "markdown": ""}),
    );
    assert_eq!(r.status, 403);
    let r = op(
        &srv,
        &srv.guest,
        "g6",
        "plugins.enable",
        json!({"space_id": srv.shared, "plugin_id": "notes-insights"}),
    );
    assert_eq!(r.status, 403);
    // Caller ids in input are rejected as unknown fields.
    let r = op(
        &srv,
        &srv.guest,
        "g7",
        "notes.list",
        json!({"space_id": srv.shared, "principal": "owner"}),
    );
    assert_eq!(r.status, 400);
    // Unknown space.
    let r = op(
        &srv,
        &srv.owner,
        "o1",
        "notes.list",
        json!({"space_id": "other"}),
    );
    assert_eq!(r.status, 404);
}

#[test]
fn stale_revision_conflicts_and_history() {
    let d = TempDir::new("stale");
    let srv = start(&d.0, None);
    let n = create(&srv, "c1", &srv.personal, "one\n");
    let id = n["id"].as_str().unwrap();
    let r1 = op(
        &srv,
        &srv.owner,
        "u1",
        "notes.update",
        json!({"space_id": srv.personal, "id": id, "base_revision": n["revision"], "markdown": "two\n"}),
    );
    assert_eq!(r1.status, 200, "{}", r1.body);
    let r2 = op(
        &srv,
        &srv.owner,
        "u2",
        "notes.update",
        json!({"space_id": srv.personal, "id": id, "base_revision": n["revision"], "markdown": "stale\n"}),
    );
    assert_eq!(r2.status, 409);
    assert_eq!(r2.body["error"]["details"]["reason"], "stale_revision");
    assert_eq!(
        r2.body["error"]["details"]["current_revision"],
        r1.body["data"]["note"]["revision"]
    );
    assert_eq!(note_body(note_file(&d.0, &srv.personal, id)), "two\n");
    let h = op(
        &srv,
        &srv.owner,
        "h1",
        "notes.history",
        json!({"space_id": srv.personal, "id": id, "limit": 1}),
    );
    assert_eq!(h.body["data"]["versions"].as_array().unwrap().len(), 1);
    assert_eq!(h.body["data"]["versions"][0]["markdown"], "two\n");
    let cur = h.body["data"]["next_cursor"].as_str().unwrap().to_string();
    let h2 = op(
        &srv,
        &srv.owner,
        "h2",
        "notes.history",
        json!({"space_id": srv.personal, "id": id, "cursor": cur}),
    );
    assert_eq!(h2.body["data"]["versions"][0]["markdown"], "one\n");
    assert!(h2.body["data"]["next_cursor"].is_null());
}

#[test]
fn retries_are_idempotent() {
    let d = TempDir::new("retry");
    let srv = start(&d.0, None);
    let a = create(&srv, "same", &srv.personal, "x\n");
    let b = create(&srv, "same", &srv.personal, "x\n");
    assert_eq!(a, b);
    let l = op(
        &srv,
        &srv.owner,
        "l",
        "notes.list",
        json!({"space_id": srv.personal}),
    );
    assert_eq!(l.body["data"]["notes"].as_array().unwrap().len(), 1);
    let r = op(
        &srv,
        &srv.owner,
        "same",
        "notes.create",
        json!({"space_id": srv.personal, "title": "T", "markdown": "y\n"}),
    );
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"]["details"]["reason"], "request_id_reused");
    // Retry of an update after the base moved on still returns the original result.
    let id = a["id"].as_str().unwrap();
    let up = json!({"space_id": srv.personal, "id": id, "base_revision": a["revision"], "markdown": "z\n"});
    let u1 = op(&srv, &srv.owner, "u", "notes.update", up.clone());
    let u2 = op(&srv, &srv.owner, "u", "notes.update", up);
    assert_eq!(u1.status, 200);
    assert_eq!(u1.body, u2.body);
    let h = op(
        &srv,
        &srv.owner,
        "h",
        "notes.history",
        json!({"space_id": srv.personal, "id": id}),
    );
    assert_eq!(h.body["data"]["versions"].as_array().unwrap().len(), 2);
    // Another principal's identical request_id is a separate request.
    let g = op(
        &srv,
        &srv.guest,
        "same",
        "notes.list",
        json!({"space_id": srv.shared}),
    );
    assert_eq!(g.status, 200);
}

#[test]
fn crash_points_recover_without_duplicates() {
    for point in [
        "after_commit",
        "before_placement",
        "after_placement",
        "before_reply",
    ] {
        let d = TempDir::new(point);
        let (id, base, personal) = {
            let srv = start(&d.0, None);
            let n = create(&srv, "c", &srv.personal, "before\n");
            (
                n["id"].as_str().unwrap().to_string(),
                n["revision"].clone(),
                srv.personal.clone(),
            )
        };
        let input =
            json!({"space_id": personal, "id": id, "base_revision": base, "markdown": "after\n"});
        {
            let mut srv = start(&d.0, Some(point));
            let r = try_op(&srv, &srv.owner.clone(), "u", "notes.update", input.clone());
            if matches!(point, "after_commit" | "before_reply") {
                assert!(r.is_none(), "{point}: expected crash before reply");
            } else if let Some(reply) = r {
                assert_eq!(
                    reply.status, 200,
                    "{point}: acknowledged reply must succeed"
                );
            }
            let st = srv.child.wait().unwrap();
            assert!(!st.success(), "{point}: process should abort");
        }
        let srv = start(&d.0, None);
        // Lazy first-use recovery places the committed revision before exposure.
        let recovered = op(
            &srv,
            &srv.owner,
            "recovery-get",
            "notes.get",
            json!({"id":id}),
        );
        assert_eq!(recovered.status, 200);
        assert_eq!(
            note_body(note_file(&d.0, &srv.personal, &id)),
            "after\n",
            "{point}"
        );
        let r = op(&srv, &srv.owner, "u", "notes.update", input);
        assert_eq!(r.status, 200, "{point}: {}", r.body);
        assert_eq!(r.body["data"]["note"]["markdown"], "after\n");
        let h = op(
            &srv,
            &srv.owner,
            "h",
            "notes.history",
            json!({"space_id": srv.personal, "id": id}),
        );
        assert_eq!(
            h.body["data"]["versions"].as_array().unwrap().len(),
            2,
            "{point}: duplicate history"
        );
        // No temp files left behind.
        let leftovers: Vec<_> = std::fs::read_dir(note_dir(&d.0, &srv.personal))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "{point}");
    }
}

#[test]
fn external_edit_blocks_overwrite() {
    let d = TempDir::new("ext");
    let srv = start(&d.0, None);
    let n = create(&srv, "c", &srv.personal, "ours\n");
    let id = n["id"].as_str().unwrap();
    let path = note_file(&d.0, &srv.personal, id);
    std::fs::write(&path, "theirs\n").unwrap();
    let r = op(
        &srv,
        &srv.owner,
        "u",
        "notes.update",
        json!({"space_id": srv.personal, "id": id, "base_revision": n["revision"], "markdown": "new\n"}),
    );
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"]["details"]["reason"], "external_change");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "theirs\n");
    // Reads still serve the authoritative SQLite revision.
    let g = op(
        &srv,
        &srv.owner,
        "g",
        "notes.get",
        json!({"space_id": srv.personal, "id": id}),
    );
    assert_eq!(g.body["data"]["note"]["markdown"], "ours\n");

    // Deleted file also blocks; symlinked file is never followed.
    let m = create(&srv, "c2", &srv.personal, "a\n");
    let mid = m["id"].as_str().unwrap();
    let mpath = note_file(&d.0, &srv.personal, mid);
    std::fs::remove_file(&mpath).unwrap();
    std::os::unix::fs::symlink("/etc/hostname", &mpath).unwrap();
    let r = op(
        &srv,
        &srv.owner,
        "u2",
        "notes.update",
        json!({"space_id": srv.personal, "id": mid, "base_revision": m["revision"], "markdown": "b\n"}),
    );
    assert_eq!(r.status, 409);
    assert!(
        std::fs::symlink_metadata(&mpath)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn exclusive_data_dir_lock() {
    let d = TempDir::new("lock");
    let _srv = start(&d.0, None);
    let st = Command::new(env!("CARGO_BIN_EXE_calternal"))
        .args(["serve", "--development", "--port", "0"])
        .args(roots(&d.0))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(1));
}

#[test]
fn host_origin_and_bearer_checks() {
    let d = TempDir::new("origin");
    let srv = start(&d.0, None);
    let auth = format!("Bearer {}", srv.owner);
    let p = srv.port;
    let get = |h: &[(&str, &str)]| raw(p, "GET", "/api/v1/describe", h, None).unwrap();
    let ok = get(&[("Authorization", &auth)]);
    assert_eq!(ok.status, 200);
    let principal = uuid::Uuid::parse_str(ok.body["principal"]["id"].as_str().unwrap()).unwrap();
    assert_eq!(principal.get_version_num(), 7);
    assert_eq!(
        get(&[("Authorization", &auth)]).body["principal"]["id"],
        principal.to_string()
    );
    assert!(
        ok.headers
            .contains("content-security-policy: default-src 'self'")
    );
    assert!(ok.headers.contains("x-content-type-options: nosniff"));
    assert!(!ok.headers.contains("access-control-allow-origin"));
    assert_eq!(get(&[]).status, 401);
    assert_eq!(get(&[("Authorization", "Bearer nope")]).status, 401);
    assert_eq!(
        get(&[("Authorization", &auth), ("Host", "evil.example:80")]).status,
        403
    );
    assert_eq!(
        get(&[
            ("Authorization", &auth),
            ("Host", &format!("attacker.test:{p}"))
        ])
        .status,
        403
    );
    assert_eq!(
        get(&[("Authorization", &auth), ("Origin", "http://evil.example")]).status,
        403
    );
    assert_eq!(
        get(&[("Authorization", &auth), ("Origin", "null")]).status,
        403
    );
    let same = format!("http://127.0.0.1:{p}");
    assert_eq!(
        get(&[("Authorization", &auth), ("Origin", &same)]).status,
        200
    );
    let lh = format!("localhost:{p}");
    let lho = format!("http://localhost:{p}");
    assert_eq!(
        get(&[("Authorization", &auth), ("Host", &lh), ("Origin", &lho)]).status,
        200
    );
    // Origin must match the Host actually used.
    assert_eq!(
        get(&[("Authorization", &auth), ("Host", &lh), ("Origin", &same)]).status,
        403
    );
    // Wrong content type and malformed envelope.
    let r = raw(
        p,
        "POST",
        "/api/v1/operations",
        &[("Authorization", &auth), ("Content-Type", "text/plain")],
        Some(b"{}"),
    )
    .unwrap();
    assert_eq!(r.status, 400);
    let r = raw(
        p,
        "POST",
        "/api/v1/operations",
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
        ],
        Some(b"{nope"),
    )
    .unwrap();
    assert_eq!((r.status, r.body["request_id"].clone()), (400, Value::Null));
    let r = op(&srv, &srv.owner, "x", "nope.op", json!({}));
    assert_eq!((r.status, r.body["request_id"].as_str()), (404, Some("x")));
    // Oversized body.
    let big = vec![b' '; 2 * 1024 * 1024 + 10];
    let r = raw(
        p,
        "POST",
        "/api/v1/operations",
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
        ],
        Some(&big),
    );
    assert_eq!(r.map(|r| r.status), Some(413));
    // Token files are owner-only and never printed (stdout checked in startup).
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(d.0.join("appdata/dev-tokens/owner.token"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn static_dir_serves_files_without_spa_fallback() {
    let d = TempDir::new("web");
    let web = d.0.join("web");
    std::fs::create_dir_all(web.join("assets")).unwrap();
    std::fs::write(web.join("index.html"), "<!doctype html><title>x</title>").unwrap();
    std::fs::write(web.join("assets/app.js"), "console.log(1)").unwrap();
    let data = d.0.join("data");
    let srv = start_with(&data, None, Some(&web));
    let get = |p: &str| raw(srv.port, "GET", p, &[], None).unwrap();
    let r = get("/");
    assert_eq!(r.status, 200);
    assert!(r.headers.contains("content-security-policy"));
    assert_eq!(get("/assets/app.js").status, 200);
    assert_eq!(get("/s/personal/notes/x").status, 404);
    assert_eq!(get("/assets/missing.js").status, 404);
    // API paths never fall back to files.
    let auth = format!("Bearer {}", srv.owner);
    let r = raw(
        srv.port,
        "GET",
        "/api/v1/nope",
        &[("Authorization", &auth)],
        None,
    )
    .unwrap();
    assert_eq!((r.status, r.body["ok"].as_bool()), (404, Some(false)));
    // Static paths still require an allowed Host.
    let r = raw(srv.port, "GET", "/", &[("Host", "evil.example")], None).unwrap();
    assert_eq!(r.status, 403);
}

#[test]
fn plugin_enable_disable_receipts_survive_opposite_mutation() {
    let d = TempDir::new("plugrcpt");
    let root = d.0.join("plugins/notes");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("manifest.json"),
        r#"{"id":"notes","name":"Notes","icon":"note","version":"0.1.0","api_version":1,"kinds":[{"id":"notes","storage":{"folder":"Notes"},"claimed_text":true,"schema":{"type":"object"}}],"operations":[],"collections":[],"views":[],"attention":[],"settings":[],"schedules":[],"webhooks":[],"secrets":[],"hooks":[],"themes":[],"host_calls":[]}"#,
    )
    .unwrap();
    let srv = start_full(&d.0.join("data"), None, None, Some(&d.0.join("plugins")));
    let inp = json!({"space_id": srv.personal, "plugin_id": "notes"});
    let e1 = op(&srv, &srv.owner, "e1", "plugins.enable", inp.clone());
    assert_eq!(e1.status, 200, "{}", e1.body);
    assert_eq!(e1.body["data"]["plugin"]["enabled"], true);
    let d1 = op(&srv, &srv.owner, "d1", "plugins.disable", inp.clone());
    assert_eq!(d1.body["data"]["plugin"]["enabled"], false);
    let gen_after_disable = d1.body["data"]["plugin"]["generation"].clone();
    // Retrying the older enable returns its original result and changes nothing.
    let e1b = op(&srv, &srv.owner, "e1", "plugins.enable", inp.clone());
    assert_eq!(e1b.body, e1.body);
    let l = op(
        &srv,
        &srv.owner,
        "l",
        "plugins.list",
        json!({"space_id": srv.personal}),
    );
    let notes = &l.body["data"]["plugins"][0];
    assert_eq!(
        (notes["enabled"].clone(), notes["generation"].clone()),
        (json!(false), gen_after_disable)
    );
    // Reusing a request_id for the opposite operation is a conflict.
    let r = op(&srv, &srv.owner, "e1", "plugins.disable", inp.clone());
    assert_eq!(r.status, 409);
    // Shared space state is independent; guest cannot manage.
    let l = op(
        &srv,
        &srv.guest,
        "gl",
        "plugins.list",
        json!({"space_id": srv.shared}),
    );
    assert_eq!(l.body["data"]["plugins"][0]["enabled"], true);
    // Preview of a view-less plugin is not_found, never an activation.
    let r = op(&srv, &srv.owner, "p", "plugins.preview", inp);
    assert_eq!(r.status, 404);
}

#[test]
fn first_space_access_cleans_only_owned_temp_files() {
    let d = TempDir::new("tmpclean");
    let (id, personal) = {
        let srv = start(&d.0, None);
        let id = create(&srv, "c", &srv.personal, "x\n")["id"]
            .as_str()
            .unwrap()
            .to_string();
        (id, srv.personal.clone())
    };
    let dir = note_dir(&d.0, &personal);
    let final_name = note_file(&d.0, &personal, &id)
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let owned = dir.join(format!(".calternal-tmp-0123456789ab-{final_name}"));
    let foreign = [".editor-swap.tmp", ".notauuid.0123456789ab.tmp", "user.tmp"];
    std::fs::write(&owned, "partial").unwrap();
    for f in foreign {
        std::fs::write(dir.join(f), "keep").unwrap();
    }
    let srv = start(&d.0, None);
    assert!(
        owned.exists(),
        "startup must leave inactive spaces unopened"
    );
    let read = op(&srv, &srv.owner, "activate", "notes.get", json!({"id":id}));
    assert_eq!(read.status, 200);
    assert!(!owned.exists());
    for f in foreign {
        assert!(dir.join(f).exists(), "{f} must be preserved");
    }
}

#[test]
fn development_tokens_require_an_explicit_startup_flag() {
    let d = TempDir::new("explicit-development");
    // A normal start must not create fixture identities or token files.
    let start_default = || {
        let startup = d.0.join("appdata/startup.json");
        let _ = std::fs::remove_file(&startup);
        let child = Command::new(env!("CARGO_BIN_EXE_calternal"))
            .args(["serve", "--port", "0"])
            .args(roots(&d.0))
            .arg("--plugin-root")
            .arg("/nonexistent-test-plugins")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut child = child;
        let at = Instant::now();
        loop {
            if let Ok(bytes) = std::fs::read(&startup)
                && let Ok(v) = serde_json::from_slice::<Value>(&bytes)
                && v["pid"].as_u64() == Some(child.id() as u64)
            {
                assert_eq!(v["development_tokens"], false);
                assert_eq!(v["token_files"], json!({}));
                let port = v["base_url"]
                    .as_str()
                    .unwrap()
                    .rsplit(':')
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                return Server {
                    child,
                    port,
                    owner: String::new(),
                    guest: String::new(),
                    personal: String::new(),
                    shared: String::new(),
                };
            }
            assert!(child.try_wait().unwrap().is_none(), "default server exited");
            assert!(
                at.elapsed() < Duration::from_secs(15),
                "default startup timed out"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    let normal = start_default();
    assert!(!d.0.join("appdata/dev-tokens").exists());
    let status = raw(normal.port, "GET", "/api/v1/auth/status", &[], None).unwrap();
    assert_eq!(status.status, 200);
    assert_eq!(status.body, json!({"setup":true,"signed_in":false}));
    assert_eq!(
        std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(d.0.join("appdata/setup.token"))
                .unwrap()
                .permissions()
        ) & 0o777,
        0o600
    );
    drop(normal);
    let development = start(&d.0, None);
    let token = development.owner.clone();
    drop(development);
    let normal = start_default();
    let response = raw(
        normal.port,
        "GET",
        "/api/v1/describe",
        &[("Authorization", &format!("Bearer {token}"))],
        None,
    )
    .unwrap();
    assert_eq!(response.status, 401);
}

#[test]
fn note_storage_is_physically_per_space_and_files_have_frontmatter_identity() {
    use std::os::unix::fs::MetadataExt;
    let d = TempDir::new("content-separation");
    let srv = start(&d.0, None);
    let a = create(&srv, "a", &srv.personal, "private body");
    let b = create(&srv, "b", &srv.shared, "shared body");
    let aid = a["space_id"].as_str().unwrap();
    let bid = b["space_id"].as_str().unwrap();
    let pa = d.0.join("appdata/spaces").join(aid).join("content.db");
    let pb = d.0.join("appdata/spaces").join(bid).join("content.db");
    assert!(pa.is_file() && pb.is_file());
    assert_ne!(
        std::fs::metadata(&pa).unwrap().ino(),
        std::fs::metadata(&pb).unwrap().ino()
    );
    for (path, own, foreign) in [(&pa, &a, &b), (&pb, &b, &a)] {
        let db =
            rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
                .unwrap();
        assert!(
            db.query_row(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1)",
                [own["id"].as_str().unwrap()],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
        );
        assert!(
            !db.query_row(
                "SELECT EXISTS(SELECT 1 FROM items WHERE id=?1)",
                [foreign["id"].as_str().unwrap()],
                |r| r.get::<_, bool>(0)
            )
            .unwrap()
        );
    }
    // No installation-wide state database holds content or plugin state.
    assert!(!d.0.join("appdata/state.sqlite3").exists());
    let path = note_file(&d.0, aid, a["id"].as_str().unwrap());
    assert_eq!(path.file_name().unwrap(), "T.md");
    let bytes = std::fs::read_to_string(path).unwrap();
    assert!(bytes.starts_with(&format!(
        "---\ncalternal-id: {}\n---\n",
        a["id"].as_str().unwrap()
    )));
    assert!(
        d.0.join("appdata/spaces")
            .join(aid)
            .join("index.db")
            .is_file()
    );
}

#[test]
fn interrupted_create_and_title_rename_recover_fixed_paths() {
    for point in ["after_commit", "before_placement", "after_placement"] {
        let d = TempDir::new(&format!("create-{point}"));
        {
            let mut srv = start(&d.0, Some(point));
            let response = try_op(
                &srv,
                &srv.owner.clone(),
                "create",
                "notes.create",
                json!({"space_id": srv.personal,"title":"First","markdown":"one"}),
            );
            if point == "after_commit" {
                assert!(response.is_none());
            } else if let Some(reply) = response {
                assert_eq!(reply.status, 200);
            }
            assert!(!srv.child.wait().unwrap().success());
        }
        let (id, revision, personal) = {
            let srv = start(&d.0, None);
            let response = op(
                &srv,
                &srv.owner,
                "create",
                "notes.create",
                json!({"space_id": srv.personal,"title":"First","markdown":"one"}),
            );
            assert_eq!(response.status, 200, "{}", response.body);
            let note = &response.body["data"]["note"];
            let id = note["id"].as_str().unwrap().to_owned();
            assert_eq!(
                note_file(&d.0, &srv.personal, &id).file_name().unwrap(),
                "First.md"
            );
            (id, note["revision"].clone(), srv.personal.clone())
        };
        let input = json!({"space_id": personal,"id":id,"base_revision":revision,"title":"FIRST","markdown":"two"});
        {
            let mut srv = start(&d.0, Some(point));
            let reply = try_op(
                &srv,
                &srv.owner.clone(),
                "rename",
                "notes.update",
                input.clone(),
            );
            if point == "after_commit" {
                assert!(reply.is_none());
            } else if let Some(reply) = reply {
                assert_eq!(reply.status, 200);
            }
            assert!(!srv.child.wait().unwrap().success());
        }
        let srv = start(&d.0, None);
        let response = op(&srv, &srv.owner, "rename", "notes.update", input);
        assert_eq!(response.status, 200, "{}", response.body);
        let path = note_file(&d.0, &srv.personal, &id);
        assert_eq!(path.file_name().unwrap(), "FIRST.md");
        assert!(!path.with_file_name("First.md").exists());
        assert_eq!(note_body(path), "two");
        let history = op(
            &srv,
            &srv.owner,
            "history",
            "notes.history",
            json!({"space_id": srv.personal,"id":id}),
        );
        assert_eq!(
            history.body["data"]["versions"].as_array().unwrap().len(),
            2
        );
    }
}

/// Starts `serve` on explicit roots and returns its exit status, or `None` if it
/// was still serving after five seconds (then it is killed).
fn serve_status(
    installation: Option<&str>,
    appdata: &Path,
    ssd: &Path,
    hdd: &Path,
    create: bool,
) -> Option<i32> {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_calternal"));
    cmd.env_remove("CALTERNAL_INSTALLATION_ID");
    cmd.args(["serve", "--development", "--port", "0", "--appdata"])
        .arg(appdata)
        .arg("--userdata-ssd")
        .arg(ssd)
        .arg("--userdata-hdd")
        .arg(hdd)
        .arg("--plugin-root")
        .arg("/nonexistent-test-plugins")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if create {
        cmd.arg("--create");
    }
    if let Some(id) = installation {
        cmd.args(["--installation-id", id]);
    }
    let mut child = cmd.spawn().unwrap();
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(5) {
        if let Some(status) = child.try_wait().unwrap() {
            return status.code();
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    None
}

/// The roots guard runs before anything is created, against the installation ID
/// pinned in configuration: a missing pin, empty roots without `--create`, swapped
/// roots, a different pin and a root of another installation are all refused, and a
/// refused start leaves no database behind.
#[test]
fn server_refuses_empty_swapped_or_foreign_roots() {
    let d = TempDir::new("roots");
    let (appdata, ssd, hdd) = (
        d.0.join("appdata"),
        d.0.join("userdata-ssd"),
        d.0.join("userdata-hdd"),
    );
    for dir in [&appdata, &ssd, &hdd] {
        std::fs::create_dir_all(dir).unwrap();
    }
    let pin = Some(INSTALLATION);
    assert_eq!(
        serve_status(None, &appdata, &ssd, &hdd, true),
        Some(1),
        "no pin"
    );
    assert_eq!(
        serve_status(pin, &appdata, &ssd, &hdd, false),
        Some(1),
        "empty roots"
    );
    assert!(
        !appdata.join("system").exists(),
        "a refused start created state"
    );
    // First setup, then a normal restart, both serve.
    drop(start(&d.0, None));
    assert_eq!(
        serve_status(pin, &appdata, &ssd, &hdd, false),
        None,
        "paired restart"
    );
    let other_pin = Some("00000000-0000-0000-0000-000000000002");
    assert_eq!(
        serve_status(other_pin, &appdata, &ssd, &hdd, false),
        Some(1),
        "different pin"
    );
    // Swapped userdata roots carry the wrong role markers.
    assert_eq!(
        serve_status(pin, &appdata, &hdd, &ssd, false),
        Some(1),
        "swapped roots"
    );
    assert_eq!(
        serve_status(pin, &appdata, &hdd, &ssd, true),
        Some(1),
        "--create cannot repair"
    );
    // A userdata root of another installation carries another installation ID.
    let other = TempDir::new("roots-other");
    let (oa, os, oh) = (
        other.0.join("appdata"),
        other.0.join("userdata-ssd"),
        other.0.join("userdata-hdd"),
    );
    assert_eq!(
        serve_status(other_pin, &oa, &os, &oh, true),
        None,
        "other first setup"
    );
    assert_eq!(
        serve_status(pin, &appdata, &os, &hdd, false),
        Some(1),
        "foreign root"
    );
    // The recovery log lives in userdata-ssd, owner-only.
    let log = ssd.join(".calternal/recovery");
    assert!(
        log.join("snapshot.json").is_file(),
        "recovery log provisioned"
    );
}
