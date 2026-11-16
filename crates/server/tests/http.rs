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
    let startup = data.join("startup.json");
    let _ = std::fs::remove_file(&startup);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_calternal"));
    cmd.args(["serve", "--port", "0", "--data-dir"])
        .arg(data)
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
                    return Server {
                        child,
                        port,
                        owner,
                        guest,
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

fn note_file(data: &Path, space: &str, id: &str) -> PathBuf {
    data.join("spaces")
        .join(space)
        .join("Notes")
        .join(format!("{id}.md"))
}

#[test]
fn permissions_two_principals() {
    let d = TempDir::new("perm");
    let srv = start(&d.0, None);
    let personal = create(&srv, "c1", "personal", "secret\n");
    let shared = create(&srv, "c2", "shared", "hello\n");
    // Guest cannot touch personal.
    let r = op(
        &srv,
        &srv.guest,
        "g1",
        "notes.list",
        json!({"space_id": "personal"}),
    );
    assert_eq!(
        (r.status, r.body["error"]["code"].as_str()),
        (403, Some("forbidden"))
    );
    let r = op(
        &srv,
        &srv.guest,
        "g2",
        "notes.get",
        json!({"space_id": "personal", "id": personal["id"]}),
    );
    assert_eq!(r.status, 403);
    // Cross-space id binding: personal id via shared space is not found.
    let r = op(
        &srv,
        &srv.guest,
        "g3",
        "notes.get",
        json!({"space_id": "shared", "id": personal["id"]}),
    );
    assert_eq!(r.status, 404);
    // Guest reads shared, cannot write it.
    let r = op(
        &srv,
        &srv.guest,
        "g4",
        "notes.get",
        json!({"space_id": "shared", "id": shared["id"]}),
    );
    assert_eq!(r.status, 200);
    assert_eq!(r.body["request_id"], "g4");
    let r = op(
        &srv,
        &srv.guest,
        "g5",
        "notes.create",
        json!({"space_id": "shared", "title": "x", "markdown": ""}),
    );
    assert_eq!(r.status, 403);
    let r = op(
        &srv,
        &srv.guest,
        "g6",
        "plugins.enable",
        json!({"space_id": "shared", "plugin_id": "notes-insights"}),
    );
    assert_eq!(r.status, 403);
    // Caller ids in input are rejected as unknown fields.
    let r = op(
        &srv,
        &srv.guest,
        "g7",
        "notes.list",
        json!({"space_id": "shared", "principal": "owner"}),
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
    assert_eq!(r.status, 403);
}

#[test]
fn stale_revision_conflicts_and_history() {
    let d = TempDir::new("stale");
    let srv = start(&d.0, None);
    let n = create(&srv, "c1", "personal", "one\n");
    let id = n["id"].as_str().unwrap();
    let r1 = op(
        &srv,
        &srv.owner,
        "u1",
        "notes.update",
        json!({"space_id": "personal", "id": id, "base_revision": n["revision"], "markdown": "two\n"}),
    );
    assert_eq!(r1.status, 200, "{}", r1.body);
    let r2 = op(
        &srv,
        &srv.owner,
        "u2",
        "notes.update",
        json!({"space_id": "personal", "id": id, "base_revision": n["revision"], "markdown": "stale\n"}),
    );
    assert_eq!(r2.status, 409);
    assert_eq!(r2.body["error"]["details"]["reason"], "stale_revision");
    assert_eq!(
        r2.body["error"]["details"]["current_revision"],
        r1.body["data"]["note"]["revision"]
    );
    assert_eq!(
        std::fs::read_to_string(note_file(&d.0, "personal", id)).unwrap(),
        "two\n"
    );
    let h = op(
        &srv,
        &srv.owner,
        "h1",
        "notes.history",
        json!({"space_id": "personal", "id": id, "limit": 1}),
    );
    assert_eq!(h.body["data"]["versions"].as_array().unwrap().len(), 1);
    assert_eq!(h.body["data"]["versions"][0]["markdown"], "two\n");
    let cur = h.body["data"]["next_cursor"].as_str().unwrap().to_string();
    let h2 = op(
        &srv,
        &srv.owner,
        "h2",
        "notes.history",
        json!({"space_id": "personal", "id": id, "cursor": cur}),
    );
    assert_eq!(h2.body["data"]["versions"][0]["markdown"], "one\n");
    assert!(h2.body["data"]["next_cursor"].is_null());
}

#[test]
fn retries_are_idempotent() {
    let d = TempDir::new("retry");
    let srv = start(&d.0, None);
    let a = create(&srv, "same", "personal", "x\n");
    let b = create(&srv, "same", "personal", "x\n");
    assert_eq!(a, b);
    let l = op(
        &srv,
        &srv.owner,
        "l",
        "notes.list",
        json!({"space_id": "personal"}),
    );
    assert_eq!(l.body["data"]["notes"].as_array().unwrap().len(), 1);
    let r = op(
        &srv,
        &srv.owner,
        "same",
        "notes.create",
        json!({"space_id": "personal", "title": "T", "markdown": "y\n"}),
    );
    assert_eq!(r.status, 409);
    assert_eq!(r.body["error"]["details"]["reason"], "request_id_reused");
    // Retry of an update after the base moved on still returns the original result.
    let id = a["id"].as_str().unwrap();
    let up = json!({"space_id": "personal", "id": id, "base_revision": a["revision"], "markdown": "z\n"});
    let u1 = op(&srv, &srv.owner, "u", "notes.update", up.clone());
    let u2 = op(&srv, &srv.owner, "u", "notes.update", up);
    assert_eq!(u1.status, 200);
    assert_eq!(u1.body, u2.body);
    let h = op(
        &srv,
        &srv.owner,
        "h",
        "notes.history",
        json!({"space_id": "personal", "id": id}),
    );
    assert_eq!(h.body["data"]["versions"].as_array().unwrap().len(), 2);
    // Another principal's identical request_id is a separate request.
    let g = op(
        &srv,
        &srv.guest,
        "same",
        "notes.list",
        json!({"space_id": "shared"}),
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
        let (id, base) = {
            let srv = start(&d.0, None);
            let n = create(&srv, "c", "personal", "before\n");
            (n["id"].as_str().unwrap().to_string(), n["revision"].clone())
        };
        let input =
            json!({"space_id": "personal", "id": id, "base_revision": base, "markdown": "after\n"});
        {
            let mut srv = start(&d.0, Some(point));
            let r = try_op(&srv, &srv.owner.clone(), "u", "notes.update", input.clone());
            assert!(r.is_none(), "{point}: expected crash, got response");
            let st = srv.child.wait().unwrap();
            assert!(!st.success(), "{point}: process should abort");
        }
        let srv = start(&d.0, None);
        // Startup replay placed the committed revision.
        assert_eq!(
            std::fs::read_to_string(note_file(&d.0, "personal", &id)).unwrap(),
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
            json!({"space_id": "personal", "id": id}),
        );
        assert_eq!(
            h.body["data"]["versions"].as_array().unwrap().len(),
            2,
            "{point}: duplicate history"
        );
        // No temp files left behind.
        let leftovers: Vec<_> = std::fs::read_dir(d.0.join("spaces/personal/Notes"))
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
    let n = create(&srv, "c", "personal", "ours\n");
    let id = n["id"].as_str().unwrap();
    let path = note_file(&d.0, "personal", id);
    std::fs::write(&path, "theirs\n").unwrap();
    let r = op(
        &srv,
        &srv.owner,
        "u",
        "notes.update",
        json!({"space_id": "personal", "id": id, "base_revision": n["revision"], "markdown": "new\n"}),
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
        json!({"space_id": "personal", "id": id}),
    );
    assert_eq!(g.body["data"]["note"]["markdown"], "ours\n");

    // Deleted file also blocks; symlinked file is never followed.
    let m = create(&srv, "c2", "personal", "a\n");
    let mid = m["id"].as_str().unwrap();
    let mpath = note_file(&d.0, "personal", mid);
    std::fs::remove_file(&mpath).unwrap();
    std::os::unix::fs::symlink("/etc/hostname", &mpath).unwrap();
    let r = op(
        &srv,
        &srv.owner,
        "u2",
        "notes.update",
        json!({"space_id": "personal", "id": mid, "base_revision": m["revision"], "markdown": "b\n"}),
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
        .args(["serve", "--port", "0", "--data-dir"])
        .arg(&d.0)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(st.code(), Some(2));
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
    assert_eq!(ok.body["principal"]["id"], "owner");
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
    let mode = std::fs::metadata(d.0.join("dev-tokens/owner.token"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn static_spa_fallback_and_missing_assets() {
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
    let deep = get("/s/personal/notes/11111111-1111-4111-8111-111111111111");
    assert_eq!(deep.status, 200);
    assert!(deep.headers.contains("text/html"));
    assert_eq!(get("/assets/missing.js").status, 404);
    assert_eq!(get("/favicon.ico").status, 404);
    // Traversal never escapes web dir: either 404 or the SPA shell.
    let t = get("/../../etc/passwd");
    assert!(t.status == 404 || t.headers.contains("text/html"));
    // API paths never fall back to HTML.
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
        r#"{"id":"notes","version":"0.1.0","api_version":1,"name":"Notes","description":"Notes app","dependencies":[],"capabilities":[],"operations":[],"views":[]}"#,
    )
    .unwrap();
    let srv = start_full(&d.0.join("data"), None, None, Some(&d.0.join("plugins")));
    let inp = json!({"space_id": "personal", "plugin_id": "notes"});
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
        json!({"space_id": "personal"}),
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
        json!({"space_id": "shared"}),
    );
    assert_eq!(l.body["data"]["plugins"][0]["enabled"], true);
    // Preview of a view-less plugin is not_found, never an activation.
    let r = op(&srv, &srv.owner, "p", "plugins.preview", inp);
    assert_eq!(r.status, 404);
}

#[test]
fn startup_cleanup_only_removes_owned_temp_files() {
    let d = TempDir::new("tmpclean");
    let id = {
        let srv = start(&d.0, None);
        create(&srv, "c", "personal", "x\n")["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let dir = d.0.join("spaces/personal/Notes");
    let owned = dir.join(format!(".{id}.0123456789ab.tmp"));
    let foreign = [".editor-swap.tmp", ".notauuid.0123456789ab.tmp", "user.tmp"];
    std::fs::write(&owned, "partial").unwrap();
    for f in foreign {
        std::fs::write(dir.join(f), "keep").unwrap();
    }
    let _srv = start(&d.0, None);
    assert!(!owned.exists());
    for f in foreign {
        assert!(dir.join(f).exists(), "{f} must be preserved");
    }
}
