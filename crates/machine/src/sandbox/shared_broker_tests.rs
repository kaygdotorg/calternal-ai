//! The Claude and Codex fixtures share the machine-local broker and Podman boundary.
use super::*;
use std::{
    sync::atomic::{AtomicU32, Ordering},
    time::Instant,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, UnixStream},
};

#[test]
fn inference_exposes_only_the_fixed_tool_and_rejects_other_calls() {
    let input = br#"{"tools":[{"type":"namespace","name":"mcp__workspace","tools":[{"type":"function","name":"workspace_command"},{"type":"function","name":"hidden"}]},{"type":"function","name":"shell"},{"type":"web_search"}]}"#;
    let output: serde_json::Value =
        serde_json::from_slice(&isolate_request(input, SharedToolsProvider::Codex).unwrap())
            .unwrap();
    assert_eq!(
        output,
        serde_json::json!({"tools":[{"name":"mcp__workspace","tools":[{"name":"workspace_command","type":"function"}],"type":"namespace"}]})
    );
    assert!(isolate_request(br#"{"tools":"invalid"}"#, SharedToolsProvider::Claude).is_none());
    assert_eq!(
        isolate_request(
            br#"{"mcp_servers":[{"name":"remote"}]}"#,
            SharedToolsProvider::Claude
        ),
        Some(b"{}".to_vec())
    );
    assert!(check_event(b"data: [DONE]\r\n\r\n").is_ok());
    assert!(valid_reply(br#"{"item":{"type":"function_call","namespace":"mcp__workspace","name":"workspace_command"}}"#));
    assert!(!valid_reply(
        br#"{"item":{"type":"function_call","namespace":"mcp__other","name":"workspace_command"}}"#
    ));
    assert!(!valid_reply(
        br#"{"item":{"type":"function_call","name":"apply_patch"}}"#
    ));
    assert!(
        check_event(b"data: {\"content_block\":{\"type\":\"tool_use\",\"name\":\"Bash\"}}\n\n")
            .is_err()
    );
}

async fn unix_request(broker: &Broker, path: &str, token: &str, payload: &str) -> String {
    let mut stream = UnixStream::connect(broker.directory.join("gateway.sock"))
        .await
        .unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut body = String::new();
    stream.read_to_string(&mut body).await.unwrap();
    body
}

fn bridge_fixture(broker: &Broker) -> PathBuf {
    let fixture = broker.directory.join("test-client.mjs");
    let source = include_str!("shared_broker_client.mjs").replace(
        "/run/calternal-broker/gateway.sock",
        broker.directory.join("gateway.sock").to_str().unwrap(),
    );
    std::fs::write(&fixture, source).unwrap();
    fixture
}

async fn bridge_gateway(broker: &Broker, fixture: &Path) -> (tokio::process::Child, String) {
    let mut gateway = Command::new("node")
        .args(["--input-type=module", "-e",
            "const bridge = await import(process.argv[1]); console.log((await bridge.startGateway()).baseUrl);"])
        .arg(fixture)
        .env("CALTERNAL_GATEWAY_TOKEN", &broker.access)
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let address = BufReader::new(gateway.stdout.take().unwrap())
        .lines()
        .next_line()
        .await
        .unwrap()
        .unwrap();
    (gateway, address)
}

async fn mcp_tool(broker: &Broker, fixture: &Path, command: &str) -> serde_json::Value {
    let mut child = Command::new("node")
        .arg(fixture)
        .arg("mcp")
        .env("CALTERNAL_GATEWAY_TOKEN", &broker.access)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let invocation = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {"name": CODEX_METHOD, "arguments": {"command": command}},
    });
    let mut input = child.stdin.take().unwrap();
    input
        .write_all(format!("{invocation}\n").as_bytes())
        .await
        .unwrap();
    drop(input);
    let output = child.wait_with_output().await.unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn shared_broker_keeps_claude_and_codex_credentials_out_of_tools() {
    let supplied = std::env::var("CALTERNAL_TEST_SHARED_BROKER_IMAGE").ok();
    let image = supplied
        .as_deref()
        .unwrap_or("docker.io/library/node:22-bookworm-slim");
    let present = Command::new("podman")
        .args(["image", "exists", image])
        .status()
        .await
        .is_ok_and(|status| status.success());
    if !present {
        assert!(
            supplied.is_none(),
            "the requested Podman fixture is unavailable"
        );
        eprintln!("SKIPPED: the local Podman Node image is unavailable");
        return;
    }
    for provider in [SharedToolsProvider::Claude, SharedToolsProvider::Codex] {
        let dir = tempfile::tempdir().unwrap();
        let space = SpaceId::new();
        let keys = dir
            .path()
            .join("broker-credentials")
            .join(space.to_string());
        let gateway_dir = dir.path().join("socket");
        let work = dir.path().join("work");
        for path in [&keys, &gateway_dir, &work] {
            calternal_fsio::private::directory(path).unwrap();
        }
        let key = format!("synthetic-{}", Uuid::new_v4());
        let (filename, path, header_name, expected, payload) = match provider {
            SharedToolsProvider::Claude => (
                "anthropic.key",
                "/v1/messages",
                "x-api-key",
                key.clone(),
                r#"{"messages":[],"tools":[{"name":"mcp__workspace__workspace_command"}]}"#,
            ),
            SharedToolsProvider::Codex => (
                "openai.key",
                "/v1/responses",
                "authorization",
                format!("Bearer {key}"),
                r#"{"input":[],"tools":[{"type":"namespace","name":"mcp__workspace","tools":[{"type":"function","name":"workspace_command"}]}]}"#,
            ),
        };
        let login = keys.join(filename);
        std::fs::write(&login, &key).unwrap();
        std::fs::set_permissions(&login, std::fs::Permissions::from_mode(0o600)).unwrap();
        let login_handle = std::fs::File::open(&login).unwrap();
        // Exercise Podman's descriptor boundary even for a host descriptor
        // without close-on-exec. The test file contains only a generated canary.
        rustix::io::fcntl_setfd(&login_handle, rustix::io::FdFlags::empty()).unwrap();

        let verified = Arc::new(AtomicU32::new(0));
        let observed = verified.clone();
        let mock = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", mock.local_addr().unwrap());
        let upstream = tokio::spawn(async move {
            let router = Router::new().fallback(move |request: Request| {
                let observed = observed.clone();
                let expected = expected.clone();
                async move {
                    if request.uri().path() == path
                        && request
                            .headers()
                            .get(header_name)
                            .and_then(|value| value.to_str().ok())
                            == Some(expected.as_str())
                    {
                        observed.fetch_add(1, Ordering::Relaxed);
                        (StatusCode::OK, r#"{"text":"mock-provider-reply"}"#)
                    } else {
                        // A real upstream can echo what it received on denial.
                        (StatusCode::UNAUTHORIZED, "mock-credential-details")
                    }
                }
            });
            axum::serve(mock, router).await.unwrap();
        });
        let (closed, cancellation) = watch::channel(false);
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        let broker = Broker::bind(
            gateway_dir,
            "fixture-gateway".into(),
            closed,
            Arc::new(Gateway {
                access: "Bearer fixture-gateway".into(),
                credential: load_credential(dir.path(), space, provider).unwrap(),
                provider,
                client: client.clone(),
                tool_slot: Semaphore::new(1),
                tool: ToolSandbox {
                    podman: "podman".into(),
                    image: image.into(),
                    root_label: format!("org.calternal.test-broker={space}"),
                    work: work.clone(),
                    writable: true,
                },
                closed: cancellation,
                mock_base: Some(base.clone()),
            }),
        )
        .unwrap();

        let probe = format!(
            r#"
set -eu
/usr/local/bin/node - <<'JS'
const fs = require('node:fs'), http = require('node:http');
const login = {login:?};
const hostPid = {host_pid};
try {{ const fd=fs.openSync(login,'r');fs.closeSync(fd);process.exit(11); }} catch {{}}
if (fs.existsSync('/proc/'+hostPid)) process.exit(12);
for (const fd of fs.readdirSync('/proc/self/fd')) {{
  let result;
  try {{ result=fs.readlinkSync('/proc/self/fd/'+fd); }} catch {{ continue; }}
  if (result===login) process.exit(13);
}}
if (Object.keys(process.env).some(k=>/^(ANTHROPIC_|OPENAI_|CALTERNAL_GATEWAY|CLAUDE_CODE_OAUTH)/.test(k))) process.exit(14);
if (fs.existsSync('/run/calternal-broker/gateway.sock')) process.exit(15);
const denied = input => new Promise(resolve => {{
  const req=http.get(input, response=>{{response.resume();process.exit(16);}});
  req.setTimeout(1500,()=>req.destroy());
  req.on('error', resolve);
}});
Promise.all([denied('{base}'), denied({{socketPath:'/run/calternal-broker/gateway.sock',path:'/'}})])
  .then(()=>{{fs.writeFileSync('/work/isolated','yes');console.log('boundary intact');}});
JS
"#,
            login = login.to_string_lossy(),
            host_pid = std::process::id(),
        );
        let fixture = bridge_fixture(&broker);
        let tool = mcp_tool(&broker, &fixture, &probe).await;
        let reply = tool["result"]["content"][0]["text"].as_str().unwrap();
        assert_eq!(tool["result"]["isError"], false);
        assert!(
            reply.contains("Exit 0") && reply.contains("boundary intact"),
            "isolated tool did not complete: {reply:?}"
        );
        assert_eq!(
            std::fs::read_to_string(work.join("isolated")).unwrap(),
            "yes"
        );
        assert!(!reply.contains(&key));
        assert!(
            unix_request(&broker, path, "wrong-token", payload)
                .await
                .starts_with("HTTP/1.1 401")
        );
        assert!(
            unix_request(&broker, "/v1/unapproved", &broker.access, payload)
                .await
                .starts_with("HTTP/1.1 404")
        );

        let mock_url = format!("{base}{path}");
        let (mut bridge, bridge_url) = bridge_gateway(&broker, &fixture).await;
        let relayed_url = format!("{bridge_url}{path}");
        let mut overhead = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let direct = client
                .post(&mock_url)
                .header(
                    header_name,
                    if provider == SharedToolsProvider::Claude {
                        key.clone()
                    } else {
                        format!("Bearer {key}")
                    },
                )
                .body(payload)
                .send()
                .await
                .unwrap();
            assert!(direct.status().is_success());
            assert_eq!(
                direct.text().await.unwrap(),
                r#"{"text":"mock-provider-reply"}"#
            );
            let direct = start.elapsed();
            let start = Instant::now();
            let relayed = client
                .post(&relayed_url)
                .body(payload)
                .send()
                .await
                .unwrap();
            assert!(relayed.status().is_success());
            assert_eq!(
                relayed.text().await.unwrap(),
                r#"{"text":"mock-provider-reply"}"#
            );
            overhead.push(start.elapsed().as_micros() as i64 - direct.as_micros() as i64);
        }
        overhead.sort_unstable();
        println!(
            "{provider:?}: added local mock latency p50={} µs p95={} µs (100 pairs)",
            overhead[49], overhead[94]
        );
        assert_eq!(verified.load(Ordering::Relaxed), 200);
        bridge.kill().await.unwrap();
        drop(login_handle);
        broker.close();
        upstream.abort();
    }
}
