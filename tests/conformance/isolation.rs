//! Isolation and authorization conformance tests against a disposable server. Only fixture provisioning and offline expiry
//! injection use protected local APIs/SQL; every asserted conformance check uses TCP HTTP.
//! All data is disposable; no existing listener or data directory is used.
//! Development-token support is exercised only by its explicit opt-in regression.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
use calternal_auth::{Auth, AuthConfig};
use reqwest::{Client, Method};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, RequestChallengeResponse};

mod agent_cases;
/// Plugin job cases: streams, restart, cancellation, sign-out and space isolation.
#[path = "jobs_cases.rs"]
mod jobs_cases;
mod people_cases;

mod requests_cases;
mod requests_review_cases;
mod threads_cases;

mod drafts_cases;
#[path = "live_cases.rs"]
mod live_cases;
mod notes_cases;
mod perf_cases;
mod simplify_review_cases;
mod worker_cases;

const ORIGIN: &str = "http://localhost:18080";
const AUTH_ROUTES: &[(&str, &str)] = &[
    ("GET", "status"),
    ("POST", "setup/start"),
    ("POST", "setup/finish"),
    ("POST", "login/start"),
    ("POST", "login/finish"),
    ("POST", "reverify/start"),
    ("POST", "reverify/finish"),
    ("POST", "logout"),
    ("GET", "me"),
    ("GET", "installations"),
    ("POST", "installations/revoke"),
    ("GET", "passkeys"),
    ("POST", "passkeys/add/start"),
    ("POST", "passkeys/add/finish"),
    ("POST", "passkeys/rename"),
    ("POST", "passkeys/remove"),
];
struct Answer {
    status: u16,
    body: Value,
    headers: BTreeMap<String, String>,
    micros: u128,
}
impl Answer {
    fn ok(&self) {
        assert_eq!(self.status, 200, "{}", self.body);
    }
    fn cookie(&self) -> String {
        self.headers["set-cookie"].split(';').next().unwrap().into()
    }
    fn signature(&self) -> Value {
        let mut body = self.body.clone();
        if let Some(e) = body.get_mut("error").and_then(Value::as_object_mut) {
            e.remove("request_id");
        }
        if let Some(o) = body.as_object_mut() {
            o.remove("request_id");
        }
        let mut h = self.headers.clone();
        h.remove("date");
        // Request IDs are random correlation metadata, not target-dependent differences.
        json!({"status":self.status,"body":body,"headers":h})
    }
}
struct Device {
    key: WebauthnAuthenticator<SoftPasskey>,
    cred: Value,
    user: Value,
}
impl Device {
    fn new() -> Self {
        Self {
            key: WebauthnAuthenticator::new(SoftPasskey::new(true)),
            cred: Value::Null,
            user: Value::Null,
        }
    }
    fn register(&mut self, start: &Answer) -> Value {
        let mut opts = start.body["options"].clone();
        assert_eq!(
            opts["publicKey"]["authenticatorSelection"]["residentKey"],
            "required"
        );
        opts["publicKey"]["authenticatorSelection"]["residentKey"] = json!("discouraged");
        opts["publicKey"]["authenticatorSelection"]["requireResidentKey"] = json!(false);
        self.user = opts["publicKey"]["user"]["id"].clone();
        let c: CreationChallengeResponse = serde_json::from_value(opts).unwrap();
        let v = serde_json::to_value(
            self.key
                .do_registration(url::Url::parse(ORIGIN).unwrap(), c)
                .unwrap(),
        )
        .unwrap();
        self.cred = v["rawId"].clone();
        v
    }
    fn sign(&mut self, start: &Answer) -> Value {
        let mut opts = start.body["options"].clone();
        opts["publicKey"]["allowCredentials"] = json!([{"type":"public-key","id":self.cred}]);
        let c: RequestChallengeResponse = serde_json::from_value(opts).unwrap();
        let mut v = serde_json::to_value(
            self.key
                .do_authentication(url::Url::parse(ORIGIN).unwrap(), c)
                .unwrap(),
        )
        .unwrap();
        v["response"]["userHandle"] = self.user.clone();
        v
    }
}
struct Fixture {
    dir: tempfile::TempDir,
    child: Option<Child>,
    client: Client,
    base: String,
    cookies: Vec<String>,
    ids: Vec<String>,
    devices: Vec<Device>,
    spaces: Vec<String>,
    development: bool,
    remote_workers: bool,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop();
    }
}
impl Fixture {
    /// The server's appdata root; userdata-ssd and userdata-hdd are its siblings.
    fn appdata(&self) -> PathBuf {
        self.dir.path().join("appdata")
    }
    fn stop(&mut self) {
        if let Some(mut p) = self.child.take() {
            let _ = p.kill();
            let _ = p.wait();
        }
    }
    async fn start(&mut self) {
        self.start_with_plugin_root(true).await;
    }

