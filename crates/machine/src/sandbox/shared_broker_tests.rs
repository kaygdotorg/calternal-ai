//! The Claude and Codex fixtures share the machine-local broker and Podman boundary.
use super::*;
use std::{sync::atomic::{AtomicU32, Ordering}, time::Instant};
use tokio::{io::AsyncWriteExt, net::{TcpListener, UnixStream}};

#[test]
fn inference_only_accepts_the_fixed_tool() {
    assert!(uses_isolated_tools(br#"{"tools":[{"name":"mcp__workspace__workspace_command"}]}"#));
    assert!(uses_isolated_tools(br#"{"tools":[{"type":"function","name":"mcp__workspace__workspace_command"}]}"#));
    assert!(!uses_isolated_tools(br#"{"tools":[{"type":"function","name":"shell"}]}"#));
    assert!(!uses_isolated_tools(br#"{"tools":[{"type":"web_search"}]}"#));
    assert!(!uses_isolated_tools(br#"{"tools":"invalid"}"#));
}

async fn unix_request(broker: &Broker, path: &str, token: &str, payload: &str) -> String {
    let mut stream = UnixStream::connect(broker.directory.join("gateway.sock")).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut body = String::new();
    stream.read_to_string(&mut body).await.unwrap();
    body
}

#[tokio::test]
async fn shared_broker_keeps_claude_and_codex_credentials_out_of_tools() {
    let Some(image) = std::env::var_os("CALTERNAL_TEST_SHARED_BROKER_IMAGE") else {
        eprintln!("Set CALTERNAL_TEST_SHARED_BROKER_IMAGE to a local Podman Node image to exercise the isolation boundary.");
        return;
    };
    for provider in [SharedToolsProvider::Claude, SharedToolsProvider::Codex] {
        let dir = tempfile::tempdir().unwrap();
        let space = SpaceId::new();
        let keys = dir.path().join("broker-credentials").join(space.to_string());
        let gateway_dir = dir.path().join("socket");
        let work = dir.path().join("work");
        for path in [&keys, &gateway_dir, &work] {
            calternal_fsio::private::directory(path).unwrap();
        }
        let key = format!("synthetic-{}", Uuid::new_v4());
        let (filename, path, header_name, expected, payload) = match provider {
            SharedToolsProvider::Claude => ("anthropic.key", "/v1/messages", "x-api-key", key.clone(),
                r#"{"messages":[],"tools":[{"name":"mcp__workspace__workspace_command"}]}"#),
            SharedToolsProvider::Codex => ("openai.key", "/v1/responses", "authorization", format!("Bearer {key}"),
                r#"{"input":[],"tools":[{"type":"function","name":"mcp__workspace__workspace_command"}]}"#),
        };
        let login = keys.join(filename);
        std::fs::write(&login, &key).unwrap();
        std::fs::set_permissions(&login, std::fs::Permissions::from_mode(0o600)).unwrap();
        let login_handle = std::fs::File::open(&login).unwrap();

        let verified = Arc::new(AtomicU32::new(0));
        let observed = verified.clone();
        let mock = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", mock.local_addr().unwrap());
        let upstream = tokio::spawn(async move {
            let router = Router::new().fallback(move |request: Request| {
                let observed = observed.clone();
                let expected = expected.clone();
                async move {
                    if request.uri().path() == path && request.headers().get(header_name)
                        .and_then(|value| value.to_str().ok()) == Some(expected.as_str())
                    {
                        observed.fetch_add(1, Ordering::Relaxed);
                        (StatusCode::OK, "mock-provider-reply")
                    } else {
                        // A real upstream can echo what it received on denial.
                        (StatusCode::UNAUTHORIZED, "mock-credential-details")
                    }
                }
            });
            axum::serve(mock, router).await.unwrap();
        });
        let (closed, cancellation) = watch::channel(false);
        let client = reqwest::Client::builder().no_proxy()
            .redirect(reqwest::redirect::Policy::none()).build().unwrap();
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
                    podman: "podman".into(), image: image.to_string_lossy().into_owned(),
                    root_label: format!("org.calternal.test-broker={space}"),
                    work: work.clone(), writable: true,
                },
                closed: cancellation,
                mock_base: Some(base.clone()),
            }),
        ).unwrap();

        let probe = format!(r#"
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
            login = login.to_string_lossy(), host_pid = std::process::id(),
        );
        let reply = unix_request(&broker, "/tool", &broker.access,
            &serde_json::json!({"command": probe}).to_string()).await;
        assert!(reply.contains("Exit 0") && reply.contains("boundary intact"), "isolated tool did not complete");
        assert_eq!(std::fs::read_to_string(work.join("isolated")).unwrap(), "yes");
        assert!(!reply.contains(&key));
        assert!(unix_request(&broker, path, "wrong-token", payload).await.starts_with("HTTP/1.1 401"));
        assert!(unix_request(&broker, "/v1/unapproved", &broker.access, payload).await.starts_with("HTTP/1.1 404"));

        let mock_url = format!("{base}{path}");
        let mut overhead = Vec::new();
        for _ in 0..100 {
            let start = Instant::now();
            let direct = client.post(&mock_url).header(header_name, if provider == SharedToolsProvider::Claude { key.clone() } else { format!("Bearer {key}") })
                .header(header::CONNECTION, "close").body(payload).send().await.unwrap();
            assert!(direct.status().is_success());
            assert_eq!(direct.text().await.unwrap(), "mock-provider-reply");
            let direct = start.elapsed();
            let start = Instant::now();
            let relayed = unix_request(&broker, path, &broker.access, payload).await;
            assert!(relayed.starts_with("HTTP/1.1 200") && relayed.contains("mock-provider-reply"));
            assert!(!relayed.contains(&key));
            overhead.push(start.elapsed().as_micros() as i64 - direct.as_micros() as i64);
        }
        overhead.sort_unstable();
        println!("{provider:?}: added local mock latency p50={} µs p95={} µs (100 pairs)", overhead[49], overhead[94]);
        assert_eq!(verified.load(Ordering::Relaxed), 200);
        drop(login_handle);
        broker.close();
        upstream.abort();
    }
}
