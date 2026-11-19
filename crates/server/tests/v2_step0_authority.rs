//! Scripted client and launch command for the disposable sealed host.
#[path = "../../../spikes/plugin-api-v2/authority/mod.rs"]
mod authority;
#[path = "v2_step0_authority/chrome.rs"]
mod chrome;
#[path = "v2_step0_authority/crash.rs"]
mod crash;
#[path = "../../../spikes/plugin-api-v2/fixture.rs"]
mod fixture;
#[path = "v2_step0_authority/script.rs"]
mod script;

use authority::Host;
use reqwest::Client;
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

struct ClientFixture {
    host: Arc<Host>,
    http: Client,
    base: String,
    owner: String,
    guest: String,
    task: tokio::task::JoinHandle<()>,
    _root: tempfile::TempDir,
}

fn token(path: &Path, name: &str) -> String {
    std::fs::read_to_string(path.join(format!("appdata/dev-tokens/{name}.token"))).unwrap()
}

impl ClientFixture {
    async fn new() -> Self {
        Self::with_rows(12).await
    }

    async fn with_rows(size: usize) -> Self {
        let root = tempfile::tempdir().unwrap();
        let host = Host::create_sized(&fixture::roots(root.path()), size).await;
        let owner = token(root.path(), "owner");
        let guest = token(root.path(), "guest");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/step0/v2", listener.local_addr().unwrap());
        let app = host.clone().router();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let http = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        Self {
            host,
            http,
            base,
            owner,
            guest,
            task,
            _root: root,
        }
    }

    async fn bootstrap(&self, token: &str) -> Value {
        self.http
            .get(format!("{}/fixture", self.base))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn call(
        &self,
        token: &str,
        route: &str,
        lease: Option<&Value>,
        input: Value,
    ) -> (u16, Value) {
        let mut req = self
            .http
            .post(format!("{}/{route}", self.base))
            .bearer_auth(token)
            .json(&input);
        if let Some(lease) = lease {
            req = req.header("x-step0-view-lease", lease.as_str().unwrap());
        }
        let reply = req.send().await.unwrap();
        let status = reply.status().as_u16();
        (status, reply.json().await.unwrap())
    }

    async fn ok(&self, token: &str, route: &str, lease: Option<&Value>, input: Value) -> Value {
        let (status, result) = self.call(token, route, lease, input).await;
        assert_eq!(status, 200, "{route} {result}");
        result
    }
}
impl Drop for ClientFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn open(id: &Value) -> Value {
    json!({"type":"view_open","view_ref":id})
}
fn window() -> Value {
    json!({"type":"window","request":{"bind":"notes","from":0,"to":20}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable Step 0 authority host smoke"]
async fn sealed_view_and_window() {
    let f = ClientFixture::new().await;
    let routes = f.bootstrap(&f.owner).await;
    let own = f
        .ok(&f.owner, "view.open", None, open(&routes["views"]["home"]))
        .await;
    assert_eq!(own["audience"], "own_space_member");
    let rows = f
        .ok(
            &f.owner,
            "window",
            Some(&own["lease"]["lease_ref"]),
            window(),
        )
        .await;
    assert_eq!(rows["rows"].as_array().unwrap().len(), 4);
    assert_eq!(rows["rows"][0]["fields"]["title"], "Project brief");
    let tail = f
        .ok(
            &f.owner,
            "window",
            Some(&own["lease"]["lease_ref"]),
            json!({"type":"window","request":{"bind":"notes","from":0,"to":1,"anchor":"end"}}),
        )
        .await;
    assert_eq!(tail["anchor"], "end");
    assert_eq!(tail["rows"][0]["id"], rows["rows"][3]["id"]);

    let member = f
        .ok(
            &f.guest,
            "view.open",
            None,
            open(&routes["views"]["shared"]),
        )
        .await;
    let visible = f
        .ok(
            &f.guest,
            "window",
            Some(&member["lease"]["lease_ref"]),
            window(),
        )
        .await;
    assert_eq!(visible["audience"], "member");
    assert_eq!(visible["rows"].as_array().unwrap().len(), 4);
    assert_eq!(visible["activation_contexts"], json!([]));
    assert_eq!(
        f.call(&f.guest, "view.open", None, open(&routes["views"]["home"]))
            .await,
        (404, json!({"error":"not_found"}))
    );
    let grant = f.host.sharing(true).await.unwrap();
    let guest = f
        .ok(&f.guest, "view.open", None, open(&routes["views"]["home"]))
        .await;
    let lease = &guest["lease"]["lease_ref"];
    let visible = f.ok(&f.guest, "window", Some(lease), window()).await;
    assert_eq!(visible["audience"], "guest");
    assert_eq!(visible["rows"].as_array().unwrap().len(), 1);
    assert!(visible.get("count").is_none());
    assert_eq!(
        visible["rows"][0]["fields"],
        json!({"title":"Project brief"})
    );
    assert_eq!(
        f.bootstrap(&f.guest).await["templates"]["home"]["binds"][0]["fields"],
        json!(["title"])
    );
    assert_eq!(
        f.call(
            &f.guest,
            "window",
            Some(lease),
            json!({"type":"window","request":{"bind":"notes","from":0,"to":1,"anchor":"end"}})
        )
        .await,
        (
            422,
            json!({"status":422,"code":"unsupported_query_plan","reason":"end_unavailable"})
        )
    );

    f.host.unshare(grant).await;
    assert_eq!(
        f.call(&f.guest, "window", Some(lease), window()).await,
        (404, json!({"error":"not_found"}))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "serve a disposable authority fixture for renderer integration"]
async fn serve_authority_fixture() {
    let root = tempfile::tempdir().unwrap();
    let size = if std::env::var("STEP0_AUTHORITY_SCALE").as_deref() == Ok("1") {
        5000
    } else {
        12
    };
    let churn = std::env::var("STEP0_AUTHORITY_CHURN_HZ").map_or(0, |s| s.parse::<u64>().unwrap());
    assert!(churn <= 10, "fixture churn must be 0–10 Hz");
    let host = Host::create_sized(&fixture::roots(root.path()), size).await;
    let churn_task = host.start_churn(churn);
    let port = std::env::var("STEP0_AUTHORITY_PORT").map_or(43125, |s| s.parse::<u16>().unwrap());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    println!("BASE http://{}/step0/v2", listener.local_addr().unwrap());
    for name in ["owner", "guest"] {
        let path: PathBuf = root.path().join(format!("appdata/dev-tokens/{name}.token"));
        println!("{name} bearer file {}", path.display());
    }
    println!(
        "Use GET /step0/v2/fixture with a bearer to discover authorized view refs and the template."
    );
    axum::serve(listener, host.router())
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.unwrap();
        })
        .await
        .unwrap();
    if let Some(task) = churn_task {
        task.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "disposable job claim, park and protected approval races"]
async fn sealed_queue_authority() {
    let f = ClientFixture::new().await;
    let repeats = std::env::var("STEP0_QUEUE_RUNS").map_or(10, |n| n.parse().unwrap());
    f.host.queue_races(repeats).await;
}