    async fn start_with_plugin_root(&mut self, explicit: bool) {
        let path = self.appdata().join("startup.json");
        let _ = std::fs::remove_file(&path);
        let binary = std::env::var("CALTERNAL_SERVER_BIN")
            .expect("set CALTERNAL_SERVER_BIN to built dev-VM binary");
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut command = Command::new(binary);
        command.env_clear();
        if self.dir.path().join("node-gate").is_file() {
            command
                .arg("serve")
                .arg("--node")
                .arg(self.dir.path().join("node-gate"));
        } else {
            command.arg("serve");
        }
        if !self.remote_workers {
            command.env("CALTERNAL_WORKER_TEST_BACKEND", "direct");
        }
        if self.development {
            command.arg("--development");
        }
        if explicit {
            command
                .arg("--plugin-root")
                .arg(repo.join("plugins"))
                .current_dir(&repo);
        } else {
            command.current_dir(self.dir.path());
        }
        let crash =
            std::fs::read_to_string(self.dir.path().join("content-fault")).unwrap_or_else(|_| {
                if self.dir.path().join("thread-send-fault").is_file() {
                    "after_thread_message".into()
                } else {
                    String::new()
                }
            });
        self.child = Some(
            command
                .args(["--port", "0", "--origin", ORIGIN, "--create"])
                .args(["--installation-id", "00000000-0000-0000-0000-000000000001"])
                .arg("--appdata")
                .arg(self.appdata())
                .arg("--userdata-ssd")
                .arg(self.dir.path().join("userdata-ssd"))
                .arg("--userdata-hdd")
                .arg(self.dir.path().join("userdata-hdd"))
                // The binary is built with `test-plugins`; see README.
                .arg("--test-plugin-root")
                .arg(repo.join("tests/plugins"))
                .env("PATH", std::env::var("PATH").unwrap_or_default())
                .env("LANG", "C.UTF-8")
                .env("RUST_LOG", "warn")
                .env("CALTERNAL_FAULT", crash)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::inherit())
                .spawn()
                .unwrap(),
        );
        for _ in 0..1000 {
            assert!(
                self.child.as_mut().unwrap().try_wait().unwrap().is_none(),
                "server exited during startup"
            );
            if let Ok(s) = std::fs::read(&path) {
                if let Ok(v) = serde_json::from_slice::<Value>(&s) {
                    assert_eq!(v["development_tokens"], self.development);
                    // Connect to actual ephemeral listener; Host/Origin are the explicitly configured RP.
                    // startup base_url is public origin, so discover bound port from /proc socket tables below.
                    self.base = format!(
                        "http://127.0.0.1:{}",
                        listener_port(self.child.as_ref().unwrap().id())
                    );
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("startup timeout");
    }
    async fn req(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: Value,
        origin: Option<&str>,
    ) -> Answer {
        self.raw(method, path, cookie, body.to_string(), origin)
            .await
    }
    async fn raw(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: String,
        origin: Option<&str>,
    ) -> Answer {
        let mut r = self
            .client
            .request(
                Method::from_bytes(method.as_bytes()).unwrap(),
                format!("{}{}", self.base, path),
            )
            .header("host", "localhost:18080")
            .header("content-type", "application/json");
        if let Some(c) = cookie {
            r = r.header("cookie", c);
        }
        if let Some(o) = origin {
            r = r.header("origin", o);
        }
        let now = Instant::now();
        let r = r.body(body).send().await.unwrap();
        let status = r.status().as_u16();
        let headers = r
            .headers()
            .keys()
            .map(|key| {
                let values = r
                    .headers()
                    .get_all(key)
                    .iter()
                    .map(|v| v.to_str().unwrap())
                    .collect::<Vec<_>>()
                    .join("\n");
                (key.to_string(), values)
            })
            .collect();
        let text = r.text().await.unwrap();
        Answer {
            status,
            body: serde_json::from_str(&text).unwrap_or(json!({"raw":text})),
            headers,
            micros: now.elapsed().as_micros(),
        }
    }
    async fn auth(&self, name: &str, cookie: Option<&str>, body: Value) -> Answer {
        self.req(
            "POST",
            &format!("/api/v1/auth/{name}"),
            cookie,
            body,
            Some(ORIGIN),
        )
        .await
    }
    async fn op_id(&self, who: usize, name: &str, input: Value, rid: &str) -> Answer {
        self.req(
            "POST",
            "/api/v1/operations",
            Some(&self.cookies[who]),
            json!({"request_id":rid,"operation":name,"input":input}),
            Some(ORIGIN),
        )
        .await
    }
    async fn op(&self, who: usize, name: &str, input: Value) -> Answer {
        self.op_id(who, name, input, &uuid::Uuid::now_v7().to_string())
            .await
    }
    async fn login(&mut self, who: usize) -> Answer {
        let s = self.auth("login/start", None, json!({})).await;
        s.ok();
        let cred = self.devices[who].sign(&s);
        self.auth(
            "login/finish",
            None,
            json!({"ceremony":s.body["ceremony"],"credential":cred,"device":"conformance-login"}),
        )
        .await
    }
    async fn enroll(&mut self, token: &str) {
        self.enroll_named(token, "conformance account").await;
    }
    async fn enroll_named(&mut self, token: &str, name: &str) {
        let s = self
            .auth("setup/start", None, json!({"token":token,"name":name}))
            .await;
        s.ok();
        let mut d = Device::new();
        let cred = d.register(&s);
        let fin = self
            .auth(
                "setup/finish",
                None,
                json!({"ceremony":s.body["ceremony"],"credential":cred,"device":"conformance-setup"}),
            )
            .await;
        fin.ok();
        self.cookies.push(fin.cookie());
        self.devices.push(d);
        let i = self.devices.len() - 1;
        let login = self.login(i).await;
        login.ok();
        self.cookies[i] = login.cookie();
        let me = self
            .req(
                "GET",
                "/api/v1/auth/me",
                Some(&self.cookies[i]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        me.ok();
        self.ids
            .push(me.body["user"]["id"].as_str().unwrap().into());
    }
    async fn new() -> Self {
        let mut f = Self {
            dir: tempfile::tempdir().unwrap(),
            child: None,
            client: Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap(),
            base: String::new(),
            cookies: vec![],
            ids: vec![],
            devices: vec![],
            spaces: vec![],
            development: false,
            remote_workers: false,
        };
        f.start().await;
        let token = std::fs::read_to_string(f.appdata().join("setup.token")).unwrap();
        f.enroll(&token).await;
        f.stop();
        let auth = Auth::open(&f.appdata(), AuthConfig::new(ORIGIN).unwrap()).unwrap();
        let mut tokens = vec![];
        for name in ["Account A", "Account B", "No membership", "Ordinary target"] {
            tokens.push(auth.create(name).await.unwrap().token);
        }
        drop(auth);
        f.start().await;
        for token in tokens {
            f.enroll(&token).await;
        }
        for i in [1, 2] {
            let s = f
                .op(i, "spaces.create", json!({"name":format!("private {i}")}))
                .await;
            s.ok();
            f.spaces
                .push(s.body["data"]["space"]["id"].as_str().unwrap().into());
        }
        // Preprovision this common fixture so tests start with fixed roles.
        for role in ["reader", "editor", "manager"] {
            let s = f
                .op(1, "spaces.create", json!({"name":format!("role {role}")}))
                .await;
            s.ok();
            f.spaces
                .push(s.body["data"]["space"]["id"].as_str().unwrap().into());
        }
        f.stop();
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        for (s, r) in f.spaces[2..].iter().zip(["reader", "editor", "manager"]) {
            c.execute(
                "INSERT INTO space_memberships VALUES(?1,?2,?3)",
                params![s, f.ids[2], r],
            )
            .unwrap();
        }
        c.execute(
            "INSERT INTO space_memberships VALUES(?1,?2,'reader')",
            params![f.spaces[4], f.ids[4]],
        )
        .unwrap();
        drop(c);
        f.start().await;
        let admin = f
            .req(
                "GET",
                "/api/v1/auth/me",
                Some(&f.cookies[0]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(admin.body["user"]["role"], "admin");
        for i in [0, 3] {
            let s = f.op(i, "spaces.list", json!({})).await;
            s.ok();
            let spaces = s.body["data"]["spaces"].as_array().unwrap();
            assert_eq!(spaces.len(), 1);
            assert_eq!(spaces[0]["personal"], true);
            assert_eq!(spaces[0]["role"], "owner");
            assert_eq!(spaces[0]["owner"], f.ids[i]);
            assert_ne!(spaces[0]["id"], f.ids[i]);
        }
        f
    }
    async fn note(&self, who: usize, space: &str) -> Value {
        let r = self
            .op(
                who,
                "notes.create",
                json!({"space_id":space,"title":"Secret","markdown":"private content\n"}),
            )
            .await;
        r.ok();
        r.body["data"]["note"].clone()
    }
}
// Linux-only /proc discovery avoids reserving a fixed port or touching the preview port.
fn listener_port(pid: u32) -> u16 {
    let inodes: Vec<String> = std::fs::read_dir(format!("/proc/{pid}/fd"))
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|e| std::fs::read_link(e.path()).ok())
        .filter_map(|p| {
            p.to_str()
                .and_then(|s| s.strip_prefix("socket:["))
                .and_then(|s| s.strip_suffix(']'))
                .map(str::to_owned)
        })
        .collect();
    let table = std::fs::read_to_string(format!("/proc/{pid}/net/tcp")).unwrap();
    for line in table.lines().skip(1) {
        let cols: Vec<_> = line.split_whitespace().collect();
        if cols[3] == "0A" && inodes.iter().any(|i| i == cols[9]) {
            return u16::from_str_radix(cols[1].split(':').nth(1).unwrap(), 16).unwrap();
        }
    }
    panic!("no listener");
}
fn evidence(name: &str, value: Value) {
    if let Ok(out) = std::env::var("CONFORMANCE_RESULTS") {
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(
            Path::new(&out).join(format!("{name}.json")),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn all_operations_hide_foreign_ids() {
    let f = Fixture::new().await;
    let n = f.note(1, &f.spaces[0]).await;
    let job = jobs_cases::start_job(
        &f,
        1,
        &f.spaces[0],
        json!({"steps":1,"delay_ms":0,"every":1}),
    )
    .await;
    let missing = uuid::Uuid::now_v7().to_string();
    let mut cases = vec![
        ("jobs.get", json!({"job_id":job}), json!({"job_id":missing})),
        (
            "jobs.cancel",
            json!({"job_id":job}),
            json!({"job_id":missing}),
        ),
        (
            "resolve",
            json!({"kind":"notes","id":n["id"]}),
            json!({"kind":"notes","id":missing}),
        ),
        ("notes.get", json!({"id":n["id"]}), json!({"id":missing})),
        (
            "notes.history",
            json!({"id":n["id"],"cursor":"1"}),
            json!({"id":missing,"cursor":"1"}),
        ),
        (
            "notes.update",
            json!({"id":n["id"],"base_revision":n["revision"],"title":"stolen","markdown":"stolen"}),
            json!({"id":missing,"base_revision":n["revision"],"title":"stolen","markdown":"stolen"}),
        ),
        (
            "notes-insights.summarize",
            json!({"id":n["id"]}),
            json!({"id":missing}),
        ),
        (
            "spaces.members",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "spaces.set_role",
            json!({"space_id":f.spaces[0],"account_id":f.ids[1],"role":"reader"}),
            json!({"space_id":missing,"account_id":f.ids[1],"role":"reader"}),
        ),
        (
            "spaces.remove_member",
            json!({"space_id":f.spaces[0],"account_id":f.ids[1]}),
            json!({"space_id":missing,"account_id":f.ids[1]}),
        ),
        (
            "spaces.leave",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "spaces.transfer",
            json!({"space_id":f.spaces[0],"account_id":f.ids[2]}),
            json!({"space_id":missing,"account_id":f.ids[2]}),
        ),
        (
            "spaces.delete",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "notes.list",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "notes.create",
            json!({"space_id":f.spaces[0],"title":"x","markdown":"x"}),
            json!({"space_id":missing,"title":"x","markdown":"x"}),
        ),
        (
            "plugins.list",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "plugins.enable",
            json!({"space_id":f.spaces[0],"plugin_id":"calternal-notes-insights"}),
            json!({"space_id":missing,"plugin_id":"calternal-notes-insights"}),
        ),
        (
            "plugins.disable",
            json!({"space_id":f.spaces[0],"plugin_id":"calternal-notes-insights"}),
            json!({"space_id":missing,"plugin_id":"calternal-notes-insights"}),
        ),
        (
            "plugins.preview",
            json!({"space_id":f.spaces[0],"plugin_id":"calternal-notes-insights"}),
            json!({"space_id":missing,"plugin_id":"calternal-notes-insights"}),
        ),
        (
            "secrets.request",
            json!({"space_id":f.spaces[0],"name":"probe","label":"L","reason":"R"}),
            json!({"space_id":missing,"name":"probe","label":"L","reason":"R"}),
        ),
        (
            "secrets.list",
            json!({"space_id":f.spaces[0]}),
            json!({"space_id":missing}),
        ),
        (
            "secrets.revoke",
            json!({"space_id":f.spaces[0],"name":"probe"}),
            json!({"space_id":missing,"name":"probe"}),
        ),
    ];
    cases.extend(notes_cases::foreign_cases(&f.spaces[0], &n, &missing));
    let catalog = f
        .req(
            "GET",
            "/api/v1/describe",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    catalog.ok();
    let mut covered: Vec<_> = cases.iter().map(|(op, _, _)| *op).collect();
    covered.extend(["spaces.list", "spaces.create"]);
    covered.extend(agent_cases::OPERATIONS.iter().copied());
    covered.extend(people_cases::ADDITIONAL_OPERATIONS.iter().copied());

    covered.extend(requests_cases::OPERATIONS.iter().copied());
    // Private account input is exercised with real sessions in drafts_cases.rs.
    covered.extend([
        "drafts.save",
        "drafts.list",
        "drafts.get",
        "drafts.delete",
        "pending.queue",
        "pending.edit",
        "pending.list",
        "pending.cancel",
        "pending.confirm",
        "pending.submit",
    ]);
    covered.sort_unstable();
    let mut registered: Vec<_> = catalog.body["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|op| op["id"].as_str().unwrap())
        .collect();
    registered.sort_unstable();
    assert_eq!(
        registered, covered,
        "operation inventory changed; add conformance cases"
    );
    let mut rows = vec![];
    let mut failures = vec![];
    for who in [0, 2, 3] {
        for (op, a, b) in &cases {
            let mut times = vec![];
            for _ in 0..3 {
                let x = f.op_id(who, op, a.clone(), "same-probe-id").await;
                let y = f.op_id(who, op, b.clone(), "same-probe-id").await;
                let equal = x.signature() == y.signature();
                if !equal || x.status != 404 {
                    failures.push(format!(
                        "{who}/{op}: {}/{} equal={equal}",
                        x.status, y.status
                    ));
                }
                times.push(json!({"foreign_us":x.micros,"missing_us":y.micros}));
                rows.push(json!({"account":who,"operation":op,"foreign":x.signature(),"missing":y.signature(),"equal":equal}));
            }
            let mut foreign: Vec<u64> = times
                .iter()
                .map(|t| t["foreign_us"].as_u64().unwrap())
                .collect();
            let mut missing: Vec<u64> = times
                .iter()
                .map(|t| t["missing_us"].as_u64().unwrap())
                .collect();
            foreign.sort_unstable();
            missing.sort_unstable();
            // A gross timing leak smoke check, not a performance benchmark or proof
            // of constant-time behavior on a shared development VM.
            if foreign[1].abs_diff(missing[1]) > 20_000 {
                failures.push(format!("{who}/{op}: median timing delta exceeds 20ms"));
            }
            rows.push(json!({"account":who,"operation":op,"timings_smoke_only":times}));
        }
    }
    for who in [0, 2, 3] {
        let r = f.op(who, "spaces.list", json!({})).await;
        r.ok();
        assert!(!r.body.to_string().contains(&f.spaces[0]));
    }
    evidence("cross-space", json!(rows));
    assert!(
        failures.is_empty(),
        "no-access must be identical not-found: {failures:?}"
    );
}

const SECRET_CANARY: &str = "sk-conformance-canary-1b7e44-never-in-clear";

fn contains_canary(dir: &Path) -> Vec<PathBuf> {
    let mut hits = vec![];
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            hits.extend(contains_canary(&path));
        } else if std::fs::read(&path).is_ok_and(|b| {
            b.windows(SECRET_CANARY.len())
                .any(|w| w == SECRET_CANARY.as_bytes())
        }) {
            hits.push(path);
        }
    }
    hits
}

#[tokio::test]
async fn secrets_are_owner_private_and_provide_needs_a_fresh_session() {
    let mut f = Fixture::new().await;
    let provide = "secrets/provide";
    let missing = uuid::Uuid::now_v7().to_string();
    // Account A's personal request is invisible to everyone else.
    let personal = f
        .op(
            1,
            "secrets.request",
            json!({"name":"github","label":"GitHub","reason":"push"}),
        )
        .await;
    personal.ok();
    let personal = personal.body["data"]["request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    for who in [0, 2, 3] {
        let list = f.op(who, "secrets.list", json!({})).await;
        list.ok();
        assert_eq!(
            list.body["data"],
            json!({"secrets":[],"requests":[]}),
            "account {who}"
        );
        let foreign = f
            .op_id(
                who,
                "secrets.revoke",
                json!({"request_id":personal}),
                "probe",
            )
            .await;
        let absent = f
            .op_id(
                who,
                "secrets.revoke",
                json!({"request_id":missing}),
                "probe",
            )
            .await;
        assert_eq!(
            (foreign.status, foreign.signature()),
            (404, absent.signature())
        );
        let foreign = f
            .auth(
                provide,
                Some(&f.cookies[who]),
                json!({"request_id":personal,"value":SECRET_CANARY}),
            )
            .await;
        let absent = f
            .auth(
                provide,
                Some(&f.cookies[who]),
                json!({"request_id":missing,"value":SECRET_CANARY}),
            )
            .await;
        assert_eq!(
            (foreign.status, foreign.signature()),
            (404, absent.signature())
        );
    }
    // A space request cannot be answered by a non-member, an account without
    // memberships or an admin without membership: same answer as no space.
    let space = f
        .op(
            1,
            "secrets.request",
            json!({"space_id":f.spaces[0],"name":"smtp","label":"SMTP","reason":"mail"}),
        )
        .await;
    space.ok();
    let space = space.body["data"]["request_id"]
        .as_str()
        .unwrap()
        .to_owned();
    for who in [0, 2, 3] {
        let foreign = f
            .auth(
                provide,
                Some(&f.cookies[who]),
                json!({"space_id":f.spaces[0],"request_id":space,"value":SECRET_CANARY}),
            )
            .await;
        let absent = f
            .auth(
                provide,
                Some(&f.cookies[who]),
                json!({"space_id":missing,"request_id":space,"value":SECRET_CANARY}),
            )
            .await;
        assert_eq!(
            (foreign.status, foreign.signature()),
            (404, absent.signature()),
            "account {who}"
        );
    }
    // A stale session (assertion older than the freshness window) is refused
    // before the body is read, and nothing is stored.
    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    c.execute(
        "UPDATE installations SET assertion_at = assertion_at - 86400000 WHERE user_id=?1",
        params![f.ids[1]],
    )
    .unwrap();
    drop(c);
    f.start().await;
    let stale = f
        .auth(
            provide,
            Some(&f.cookies[1]),
            json!({"request_id":personal,"value":SECRET_CANARY}),
        )
        .await;
    assert_eq!(stale.status, 403, "{}", stale.body);
    let list = f.op(1, "secrets.list", json!({})).await;
    list.ok();
    assert_eq!(list.body["data"]["secrets"], json!([]));
    assert_eq!(list.body["data"]["requests"].as_array().unwrap().len(), 1);
    // After signing in again the same request is answered; no value reaches disk.
    let login = f.login(1).await;
    login.ok();
    f.cookies[1] = login.cookie();
    let fresh = f
        .auth(
            provide,
            Some(&f.cookies[1]),
            json!({"request_id":personal,"value":SECRET_CANARY}),
        )
        .await;
    fresh.ok();
    assert!(!fresh.body.to_string().contains(SECRET_CANARY));
    assert_eq!(contains_canary(f.dir.path()), Vec::<PathBuf>::new());
    evidence(
        "secrets",
        json!({"stale":stale.signature(),"fresh":fresh.signature()}),
    );
}

#[tokio::test]
async fn role_boundaries_and_demoted_receipts() {
    let f = Fixture::new().await;
    let mut rows = vec![];
    for (index, role) in [(2, "reader"), (3, "editor"), (4, "manager")] {
        let s = &f.spaces[index];
        let n = f.note(1, s).await;
        for (op, input, allowed) in [
            ("notes.get", json!({"id":n["id"]}), true),
            (
                "notes.create",
                json!({"space_id":s,"title":"role write","markdown":"x"}),
                role != "reader",
            ),
            (
                "plugins.enable",
                json!({"space_id":s,"plugin_id":"calternal-notes-insights"}),
                role == "manager",
            ),
            (
                "spaces.set_role",
                json!({"space_id":s,"account_id":f.ids[2],"role":"manager"}),
                false,
            ),
            (
                "spaces.remove_member",
                json!({"space_id":s,"account_id":f.ids[1]}),
                false,
            ),
            (
                "spaces.remove_member",
                json!({"space_id":s,"account_id":f.ids[2]}),
                false,
            ),
            (
                "spaces.transfer",
                json!({"space_id":s,"account_id":f.ids[2]}),
                false,
            ),
            ("spaces.delete", json!({"space_id":s}), false),
        ] {
            // Non-manager ordinary members may remove themselves only with leave.
            let r = f.op(2, op, input).await;
            rows.push(json!({"role":role,"op":op,"status":r.status,"body":r.body}));
            assert_eq!(r.status == 200, allowed, "{role}/{op}: {}", r.body);
        }
        let leave = f.op(1, "spaces.leave", json!({"space_id":s})).await;
        assert_eq!(leave.status, 403);
    }
    let s = &f.spaces[4];
    f.op(
        2,
        "spaces.set_role",
        json!({"space_id":s,"account_id":f.ids[4],"role":"editor"}),
    )
    .await
    .ok();
    f.op(
        2,
        "spaces.remove_member",
        json!({"space_id":s,"account_id":f.ids[4]}),
    )
    .await
    .ok();
    let s = &f.spaces[3];
    let input = json!({"space_id":s,"title":"receipt","markdown":"secret"});
    f.op_id(2, "notes.create", input.clone(), "before-demotion")
        .await
        .ok();
    f.op(
        1,
        "spaces.set_role",
        json!({"space_id":s,"account_id":f.ids[2],"role":"reader"}),
    )
    .await
    .ok();
    let replay = f.op_id(2, "notes.create", input, "before-demotion").await;
    assert_eq!(replay.status, 403);
    evidence("roles", json!(rows));
}

#[tokio::test]
async fn account_routes_reject_foreign_resources_and_bad_sessions() {
    let mut f = Fixture::new().await;
    let me = f
        .req(
            "GET",
            "/api/v1/auth/me",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    let foreign = me.body["installation"].clone();
    let missing = json!(uuid::Uuid::now_v7().to_string());
    let mut rows = vec![];
    for (route, field, a, b) in [
        (
            "installations/revoke",
            "installation",
            foreign.clone(),
            missing.clone(),
        ),
        (
            "passkeys/remove",
            "id",
            f.devices[1].cred.clone(),
            json!("bm90LWEtY3JlZA"),
        ),
        (
            "passkeys/rename",
            "id",
            f.devices[1].cred.clone(),
            json!("bm90LWEtY3JlZA"),
        ),
    ] {
        let mut x = json!({"name":"conformance"});
        if route != "passkeys/rename" {
            x = json!({});
        }
        x[field] = a;
        let mut y = x.clone();
        y[field] = b;
        let a = f.auth(route, Some(&f.cookies[2]), x).await;
        let b = f.auth(route, Some(&f.cookies[2]), y).await;
        assert_eq!(a.signature(), b.signature(), "{route}");
        assert!(a.status >= 400);
        rows.push(json!({"route":route,"status":a.status}));
    }
    let second = f.login(1).await;
    second.ok();
    let second = second.cookie();
    let old = f.cookies[1].clone();
    f.auth("logout", Some(&old), json!({})).await;
    f.req(
        "GET",
        "/api/v1/auth/me",
        Some(&second),
        json!({}),
        Some(ORIGIN),
    )
    .await
    .ok();
    // Revoke a still-live third device through HTTP, then probe its session.
    let third = f.login(1).await;
    third.ok();
    let third_cookie = third.cookie();
    let third_me = f
        .req(
            "GET",
            "/api/v1/auth/me",
            Some(&third_cookie),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    third_me.ok();
    let r = f
        .auth(
            "installations/revoke",
            Some(&second),
            json!({"installation":third_me.body["installation"]}),
        )
        .await;
    r.ok();
    assert_eq!(
        f.req(
            "GET",
            "/api/v1/auth/me",
            Some(&third_cookie),
            json!({}),
            Some(ORIGIN)
        )
        .await
        .status,
        401
    );
    for (method, name) in AUTH_ROUTES {
        let r = f
            .req(
                method,
                &format!("/api/v1/auth/{name}"),
                Some(&old),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        if ![
            "status",
            "setup/start",
            "setup/finish",
            "login/start",
            "login/finish",
            "logout",
        ]
        .contains(name)
        {
            assert_eq!(r.status, 401, "{name}: {}", r.body);
        }
        rows.push(json!({"revoked_route":name,"status":r.status}));
    }
    for cookie in [
        &old,
        "__Host-calternal_session=forged",
        &format!("{second}; {second}"),
    ] {
        let r = f
            .req(
                "GET",
                "/api/v1/describe",
                Some(cookie),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(r.status, 401);
        let r = f
            .req(
                "POST",
                "/api/v1/operations",
                Some(cookie),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(r.status, 401);
    }
    for origin in [None, Some("https://evil.invalid"), Some("null")] {
        for (method, name) in AUTH_ROUTES {
            let chosen = if *name == "logout" { &old } else { &second };
            let r = f
                .req(
                    method,
                    &format!("/api/v1/auth/{name}"),
                    Some(chosen),
                    json!({}),
                    origin,
                )
                .await;
            rows.push(json!({"origin":origin,"route":name,"status":r.status}));
            if *method == "POST" && origin.is_some() {
                assert!(
                    [401, 403].contains(&r.status),
                    "{name} origin={origin:?}: {}",
                    r.body
                );
            }
        }
        let r = f
            .req(
                "POST",
                "/api/v1/operations",
                Some(&second),
                json!({"request_id":"origin-probe","operation":"spaces.list","input":{}}),
                origin,
            )
            .await;
        if origin.is_some() {
            assert!([401, 403].contains(&r.status));
        } else {
            r.ok();
        }
    }
    evidence("sessions-routes", json!(rows));
}

#[tokio::test]
async fn ceremonies_cookie_fixation_and_parallel_limits() {
    let mut f = Fixture::new().await;
    let start = f.auth("login/start", None, json!({})).await;
    start.ok();
    let cred = f.devices[1].sign(&start);
    let body = json!({"ceremony":start.body["ceremony"],"credential":cred});
    let a = f
        .auth("login/finish", Some(&f.cookies[2]), body.clone())
        .await;
    a.ok();
    assert_ne!(a.cookie(), f.cookies[2], "login must rotate token");
    let cookie = &a.headers["set-cookie"];
    for attr in [
        "__Host-calternal_session=",
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Path=/",
    ] {
        assert!(cookie.contains(attr), "missing {attr}");
    }
    assert!(!cookie.to_lowercase().contains("domain="));
    let me = f
        .req(
            "GET",
            "/api/v1/auth/me",
            Some(&a.cookie()),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(me.body["user"]["id"], f.ids[1]);
    assert!(f.auth("login/finish", None, body).await.status >= 400);
    let used = std::fs::read_to_string(f.appdata().join("setup.token")).unwrap();
    assert!(
        f.auth("setup/start", None, json!({"token":used}))
            .await
            .status
            >= 400
    );
    let start = f
        .auth("reverify/start", Some(&f.cookies[1]), json!({}))
        .await;
    start.ok();
    let cred = f.devices[1].sign(&start);
    let body = json!({"ceremony":start.body["ceremony"],"credential":cred});
    assert!(
        f.auth("reverify/finish", Some(&f.cookies[2]), body.clone())
            .await
            .status
            >= 400
    );
    f.auth("reverify/finish", Some(&f.cookies[1]), body)
        .await
        .ok();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..140 {
        let client = f.client.clone();
        let base = f.base.clone();
        tasks.spawn(async move {
            client
                .post(format!("{base}/api/v1/auth/login/start"))
                .header("host", "localhost:18080")
                .header("origin", ORIGIN)
                .header("content-type", "application/json")
                .header("x-forwarded-for", uuid::Uuid::now_v7().to_string())
                .body("{}")
                .send()
                .await
                .unwrap()
                .status()
                .as_u16()
        });
    }
    let mut statuses = BTreeMap::<u16, usize>::new();
    while let Some(x) = tasks.join_next().await {
        *statuses.entry(x.unwrap()).or_default() += 1;
    }
    assert!(
        statuses.get(&429).copied().unwrap_or(0) >= 20,
        "parallel source limiter bypass: {statuses:?}"
    );
    assert!(!statuses.keys().any(|s| *s >= 500));
    evidence("ceremonies-rate", json!(statuses));
}

#[tokio::test]
async fn input_validation_and_request_id_scope() {
    let f = Fixture::new().await;
    let n = f.note(1, &f.spaces[0]).await;
    let mut rows = vec![];
    for raw in [
        "{".to_string(),
        "null".into(),
        "[]".into(),
        "x".repeat(2 * 1024 * 1024),
    ] {
        let r = f
            .raw(
                "POST",
                "/api/v1/operations",
                Some(&f.cookies[1]),
                raw,
                Some(ORIGIN),
            )
            .await;
        assert!((400..500).contains(&r.status));
        rows.push(json!({"operation_body_status":r.status}));
    }
    for raw in ["{".to_string(), "x".repeat(65537)] {
        let r = f
            .raw("POST", "/api/v1/auth/login/start", None, raw, Some(ORIGIN))
            .await;
        assert!((400..500).contains(&r.status));
    }
    for kind in ["tasks", "files", "collections", "unknown-kind"] {
        let r = f.op(1, "resolve", json!({"kind":kind,"id":n["id"]})).await;
        assert_eq!(r.status, 404);
    }
    for id in [
        "../../escape",
        "/tmp/escape",
        "00000000-0000-0000-0000-000000000000",
        "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF",
        "{00000000-0000-0000-0000-000000000000}",
        "%2e%2e%2fescape",
        "x\0x",
    ] {
        for (op, input) in [
            ("notes.get", json!({"id":id})),
            (
                "notes.create",
                json!({"space_id":id,"title":"x","markdown":"x"}),
            ),
            (
                "plugins.enable",
                json!({"space_id":f.spaces[0],"plugin_id":id}),
            ),
        ] {
            let r = f.op(1, op, input).await;
            assert!((400..500).contains(&r.status), "{op}/{id}: {}", r.body);
            rows.push(json!({"op":op,"id":id,"status":r.status}));
        }
    }
    let input = json!({"space_id":f.spaces[0],"title":"receipt","markdown":"a"});
    let first = f.op_id(1, "notes.create", input.clone(), "duplicate").await;
    first.ok();
    let same = f.op_id(1, "notes.create", input, "duplicate").await;
    same.ok();
    assert_eq!(first.body["data"], same.body["data"]);
    let other = f
        .op_id(
            2,
            "notes.create",
            json!({"space_id":f.spaces[1],"title":"other","markdown":"b"}),
            "duplicate",
        )
        .await;
    other.ok();
    assert_ne!(
        first.body["data"]["note"]["id"],
        other.body["data"]["note"]["id"]
    );
    let reused = f
        .op_id(
            1,
            "notes.create",
            json!({"space_id":f.spaces[0],"title":"changed","markdown":"c"}),
            "duplicate",
        )
        .await;
    assert_eq!(reused.status, 409, "same account, space and request ID");
    evidence("input", json!(rows));
}

/// Request IDs are unique per (account, space). The same ID used by one account in
/// two spaces, or by two accounts in one space, yields independent results; each
/// retry replays only its own, never the other's.
#[tokio::test]
async fn request_ids_are_scoped_to_account_and_space() {
    let f = Fixture::new().await;
    // Account 1 owns spaces 0 and 3; account 2 is an editor in space 3.
    let create = |space: &str, title: &str| json!({"space_id":space,"title":title,"markdown":format!("{title}\n")});
    let mut results = vec![];
    for (who, space, title) in [
        (1, &f.spaces[0], "one in A"),
        (1, &f.spaces[3], "one in B"),
        (2, &f.spaces[3], "two in B"),
    ] {
        let first = f
            .op_id(who, "notes.create", create(space, title), "shared-id")
            .await;
        first.ok();
        let note = first.body["data"]["note"].clone();
        assert_eq!(note["title"], title);
        assert_eq!(note["space_id"], json!(space));
        let again = f
            .op_id(who, "notes.create", create(space, title), "shared-id")
            .await;
        again.ok();
        assert_eq!(
            again.body["data"]["note"], note,
            "a retry replays its own result"
        );
        results.push(note["id"].as_str().unwrap().to_owned());
    }
    let distinct: std::collections::BTreeSet<_> = results.iter().collect();
    assert_eq!(distinct.len(), 3, "three independent results");
    evidence("request_id_scope", json!(results));
}

fn walk(root: &Path, path: &Path, out: &mut Vec<Value>) {
    use std::os::unix::fs::PermissionsExt;
    for e in std::fs::read_dir(path).unwrap() {
        let e = e.unwrap();
        let m = e.metadata().unwrap();
        out.push(json!({"path":e.path().strip_prefix(root).unwrap(),"directory":m.is_dir(),"mode":format!("{:04o}",m.permissions().mode()&0o777)}));
        if m.is_dir() {
            walk(root, &e.path(), out);
        }
    }
}
#[tokio::test]
async fn filesystem_modes_and_complete_sql_inventory() {
    let f = Fixture::new().await;
    f.note(1, &f.spaces[0]).await;
    f.note(2, &f.spaces[1]).await;
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":f.spaces[0],"plugin_id":"calternal-notes-insights"}),
    )
    .await
    .ok();
    let mut files = vec![];
    // The fixture directory only holds the three roots; every entry below it is
    // the server's (roots included).
    walk(f.dir.path(), f.dir.path(), &mut files);
    let mut tables = vec![];
    // Plugin enablement lives in the space's own content.db; no installation-wide
    // state database exists.
    assert!(!f.appdata().join("state.sqlite3").exists());
    let space_db = format!("spaces/{}/content.db", f.spaces[0]);
    for db in [
        space_db.as_str(),
        "system/content-index.db",
        "system/auth.db",
    ] {
        let c = Connection::open_with_flags(
            f.appdata().join(db),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let mut st = c
            .prepare("SELECT name,sql FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        for r in st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
        {
            let (name, sql) = r.unwrap();
            let count: i64 = c
                .query_row(
                    &format!("SELECT COUNT(*) FROM \"{}\"", name.replace('"', "\"\"")),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            tables.push(json!({"db":db,"table":name,"sql":sql,"rows":count}));
        }
    }
    evidence("storage", json!({"files":files,"tables":tables}));
    let exposed: Vec<_> = files
        .iter()
        .filter(|v| u32::from_str_radix(v["mode"].as_str().unwrap(), 8).unwrap() & 0o077 != 0)
        .collect();
    assert!(
        exposed.is_empty(),
        "server-created entries accessible by group/other: {exposed:?}"
    );
}

#[tokio::test]
async fn expired_sessions_rejected_on_all_protected_routes() {
    let mut f = Fixture::new().await;
    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    c.execute(
        "UPDATE installations SET expires_at=1 WHERE user_id=?1",
        [&f.ids[1]],
    )
    .unwrap();
    drop(c);
    f.start().await;
    for (method, name) in AUTH_ROUTES {
        let r = f
            .req(
                method,
                &format!("/api/v1/auth/{name}"),
                Some(&f.cookies[1]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        if ![
            "status",
            "setup/start",
            "setup/finish",
            "login/start",
            "login/finish",
            "logout",
        ]
        .contains(name)
        {
            assert_eq!(r.status, 401, "{name}");
        }
    }
    for (m, p) in [("GET", "/api/v1/describe"), ("POST", "/api/v1/operations")] {
        assert_eq!(
            f.req(m, p, Some(&f.cookies[1]), json!({}), Some(ORIGIN))
                .await
                .status,
            401
        );
    }
}

#[tokio::test]
async fn concurrent_transfer_remove_and_kill_restart() {
    let mut f = Fixture::new().await;
    let s = f.spaces[4].clone();
    let mut rows = vec![];
    for round in 0..12 {
        let transfer = f.op(
            1,
            "spaces.transfer",
            json!({"space_id":s,"account_id":f.ids[2]}),
        );
        let remove = f.op(
            1,
            "spaces.remove_member",
            json!({"space_id":s,"account_id":f.ids[2]}),
        );
        let (a, b) = tokio::join!(transfer, remove);
        assert!(
            !(a.status == 200 && b.status == 200),
            "transfer and removal both succeeded"
        );
        f.stop();
        f.start().await;
        let c = Connection::open_with_flags(
            f.appdata().join("system/auth.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .unwrap();
        let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM spaces s JOIN space_memberships m ON m.space_id=s.id AND m.account_id=s.owner_account WHERE s.id=?1)",[&s],|r|r.get(0)).unwrap();
        assert!(valid);
        drop(c);
        rows.push(json!({"round":round,"transfer":a.status,"remove":b.status}));
        if a.status == 200 {
            f.op(
                2,
                "spaces.transfer",
                json!({"space_id":s,"account_id":f.ids[1]}),
            )
            .await
            .ok();
        } else {
            break;
        } // Removed members cannot rejoin: no admission endpoint exists.
    }
    // SIGKILL while HTTP authority mutation is submitted. Outcomes may be old or
    // new after an unacknowledged request; owner membership must always be valid.
    for operation in ["spaces.set_role", "spaces.transfer"] {
        let client = f.client.clone();
        let base = f.base.clone();
        let cookie = f.cookies[1].clone();
        let mut input = json!({"space_id":f.spaces[3],"account_id":f.ids[2]});
        if operation == "spaces.set_role" {
            input["role"] = json!("manager");
        }
        let body = json!({"request_id":uuid::Uuid::now_v7().to_string(),"operation":operation,"input":input});
        let task = tokio::spawn(async move {
            client
                .post(format!("{base}/api/v1/operations"))
                .header("host", "localhost:18080")
                .header("origin", ORIGIN)
                .header("cookie", cookie)
                .json(&body)
                .send()
                .await
        });
        tokio::time::sleep(Duration::from_micros(500)).await;
        f.stop();
        let _ = task.await;
        f.start().await;
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        let bad:i64=c.query_row("SELECT COUNT(*) FROM spaces s LEFT JOIN space_memberships m ON s.id=m.space_id AND s.owner_account=m.account_id WHERE m.account_id IS NULL",[],|r|r.get(0)).unwrap();
        assert_eq!(bad, 0);
    }
    let start = f.auth("login/start", None, json!({})).await;
    start.ok();
    let cred = f.devices[1].sign(&start);
    let body = json!({"ceremony":start.body["ceremony"],"credential":cred});
    let client = f.client.clone();
    let base = f.base.clone();
    let task = tokio::spawn(async move {
        client
            .post(format!("{base}/api/v1/auth/login/finish"))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .json(&body)
            .send()
            .await
    });
    tokio::time::sleep(Duration::from_micros(500)).await;
    f.stop();
    let _ = task.await;
    f.start().await;
    f.login(1).await.ok();
    evidence("crash-race", json!(rows));
}

// A trusted test runtime wrapper pauses after reading the first worker frame (the
// real invocation). Workers are long-lived, so it then starts the pinned shipped
// worker unchanged with its exact arguments, replays that frame and relays the rest.
fn gate_runtime(f: &Fixture) {
    use std::os::unix::fs::PermissionsExt;
    let find = |name: &str| {
        std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|p| p.join(name))
            .find(|p| p.is_file())
            .unwrap()
    };
    let python = find("python3");
    let node = find("node");
    let root = f.dir.path().display();
    let script = format!(
        "#!{}\nimport os,sys,subprocess,pathlib,time\nnode={:?}\nif sys.argv[1:]==['--version']:\n sys.exit(subprocess.call([node,'--version']))\nfirst=b''\nwhile not first.endswith(b'\\n'):\n b=os.read(0,1)\n if not b: sys.exit(1)\n first+=b\npathlib.Path({:?}).touch()\nend=time.monotonic()+2\nwhile not pathlib.Path({:?}).exists() and time.monotonic()<end: time.sleep(.005)\np=subprocess.Popen([node]+sys.argv[1:],stdin=subprocess.PIPE)\nos.write(p.stdin.fileno(),first)\nwhile True:\n b=os.read(0,65536)\n if not b: break\n os.write(p.stdin.fileno(),b)\np.stdin.close()\nsys.exit(p.wait())\n",
        python.display(),
        node.to_str().unwrap(),
        format!("{root}/worker-entered"),
        format!("{root}/worker-release")
    );
    let p = f.dir.path().join("node-gate");
    std::fs::write(&p, script).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o700)).unwrap();
}
// Each case pauses the same shipped worker at the delivery boundary.
#[derive(Clone, Copy)]
enum WorkerRevocation {
    Membership,
    Logout,
    OtherDevice,
    Expiry,
}

async fn running_worker_revoke(mode: WorkerRevocation) {
    let session = !matches!(mode, WorkerRevocation::Membership);
    let other_device = matches!(mode, WorkerRevocation::OtherDevice);
    let expiry = matches!(mode, WorkerRevocation::Expiry);
    let mut f = Fixture::new().await;
    let s = f.spaces[2].clone();
    let n = f.note(1, &s).await;
    f.op(
        1,
        "plugins.enable",
        json!({"space_id":s,"plugin_id":"calternal-notes-insights"}),
    )
    .await
    .ok();
    let other_cookie = if other_device {
        let login = f.login(2).await;
        login.ok();
        Some(login.cookie())
    } else {
        None
    };
    // Prove the real worker succeeds before imposing a revocation barrier.
    f.op(2, "notes-insights.summarize", json!({"id":n["id"]}))
        .await
        .ok();
    f.stop();
    gate_runtime(&f);
    let expires_at = if expiry {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let expires_at = now + 1_000;
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        c.execute(
            "UPDATE installations SET expires_at=?1 WHERE user_id=?2",
            params![expires_at, f.ids[2]],
        )
        .unwrap();
        Some(expires_at)
    } else {
        None
    };
    f.start().await;
    let client = f.client.clone();
    let base = f.base.clone();
    let cookie = f.cookies[2].clone();
    let body = json!({"request_id":"inflight-worker","operation":"notes-insights.summarize","input":{"id":n["id"]}});
    let task = tokio::spawn(async move {
        client
            .post(format!("{base}/api/v1/operations"))
            .header("host", "localhost:18080")
            .header("origin", ORIGIN)
            .header("cookie", cookie)
            .json(&body)
            .send()
            .await
            .unwrap()
    });
    let marker = f.dir.path().join("worker-entered");
    for _ in 0..150 {
        if marker.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert!(marker.exists(), "worker did not reach barrier");
    if let Some(expires_at) = expires_at {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        tokio::time::sleep(Duration::from_millis(
            expires_at.saturating_sub(now).max(0) as u64 + 10,
        ))
        .await;
    } else if session {
        if let Some(cookie) = other_cookie {
            let me = f
                .req(
                    "GET",
                    "/api/v1/auth/me",
                    Some(&f.cookies[2]),
                    json!({}),
                    Some(ORIGIN),
                )
                .await;
            me.ok();
            f.auth(
                "installations/revoke",
                Some(&cookie),
                json!({"installation":me.body["installation"]}),
            )
            .await
            .ok();
        } else {
            let r = f.auth("logout", Some(&f.cookies[2]), json!({})).await;
            assert_eq!(r.status, 204);
        }
    } else {
        f.op(
            1,
            "spaces.remove_member",
            json!({"space_id":s,"account_id":f.ids[2]}),
        )
        .await
        .ok();
    }
    std::fs::write(f.dir.path().join("worker-release"), "").unwrap();
    let r = task.await.unwrap();
    let status = r.status().as_u16();
    let body: Value = r.json().await.unwrap();
    evidence(
        if expiry {
            "worker-session-expiry"
        } else if other_device {
            "worker-other-device-revocation"
        } else if session {
            "worker-session-revocation"
        } else {
            "worker-membership-revocation"
        },
        json!({"status":status,"body":body}),
    );
    assert!(
        if session {
            status == 401
        } else {
            status == 404
        },
        "revoked caller received worker result after completed revocation: status={status} {body}"
    );
}
#[tokio::test]
async fn running_worker_membership_revocation_blocks_delivery() {
    running_worker_revoke(WorkerRevocation::Membership).await;
}
#[tokio::test]
async fn running_worker_session_revocation_blocks_delivery() {
    running_worker_revoke(WorkerRevocation::Logout).await;
}

#[tokio::test]
async fn running_worker_other_device_revocation_blocks_delivery() {
    running_worker_revoke(WorkerRevocation::OtherDevice).await;
}

#[tokio::test]
async fn development_tokens_require_explicit_flag_even_after_restart() {
    let mut f = Fixture::new().await;
    f.stop();
    f.development = true;
    f.start().await;
    let startup: Value =
        serde_json::from_slice(&std::fs::read(f.appdata().join("startup.json")).unwrap()).unwrap();
    let token = std::fs::read_to_string(startup["token_files"]["owner"].as_str().unwrap()).unwrap();
    let probe = |client: Client, base: String, token: String| async move {
        client
            .get(format!("{base}/api/v1/describe"))
            .header("host", "localhost:18080")
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    };
    assert_eq!(
        probe(f.client.clone(), f.base.clone(), token.clone()).await,
        200
    );
    f.stop();
    f.development = false;
    f.start().await;
    assert_eq!(probe(f.client.clone(), f.base.clone(), token).await, 401);
}

#[tokio::test]
async fn cursors_and_space_hints_do_not_cross_item_boundaries() {
    let f = Fixture::new().await;
    let n = f.note(1, &f.spaces[0]).await;
    f.op(
        1,
        "notes.update",
        json!({"id":n["id"],"base_revision":n["revision"],"title":"v2","markdown":"still secret"}),
    )
    .await
    .ok();
    let h = f
        .op(1, "notes.history", json!({"id":n["id"],"limit":1}))
        .await;
    h.ok();
    let cursor = h.body["data"]["next_cursor"].clone();
    assert!(cursor.is_string());
    let b = f.note(2, &f.spaces[1]).await;
    let result = f
        .op(
            2,
            "notes.history",
            json!({"id":b["id"],"cursor":cursor,"limit":1}),
        )
        .await;
    assert_eq!(result.status, 400, "a foreign continuation must be refused");
    assert!(!result.body.to_string().contains("still secret"));
    for (id, space) in [
        (n["id"].clone(), f.spaces[1].clone()),
        (b["id"].clone(), f.spaces[0].clone()),
    ] {
        let r = f
            .op(2, "notes.get", json!({"id":id,"space_id":space}))
            .await;
        assert_eq!(r.status, 404);
    }
    let r = f
        .op(
            2,
            "notes.history",
            json!({"id":n["id"],"cursor":cursor,"limit":1}),
        )
        .await;
    assert_eq!(r.status, 404);
}

// Deliberately stricter than the retained data-nondisclosure test above: an
// unrelated item's continuation token must be refused, even if it leaks no text.
#[tokio::test]
async fn foreign_history_cursor_is_refused() {
    let f = Fixture::new().await;
    let n = f.note(1, &f.spaces[0]).await;
    f.op(
        1,
        "notes.update",
        json!({"id":n["id"],"base_revision":n["revision"],"markdown":"second"}),
    )
    .await
    .ok();
    let h = f
        .op(1, "notes.history", json!({"id":n["id"],"limit":1}))
        .await;
    h.ok();
    let cursor = h.body["data"]["next_cursor"].clone();
    assert!(cursor.is_string());
    let b = f.note(2, &f.spaces[1]).await;
    let result = f
        .op(
            2,
            "notes.history",
            json!({"id":b["id"],"cursor":cursor,"limit":1}),
        )
        .await;
    evidence(
        "foreign-cursor",
        json!({"status":result.status,"body":result.body}),
    );
    assert!(
        (400..500).contains(&result.status),
        "foreign cursor must be refused: {} {}",
        result.status,
        result.body
    );
}

#[tokio::test]
async fn simultaneous_transfers_preserve_exactly_one_owner() {
    let f = Fixture::new().await;
    let space = &f.spaces[4];
    let mut rows = vec![];
    for _ in 0..8 {
        let (a, b) = tokio::join!(
            f.op(
                1,
                "spaces.transfer",
                json!({"space_id":space,"account_id":f.ids[2]})
            ),
            f.op(
                1,
                "spaces.transfer",
                json!({"space_id":space,"account_id":f.ids[4]})
            )
        );
        assert_eq!(
            [a.status, b.status].iter().filter(|s| **s == 200).count(),
            1
        );
        assert!([a.status, b.status].iter().all(|s| [200, 403].contains(s)));
        let winner = if a.status == 200 { 2 } else { 4 };
        let members = f
            .op(winner, "spaces.members", json!({"space_id":space}))
            .await;
        members.ok();
        let owners: Vec<_> = members.body["data"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "owner")
            .collect();
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0]["account_id"], f.ids[winner]);
        rows.push(json!({"first":a.status,"second":b.status,"owner_count":owners.len()}));
        f.op(
            winner,
            "spaces.transfer",
            json!({"space_id":space,"account_id":f.ids[1]}),
        )
        .await
        .ok();
    }
    evidence("two-transfers", json!(rows));
}

// Remove the precise policy capability from an otherwise complete grant set.
// A successful baseline after restoring it prevents schema/fixture errors from
// being mistaken for authorization enforcement.
#[tokio::test]
async fn every_bound_space_operation_requires_its_policy_capability() {
    let cases = [
        ("resolve", "space.read"),
        ("agent.readiness", "plugins.use"),
        ("spaces.members", "space.read"),
        ("spaces.set_role", "members.manage"),
        ("spaces.remove_member", "members.manage"),
        ("spaces.leave", "space.read"),
        ("spaces.transfer", "space.transfer"),
        ("spaces.delete", "space.delete"),
        ("notes.list", "space.read"),
        ("notes.get", "space.read"),
        ("notes.history", "space.read"),
        ("notes.create", "content.write"),
        ("notes.update", "content.write"),
        ("plugins.list", "space.read"),
        ("plugins.enable", "plugins.manage"),
        ("plugins.disable", "plugins.manage"),
        ("plugins.preview", "space.read"),
        ("notes-insights.summarize", "space.read"),
    ];
    let mut rows = vec![];
    for (op, cap) in cases {
        let mut f = Fixture::new().await;
        let space = f.spaces[3].clone();
        let note = f.note(1, &space).await;
        f.op(
            1,
            "plugins.enable",
            json!({"space_id":space,"plugin_id":"calternal-notes-insights"}),
        )
        .await
        .ok();
        let input = match op {
            "resolve" => json!({"kind":"notes","id":note["id"]}),
            "spaces.set_role" => json!({"space_id":space,"account_id":f.ids[2],"role":"reader"}),
            "spaces.remove_member" | "spaces.transfer" => {
                json!({"space_id":space,"account_id":f.ids[2]})
            }
            "notes.get" | "notes.history" | "notes-insights.summarize" => json!({"id":note["id"]}),
            "notes.create" => json!({"space_id":space,"title":"policy","markdown":"policy"}),
            "notes.update" => {
                json!({"id":note["id"],"base_revision":note["revision"],"markdown":"policy"})
            }
            "plugins.enable" | "plugins.disable" | "plugins.preview" => {
                json!({"space_id":space,"plugin_id":"calternal-notes-insights"})
            }
            _ => json!({"space_id":space}),
        };
        let (who, role) = if op == "spaces.leave" {
            (2, "editor")
        } else {
            (1, "owner")
        };
        f.stop();
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        assert_eq!(
            c.execute(
                "DELETE FROM space_role_capabilities WHERE role=?1 AND capability=?2",
                params![role, cap]
            )
            .unwrap(),
            1
        );
        drop(c);
        f.start().await;
        let denied = f.op(who, op, input.clone()).await;
        assert!(
            [403, 404].contains(&denied.status),
            "missing {cap} allowed {op}: {} {}",
            denied.status,
            denied.body
        );
        f.stop();
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        c.execute(
            "INSERT INTO space_role_capabilities VALUES(?1,?2)",
            params![role, cap],
        )
        .unwrap();
        drop(c);
        f.start().await;
        let allowed = f.op(who, op, input).await;
        allowed.ok();
        rows.push(
            json!({"operation":op,"required":cap,"without":denied.status,"with":allowed.status}),
        );
    }
    evidence("required-capabilities", json!(rows));
}

#[tokio::test]
async fn parallel_passkey_removal_preserves_one_credential() {
    let mut f = Fixture::new().await;
    let s = f
        .auth("passkeys/add/start", Some(&f.cookies[1]), json!({}))
        .await;
    s.ok();
    let mut second = Device::new();
    let cred = second.register(&s);
    let body = json!({"ceremony":s.body["ceremony"],"credential":cred,"name":"second"});
    // Another account cannot consume the rightful account's Add ceremony.
    assert!(
        f.auth("passkeys/add/finish", Some(&f.cookies[2]), body.clone())
            .await
            .status
            >= 400
    );
    f.auth("passkeys/add/finish", Some(&f.cookies[1]), body.clone())
        .await
        .ok();
    assert!(
        f.auth("passkeys/add/finish", Some(&f.cookies[1]), body)
            .await
            .status
            >= 400
    );
    let (a, b) = tokio::join!(
        f.auth(
            "passkeys/remove",
            Some(&f.cookies[1]),
            json!({"id":f.devices[1].cred})
        ),
        f.auth(
            "passkeys/remove",
            Some(&f.cookies[1]),
            json!({"id":second.cred})
        )
    );
    assert_eq!(
        [a.status, b.status].iter().filter(|s| **s == 200).count(),
        1
    );
    assert_eq!(
        [a.status, b.status].iter().filter(|s| **s == 409).count(),
        1
    );
    if a.status == 200 {
        f.devices[1] = second;
    }
    f.login(1).await.ok();
    evidence(
        "passkey-removal",
        json!({"first":a.status,"second":b.status}),
    );
}

#[tokio::test]
async fn each_space_capability_is_independent_over_http() {
    let mut f = Fixture::new().await;
    let s = f.spaces[3].clone();
    let note = f.note(1, &s).await;
    let mut rows = vec![];
    for cap in [
        "space.read",
        "content.write",
        "plugins.use",
        "plugins.manage",
        "members.invite",
        "members.manage",
        "managers.manage",
        "space.settings",
        "secrets.manage",
        "space.transfer",
        "space.delete",
    ] {
        f.stop();
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        c.execute(
            "DELETE FROM space_role_capabilities WHERE role='editor'",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO space_role_capabilities VALUES('editor',?1)",
            [cap],
        )
        .unwrap();
        drop(c);
        f.start().await;
        for (op, input, allowed) in [
            ("notes.get", json!({"id":note["id"]}), cap == "space.read"),
            (
                "notes.create",
                json!({"space_id":s,"title":"capability","markdown":"x"}),
                cap == "content.write",
            ),
            (
                "plugins.enable",
                json!({"space_id":s,"plugin_id":"calternal-notes-insights"}),
                cap == "plugins.manage",
            ),
            (
                "spaces.transfer",
                json!({"space_id":s,"account_id":f.ids[1]}),
                false,
            ),
            (
                "secrets.list",
                json!({"space_id":s}),
                cap == "secrets.manage",
            ),
            ("spaces.delete", json!({"space_id":s}), false),
            (
                "spaces.set_role",
                json!({"space_id":s,"account_id":f.ids[2],"role":"manager"}),
                false,
            ),
        ] {
            let r = f.op(2, op, input).await;
            assert_eq!(r.status == 200, allowed, "single {cap}: {op} {}", r.body);
            rows.push(json!({"capability":cap,"op":op,"status":r.status}));
        }
    }
    evidence("capabilities", json!(rows));
}

#[tokio::test]
async fn all_operations_reject_revoked_and_expired_sessions_before_dispatch() {
    let mut f = Fixture::new().await;
    let d = f
        .req(
            "GET",
            "/api/v1/describe",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    d.ok();
    let catalog = d.body["operations"].as_array().unwrap();
    f.auth("logout", Some(&f.cookies[1]), json!({})).await;
    for op in catalog {
        let r = f.op(1, op["id"].as_str().unwrap(), json!({})).await;
        assert_eq!(r.status, 401, "revoked {}", op["id"]);
    }
    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    c.execute(
        "UPDATE installations SET expires_at=1 WHERE user_id=?1",
        [&f.ids[2]],
    )
    .unwrap();
    drop(c);
    f.start().await;
    for op in catalog {
        let r = f.op(2, op["id"].as_str().unwrap(), json!({})).await;
        assert_eq!(r.status, 401, "expired {}", op["id"]);
    }
}

#[tokio::test]
async fn host_method_and_api_fallback_do_not_expose_resources() {
    let f = Fixture::new().await;
    for path in [
        "/api",
        "/api/",
        "/api/does-not-exist",
        "/api/v1/auth/not-a-route",
    ] {
        let a = f
            .req("GET", path, Some(&f.cookies[1]), json!({}), Some(ORIGIN))
            .await;
        let b = f
            .req("GET", path, Some(&f.cookies[0]), json!({}), Some(ORIGIN))
            .await;
        assert_eq!(a.status, 404);
        assert_eq!(a.signature(), b.signature());
    }
    for (method, path) in [
        ("GET", "/api/v1/operations"),
        ("POST", "/api/v1/describe"),
        ("OPTIONS", "/api/v1/operations"),
    ] {
        let r = f
            .req(method, path, Some(&f.cookies[1]), json!({}), Some(ORIGIN))
            .await;
        assert_eq!(r.status, 405);
        assert!(!r.headers.contains_key("access-control-allow-origin"));
    }
    for host in [
        "evil.invalid",
        "localhost:3210",
        "localhost:18080.evil.invalid",
    ] {
        let r = f
            .client
            .get(format!("{}/api/v1/describe", f.base))
            .header("host", host)
            .header("cookie", &f.cookies[1])
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 403);
    }
    let r = f
        .client
        .post(format!("{}/api/v1/operations", f.base))
        .header("host", "localhost:18080")
        .header("cookie", &f.cookies[1])
        .header("content-type", "text/plain")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 400);
}

#[tokio::test]
async fn parallel_setup_and_login_ceremonies_are_take_once() {
    let mut f = Fixture::new().await;
    f.stop();
    let auth = Auth::open(&f.appdata(), AuthConfig::new(ORIGIN).unwrap()).unwrap();
    let token = auth.create("Setup race").await.unwrap().token;
    drop(auth);
    f.start().await;
    let a = f.auth("setup/start", None, json!({"token":token})).await;
    a.ok();
    let b = f.auth("setup/start", None, json!({"token":token})).await;
    b.ok();
    let mut da = Device::new();
    let mut db = Device::new();
    let ca = da.register(&a);
    let cb = db.register(&b);
    let (a, b) = tokio::join!(
        f.auth(
            "setup/finish",
            None,
            json!({"ceremony":a.body["ceremony"],"credential":ca})
        ),
        f.auth(
            "setup/finish",
            None,
            json!({"ceremony":b.body["ceremony"],"credential":cb})
        )
    );
    assert_eq!(
        [a.status, b.status].iter().filter(|s| **s == 200).count(),
        1
    );
    let start = f.auth("login/start", None, json!({})).await;
    start.ok();
    let cred = f.devices[1].sign(&start);
    let body = json!({"ceremony":start.body["ceremony"],"credential":cred});
    let (a, b) = tokio::join!(
        f.auth("login/finish", None, body.clone()),
        f.auth("login/finish", None, body)
    );
    assert_eq!(
        [a.status, b.status].iter().filter(|s| **s == 200).count(),
        1
    );
    evidence(
        "parallel-ceremonies",
        json!({"login_a":a.status,"login_b":b.status}),
    );
}

#[tokio::test]
async fn anonymous_sessions_are_refused_on_every_protected_route_and_operation() {
    let f = Fixture::new().await;
    // This source inventory guard detects added account routes even when an old
    // hand-written route list would otherwise continue to pass.
    let source = include_str!("../../crates/server/src/account_http.rs");
    let mut registered: Vec<_> = source
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix(".route(\"/api/v1/auth/")
                .and_then(|s| s.split('"').next())
        })
        .collect();
    let mut covered: Vec<_> = AUTH_ROUTES.iter().map(|(_, name)| *name).collect();
    registered.sort_unstable();
    covered.sort_unstable();
    assert_eq!(registered, covered, "account route inventory changed");
    let catalog = f
        .req(
            "GET",
            "/api/v1/describe",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    catalog.ok();
    for (method, name) in AUTH_ROUTES {
        if [
            "status",
            "setup/start",
            "setup/finish",
            "login/start",
            "login/finish",
            "logout",
        ]
        .contains(name)
        {
            continue;
        }
        let r = f
            .req(
                method,
                &format!("/api/v1/auth/{name}"),
                None,
                json!({}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(r.status, 401, "anonymous {name}: {}", r.body);
    }
    assert_eq!(
        f.req("GET", "/api/v1/describe", None, json!({}), Some(ORIGIN))
            .await
            .status,
        401
    );
    for op in catalog.body["operations"].as_array().unwrap() {
        let r = f
            .req(
                "POST",
                "/api/v1/operations",
                None,
                json!({"request_id":"anonymous","operation":op["id"],"input":{}}),
                Some(ORIGIN),
            )
            .await;
        assert_eq!(r.status, 401, "anonymous {}: {}", op["id"], r.body);
    }
}

#[tokio::test]
async fn directory_receipt_retries_recheck_current_membership() {
    let f = Fixture::new().await;
    let s = &f.spaces[4];
    let input = json!({"space_id":s,"account_id":f.ids[4],"role":"editor"});
    f.op_id(
        2,
        "spaces.set_role",
        input.clone(),
        "directory-before-demotion",
    )
    .await
    .ok();
    f.op(
        1,
        "spaces.set_role",
        json!({"space_id":s,"account_id":f.ids[2],"role":"reader"}),
    )
    .await
    .ok();
    assert_eq!(
        f.op_id(
            2,
            "spaces.set_role",
            input.clone(),
            "directory-before-demotion"
        )
        .await
        .status,
        403
    );
    f.op(
        1,
        "spaces.remove_member",
        json!({"space_id":s,"account_id":f.ids[2]}),
    )
    .await
    .ok();
    let r = f
        .op_id(2, "spaces.set_role", input, "directory-before-demotion")
        .await;
    evidence(
        "directory-replay",
        json!({"after_removal":r.status,"body":r.body}),
    );
    // Existence hiding is checked separately; here no success/receipt may escape.
    assert!([403, 404].contains(&r.status));
}

#[tokio::test]
async fn managers_manage_is_required_even_for_owner_target_changes() {
    let mut f = Fixture::new().await;
    let s = f.spaces[4].clone();
    f.stop();
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    c.execute(
        "DELETE FROM space_role_capabilities WHERE role='owner' AND capability='managers.manage'",
        [],
    )
    .unwrap();
    drop(c);
    f.start().await;
    for (op, input) in [
        (
            "spaces.set_role",
            json!({"space_id":s,"account_id":f.ids[2],"role":"reader"}),
        ),
        (
            "spaces.remove_member",
            json!({"space_id":s,"account_id":f.ids[2]}),
        ),
        (
            "spaces.set_role",
            json!({"space_id":s,"account_id":f.ids[4],"role":"manager"}),
        ),
    ] {
        let r = f.op(1, op, input).await;
        assert_eq!(r.status, 403, "{op}: {}", r.body);
    }
}

// Real SIGKILL scheduling explores acknowledged and uncertain requests. There is
// no auth transaction fault hook: these are crash consistency regressions, not a
// claim that every internal persistence boundary was interrupted deterministically.
#[tokio::test]
async fn interrupted_authority_and_login_are_atomic_with_receipts() {
    let mut rows = vec![];
    for scenario in ["spaces.set_role", "spaces.transfer", "login"] {
        for delay_us in [0, 300, 2_000, 10_000] {
            let mut f = Fixture::new().await;
            let space = f.spaces[3].clone();
            let request_id = uuid::Uuid::now_v7().to_string();
            let (path, body) = if scenario == "login" {
                let start = f.auth("login/start", None, json!({})).await;
                start.ok();
                let cred = f.devices[1].sign(&start);
                (
                    "/api/v1/auth/login/finish",
                    json!({"ceremony":start.body["ceremony"],"credential":cred,"device":"interrupted-login"}),
                )
            } else {
                let mut input = json!({"space_id":space,"account_id":f.ids[2]});
                if scenario == "spaces.set_role" {
                    input["role"] = json!("reader");
                }
                (
                    "/api/v1/operations",
                    json!({"request_id":request_id,"operation":scenario,"input":input}),
                )
            };
            let client = f.client.clone();
            let base = f.base.clone();
            let cookie = f.cookies[1].clone();
            let task = tokio::spawn(async move {
                let response = client
                    .post(format!("{base}{path}"))
                    .header("host", "localhost:18080")
                    .header("origin", ORIGIN)
                    .header("cookie", cookie)
                    .json(&body)
                    .send()
                    .await
                    .ok()?;
                let status = response.status().as_u16();
                let cookie = response
                    .headers()
                    .get("set-cookie")
                    .map(|c| c.to_str().unwrap().split(';').next().unwrap().to_owned());
                let body = response.json::<Value>().await.ok()?;
                Some((status, cookie, body))
            });
            tokio::task::yield_now().await;
            if delay_us > 0 {
                tokio::time::sleep(Duration::from_micros(delay_us)).await;
            }
            f.stop();
            let response = task.await.unwrap();
            f.start().await;
            for db in ["system/content-index.db", "system/auth.db"] {
                let c = Connection::open_with_flags(
                    f.appdata().join(db),
                    rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
                )
                .unwrap();
                assert_eq!(
                    c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                        .unwrap(),
                    "ok"
                );
                assert!(
                    !c.prepare("PRAGMA foreign_key_check")
                        .unwrap()
                        .exists([])
                        .unwrap()
                );
            }
            let c = Connection::open_with_flags(
                f.appdata().join("system/auth.db"),
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .unwrap();
            if scenario == "login" {
                let count:i64=c.query_row("SELECT COUNT(*) FROM installations WHERE user_id=?1 AND name='interrupted-login'",[&f.ids[1]],|r|r.get(0)).unwrap();
                assert!([0, 1].contains(&count));
                if let Some((200, Some(cookie), _)) = &response {
                    assert_eq!(count, 1, "acknowledged sign-in disappeared");
                    f.req(
                        "GET",
                        "/api/v1/auth/me",
                        Some(cookie),
                        json!({}),
                        Some(ORIGIN),
                    )
                    .await
                    .ok();
                }
                f.login(1).await.ok();
                rows.push(json!({"scenario":scenario,"delay_us":delay_us,"ack_status":response.as_ref().map(|r|r.0),"new_sessions":count}));
            } else {
                let receipt:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM space_receipts WHERE actor=?1 AND scope=?2 AND request_id=?3)",params![f.ids[1],space,request_id],|r|r.get(0)).unwrap();
                let owner: String = c
                    .query_row(
                        "SELECT owner_account FROM spaces WHERE id=?1",
                        [&space],
                        |r| r.get(0),
                    )
                    .unwrap();
                let role: String = c
                    .query_row(
                        "SELECT role FROM space_memberships WHERE space_id=?1 AND account_id=?2",
                        params![space, f.ids[2]],
                        |r| r.get(0),
                    )
                    .unwrap();
                if scenario == "spaces.transfer" {
                    assert_eq!(
                        owner,
                        if receipt {
                            f.ids[2].clone()
                        } else {
                            f.ids[1].clone()
                        }
                    );
                    assert_eq!(role, if receipt { "manager" } else { "editor" });
                } else {
                    assert_eq!(owner, f.ids[1]);
                    assert_eq!(role, if receipt { "reader" } else { "editor" });
                }
                if response.as_ref().is_some_and(|r| r.0 == 200) {
                    assert!(receipt, "acknowledged mutation disappeared");
                }
                let members = f
                    .op(
                        if scenario == "spaces.transfer" && receipt {
                            2
                        } else {
                            1
                        },
                        "spaces.members",
                        json!({"space_id":space}),
                    )
                    .await;
                members.ok();
                assert_eq!(
                    members.body["data"]["members"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|m| m["role"] == "owner")
                        .count(),
                    1
                );
                rows.push(json!({"scenario":scenario,"delay_us":delay_us,"ack_status":response.as_ref().map(|r|r.0),"receipt":receipt,"role":role}));
            }
        }
    }
    evidence("atomic-crashes", json!(rows));
}

#[tokio::test]
async fn manager_cannot_demote_or_remove_another_manager() {
    let f = Fixture::new().await;
    let space = &f.spaces[4];
    f.op(
        1,
        "spaces.set_role",
        json!({"space_id":space,"account_id":f.ids[4],"role":"manager"}),
    )
    .await
    .ok();
    for (op, input) in [
        (
            "spaces.set_role",
            json!({"space_id":space,"account_id":f.ids[4],"role":"reader"}),
        ),
        (
            "spaces.remove_member",
            json!({"space_id":space,"account_id":f.ids[4]}),
        ),
    ] {
        assert_eq!(f.op(2, op, input).await.status, 403);
    }
    f.op(
        1,
        "spaces.set_role",
        json!({"space_id":space,"account_id":f.ids[4],"role":"reader"}),
    )
    .await
    .ok();
    f.op(
        2,
        "spaces.remove_member",
        json!({"space_id":space,"account_id":f.ids[4]}),
    )
    .await
    .ok();
}

#[tokio::test]
async fn running_worker_expired_session_blocks_delivery() {
    running_worker_revoke(WorkerRevocation::Expiry).await;
}

/// Machine routes as registered in `crates/server/src/machines/http.rs`.
const MACHINE_ROUTES: &[(&str, &str)] = &[
    ("POST", "/api/v1/machine/enroll"),
    ("POST", "/api/v1/machine/session/start"),
    ("POST", "/api/v1/machine/session/finish"),
    ("POST", "/api/v1/machine/sync"),
    ("POST", "/api/v1/machine/output"),
    ("GET", "/api/v1/machines"),
    ("POST", "/api/v1/machines/codes"),
    ("POST", "/api/v1/machines/{id}/revoke"),
    ("POST", "/api/v1/machines/{id}/drain"),
    ("POST", "/api/v1/machines/runs"),
    ("GET", "/api/v1/machines/runs/{lease}"),
    ("POST", "/api/v1/machines/runs/{lease}/stop"),
    ("POST", "/api/v1/machines/runs/{lease}/execs"),
    ("GET", "/api/v1/machines/execs/{exec}/output"),
];

impl Fixture {
    /// Machine-protocol request with an optional machine bearer and no cookie.
    async fn machine(&self, path: &str, bearer: Option<&str>, body: Value) -> Answer {
        let mut r = self
            .client
            .post(format!("{}{path}", self.base))
            .header("host", "localhost:18080")
            .json(&body);
        if let Some(b) = bearer {
            r = r.bearer_auth(b);
        }
        let r = r.send().await.unwrap();
        let status = r.status().as_u16();
        let text = r.text().await.unwrap();
        Answer {
            status,
            body: serde_json::from_str(&text).unwrap_or(json!({"raw":text})),
            headers: BTreeMap::new(),
            micros: 0,
        }
    }
}

#[tokio::test]
async fn machines_and_runs_are_owner_private() {
    use calternal_machine::identity::MachineKey;
    let f = Fixture::new().await;
    // Source inventory guard: a new machine route needs a case here.
    let source = include_str!("../../crates/server/src/machines/http.rs");
    // Every quoted machine path in that file is a route (rustfmt may wrap `.route(`).
    let mut registered: Vec<_> = source
        .split('"')
        .skip(1)
        .step_by(2)
        .filter(|s| s.starts_with("/api/v1/machine"))
        .collect();
    let mut covered: Vec<_> = MACHINE_ROUTES.iter().map(|(_, p)| *p).collect();
    registered.sort_unstable();
    covered.sort_unstable();
    assert_eq!(
        registered, covered,
        "machine route inventory changed; add conformance cases"
    );

    // Account A (1) enrols a machine with a code it issued; the code works once.
    let code = f
        .req(
            "POST",
            "/api/v1/machines/codes",
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    code.ok();
    let key = MachineKey::generate().unwrap();
    let enrol = json!({"code":code.body["code"],"public_key":key.public_key_b64(),"name":"a","os":"linux","arch":"x86_64","direct":false});
    let first = f
        .machine("/api/v1/machine/enroll", None, enrol.clone())
        .await;
    first.ok();
    let replay = f.machine("/api/v1/machine/enroll", None, enrol).await;
    assert_eq!(replay.status, 404, "{}", replay.body);
    let machine: uuid::Uuid = first.body["machine_id"].as_str().unwrap().parse().unwrap();
    // A session needs the machine key.
    let start = f
        .machine(
            "/api/v1/machine/session/start",
            None,
            json!({"machine_id":machine}),
        )
        .await;
    start.ok();
    let challenge = start.body["challenge"].as_str().unwrap().to_owned();
    let forged = MachineKey::generate()
        .unwrap()
        .sign_session(machine, &challenge);
    let finish = |sig: String, ch: &str| json!({"machine_id":machine,"challenge":ch,"signature":sig,"boot_id":uuid::Uuid::now_v7(),"protocol":calternal_machine::protocol::PROTOCOL_VERSION,"backend":"podman"});
    assert_eq!(
        f.machine(
            "/api/v1/machine/session/finish",
            None,
            finish(forged, &challenge)
        )
        .await
        .status,
        401
    );
    let start = f
        .machine(
            "/api/v1/machine/session/start",
            None,
            json!({"machine_id":machine}),
        )
        .await;
    let challenge = start.body["challenge"].as_str().unwrap().to_owned();
    let session = f
        .machine(
            "/api/v1/machine/session/finish",
            None,
            finish(key.sign_session(machine, &challenge), &challenge),
        )
        .await;
    session.ok();
    let token = session.body["token"].as_str().unwrap().to_owned();
    // Sync and output refuse a missing or unknown bearer.
    let sync = json!({"ack":0,"capacity":{"cpus":1,"memory_bytes":0,"max_sandboxes":2,"running_sandboxes":0},"draining":false,"leases":[]});
    for bearer in [None, Some("cm1_unknown")] {
        assert_eq!(
            f.machine("/api/v1/machine/sync", bearer, sync.clone())
                .await
                .status,
            401
        );
        let out = json!({"lease_id":machine,"fence":1,"exec_id":machine,"chunks":[]});
        assert_eq!(
            f.machine("/api/v1/machine/output", bearer, out)
                .await
                .status,
            401
        );
    }

    // A starts a run in its own space; the queued exec goes out with the assignment.
    let run = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[1]),
            json!({"space_id":f.spaces[0],"thread_id":uuid::Uuid::now_v7()}),
            Some(ORIGIN),
        )
        .await;
    run.ok();
    let lease = run.body["lease_id"].as_str().unwrap().to_owned();
    let exec = f
        .req(
            "POST",
            &format!("/api/v1/machines/runs/{lease}/execs"),
            Some(&f.cookies[1]),
            json!({"argv":["true"]}),
            Some(ORIGIN),
        )
        .await;
    exec.ok();
    let exec = exec.body["exec_id"].as_str().unwrap().to_owned();
    let assigned = f
        .machine("/api/v1/machine/sync", Some(&token), sync.clone())
        .await;
    assigned.ok();
    assert_eq!(
        assigned.body["messages"][0]["message"]["type"], "assign",
        "{}",
        assigned.body
    );

    // Everyone else gets the same answer for A's ids as for ids that do not exist.
    let missing = uuid::Uuid::now_v7().to_string();
    let paths = |m: &str, l: &str, e: &str| {
        vec![
            ("POST", format!("/api/v1/machines/{m}/revoke"), json!({})),
            (
                "POST",
                format!("/api/v1/machines/{m}/drain"),
                json!({"draining":true}),
            ),
            ("GET", format!("/api/v1/machines/runs/{l}"), json!({})),
            ("POST", format!("/api/v1/machines/runs/{l}/stop"), json!({})),
            (
                "POST",
                format!("/api/v1/machines/runs/{l}/execs"),
                json!({"argv":["id"]}),
            ),
            (
                "GET",
                format!("/api/v1/machines/execs/{e}/output"),
                json!({}),
            ),
            (
                "POST",
                "/api/v1/machines/runs".into(),
                json!({"space_id":if m == missing { missing.clone() } else { f.spaces[0].clone() },"thread_id":uuid::Uuid::now_v7()}),
            ),
        ]
    };
    let mine = paths(&machine.to_string(), &lease, &exec);
    let absent = paths(&missing, &missing, &missing);
    for who in [0, 2, 3] {
        for ((m, p, b), (_, q, c)) in mine.iter().zip(&absent) {
            let x = f
                .req(m, p, Some(&f.cookies[who]), b.clone(), Some(ORIGIN))
                .await;
            let y = f
                .req(m, q, Some(&f.cookies[who]), c.clone(), Some(ORIGIN))
                .await;
            assert_eq!(
                (x.status, x.signature()),
                (404, y.signature()),
                "{who} {m} {p}"
            );
        }
        let list = f
            .req(
                "GET",
                "/api/v1/machines",
                Some(&f.cookies[who]),
                json!({}),
                Some(ORIGIN),
            )
            .await;
        list.ok();
        assert_eq!(list.body["machines"], json!([]), "account {who}");
    }
    // Runs need plugins.use: B is a reader in spaces[2] (refused) and an editor in spaces[3].
    let as_reader = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[2]),
            json!({"space_id":f.spaces[2],"thread_id":uuid::Uuid::now_v7()}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(as_reader.status, 404);
    let as_editor = f
        .req(
            "POST",
            "/api/v1/machines/runs",
            Some(&f.cookies[2]),
            json!({"space_id":f.spaces[3],"thread_id":uuid::Uuid::now_v7()}),
            Some(ORIGIN),
        )
        .await;
    as_editor.ok();
    // A's machine has room but is not B's: B's run waits.
    assert_eq!(as_editor.body["state"], "waiting");
    // Anonymous callers are refused on every account route.
    for (m, p, b) in &mine {
        assert_eq!(
            f.req(m, p, None, b.clone(), Some(ORIGIN)).await.status,
            401,
            "{p}"
        );
    }
    for (m, p) in [
        ("GET", "/api/v1/machines"),
        ("POST", "/api/v1/machines/codes"),
    ] {
        assert_eq!(
            f.req(m, p, None, json!({}), Some(ORIGIN)).await.status,
            401,
            "{p}"
        );
    }
    // Revocation is immediate: the session dies and A's run ends.
    let revoked = f
        .req(
            "POST",
            &format!("/api/v1/machines/{machine}/revoke"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    revoked.ok();
    assert_eq!(
        f.machine("/api/v1/machine/sync", Some(&token), sync)
            .await
            .status,
        401
    );
    let run = f
        .req(
            "GET",
            &format!("/api/v1/machines/runs/{lease}"),
            Some(&f.cookies[1]),
            json!({}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(run.body["state"], "ended");
}

#[tokio::test]
async fn secret_management_is_separate_from_use_and_space_settings() {
    let mut f = Fixture::new().await;
    let space = f.spaces[3].clone();
    for cap in [
        "space.read",
        "space.settings",
        "plugins.use",
        "secrets.manage",
    ] {
        let request = f
            .op(
                1,
                "secrets.request",
                json!({"space_id":space,"name":"cap-test","label":"Key","reason":"test"}),
            )
            .await;
        request.ok();
        let request = request.body["data"]["request_id"]
            .as_str()
            .unwrap()
            .to_owned();
        f.stop();
        let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
        c.execute(
            "DELETE FROM space_role_capabilities WHERE role='editor'",
            [],
        )
        .unwrap();
        c.execute(
            "INSERT INTO space_role_capabilities VALUES('editor',?1)",
            [cap],
        )
        .unwrap();
        drop(c);
        f.start().await;
        let list = f.op(2, "secrets.list", json!({"space_id":space})).await;
        assert_eq!(
            list.status,
            if cap == "secrets.manage" { 200 } else { 403 },
            "{cap}: {}",
            list.body
        );
        let provide = f
            .auth(
                "secrets/provide",
                Some(&f.cookies[2]),
                json!({"space_id":space,"request_id":request,"value":SECRET_CANARY}),
            )
            .await;
        assert_eq!(
            provide.status,
            if cap == "secrets.manage" { 200 } else { 403 },
            "{cap}: {}",
            provide.body
        );
        let revoke = f
            .op(
                2,
                "secrets.revoke",
                if cap == "secrets.manage" {
                    json!({"space_id":space,"name":"cap-test"})
                } else {
                    json!({"space_id":space,"request_id":request})
                },
            )
            .await;
        assert_eq!(
            revoke.status,
            if cap == "secrets.manage" { 200 } else { 403 },
            "{cap}: {}",
            revoke.body
        );
        let ask = f
            .op(
                2,
                "secrets.request",
                json!({"space_id":space,"name":"use-test","label":"Key","reason":"test"}),
            )
            .await;
        assert_eq!(
            ask.status,
            if cap == "plugins.use" { 200 } else { 403 },
            "{cap}: {}",
            ask.body
        );
    }
    let key = f.appdata().join("spaces").join(&space).join("secret.key");
    assert!(key.exists());
    let deleted = f.op(1, "spaces.delete", json!({"space_id":space})).await;
    deleted.ok();
    assert!(
        !key.exists(),
        "successful owner deletion must erase its key"
    );
    let c = Connection::open(f.appdata().join("system/auth.db")).unwrap();
    assert!(
        c.query_row(
            "SELECT erasing FROM owner_secret_keys WHERE kind='space' AND owner_id=?1",
            [&space],
            |r| r.get::<_, bool>(0)
        )
        .unwrap()
    );
}
