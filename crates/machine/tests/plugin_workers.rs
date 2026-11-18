//! OS-boundary probes for the worker profile; direct mode intentionally has none.
#[path = "../../../tests/support/podman_guard.rs"]
mod podman_guard;
use podman_guard::PodmanGuard;

use calternal_machine::{
    protocol::{PluginWorker, SandboxSpec, Stream},
    sandbox::{Backend, ExecRequest, OutputEvent, Sandbox, podman::PodmanBackend, worker},
};
use calternal_types::SpaceId;
use std::sync::Arc;
use uuid::Uuid;

// The pressure probe fills its CPU and process quotas. Let it finish and tear
// down before another worker probe starts under the same Podman account.
static CONTAINER_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

async fn backend(root: &std::path::Path) -> Option<PodmanBackend> {
    if !PodmanBackend::available(std::path::Path::new("podman")).await {
        eprintln!("SKIP plugin worker container isolation: Podman is unavailable");
        return None;
    }
    let present = tokio::process::Command::new("podman")
        .args(["image", "exists", worker::IMAGE])
        .status()
        .await
        .unwrap();
    assert!(
        present.success(),
        "provision worker image first: podman pull {}",
        worker::IMAGE
    );
    Some(PodmanBackend::new("podman", root, worker::IMAGE))
}

fn spec(source: &str) -> SandboxSpec {
    SandboxSpec {
        resource_owner: None,
        guest_idle_seconds: None,
        resource_assignment: None,
        space_id: Some(calternal_types::SpaceId::new()),
        provider_login: false,
        code_directory: None,
        account_id: Uuid::new_v4(),
        thread_id: Uuid::new_v4(),
        image: Some(worker::IMAGE.into()),
        limits: worker::LIMITS,
        worker: Some(PluginWorker {
            space_id: SpaceId::new(),
            source: source.into(),
        }),
    }
}

async fn run(sb: &Arc<dyn Sandbox>, script: &str) -> (String, Option<i32>) {
    let mut exec = sb
        .exec(ExecRequest {
            argv: vec!["node".into(), "-e".into(), script.into()],
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    let mut output = Vec::new();
    let mut code = None;
    let completion = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        while let Some(event) = exec.events.recv().await {
            match event {
                OutputEvent::Data(Stream::Stdout, bytes) => {
                    output.extend(bytes);
                    assert!(output.len() < 65536);
                }
                OutputEvent::Exit(info) => code = info.code,
                _ => {}
            }
        }
    })
    .await;
    if completion.is_err() {
        let _ = sb.stop().await;
        panic!("sandbox probe timed out: {script}");
    }
    (String::from_utf8(output).unwrap(), code)
}

#[tokio::test]
async fn container_denies_sockets_host_files_foreign_space_and_code_writes() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let secret = root.path().join("appdata-key");
    tokio::fs::write(&secret, "synthetic-appdata")
        .await
        .unwrap();
    let other = spec("foreign-space-code");
    let foreign = backend.start(&other).await.unwrap();
    let sb = backend.start(&spec("space-one-code")).await.unwrap();
    let foreign_path = root
        .path()
        .join("workers")
        .join(other.thread_id.to_string())
        .join("work/worker.js");
    let script = format!(
        r#"
        const fs = require('node:fs');
        for (const path of [{}, {}]) {{
            try {{ fs.readFileSync(path); process.exit(41); }} catch (e) {{ if(e.code !== 'ENOENT') throw e; }}
        }}
        if(fs.readFileSync('/plugin/worker.js','utf8') !== 'space-one-code') process.exit(42);
        try {{ fs.writeFileSync('/plugin/worker.js','changed'); process.exit(43); }} catch(e) {{ if(e.code !== 'EROFS') throw e; }}
        const server = require('node:net').createServer();
        server.on('error', e => {{ if(e.code !== 'EPERM' && e.code !== 'EACCES') throw e; console.log('isolated'); }});
        server.listen(0, '127.0.0.1', () => process.exit(44));
    "#,
        serde_json::to_string(&secret.to_string_lossy()).unwrap(),
        serde_json::to_string(&foreign_path.to_string_lossy()).unwrap()
    );
    let result = run(&sb, &script).await;
    sb.stop().await.unwrap();
    foreign.stop().await.unwrap();
    assert_eq!(result, ("isolated\n".into(), Some(0)));
}

#[tokio::test]
async fn container_enforces_memory_cpu_pids_and_streams_ordered_stdin() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let limits = spec("limits");
    let sb = backend.start(&limits).await.unwrap();
    let (output, code) = run(&sb, r#"
        const fs = require('node:fs');
        const values = ['memory.max','cpu.max','pids.max'].map(n=>fs.readFileSync('/sys/fs/cgroup/'+n,'utf8').trim());
        console.log(JSON.stringify(values));
    "#).await;
    assert_eq!(code, Some(0));
    let values: Vec<String> = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(values[0], (256 * 1024 * 1024).to_string());
    assert_eq!(values[1], "100000 100000");
    assert_eq!(values[2], "64");
    let mut exec = sb
        .exec(ExecRequest {
            argv: vec![
                "node".into(),
                "-e".into(),
                "process.stdin.pipe(process.stdout)".into(),
            ],
            interactive: true,
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    let input = exec.input.take().unwrap();
    input.send(b"one\n".to_vec()).await.unwrap();
    input.send(b"two\n".to_vec()).await.unwrap();
    drop(input);
    let mut output = vec![];
    while let Some(event) = exec.events.recv().await {
        if let OutputEvent::Data(Stream::Stdout, bytes) = event {
            output.extend(bytes);
        }
    }
    assert_eq!(output, b"one\ntwo\n");
    if !cfg!(target_os = "linux") {
        eprintln!(
            "SKIP pressure counters: host Linux cgroup access is unavailable; configured limits and stdin were checked"
        );
        sb.stop().await.unwrap();
        return;
    }
    let group = cgroup(&limits).await;
    let before = counter(&group.join("pids.events"), "max").await;
    // Hold every child in this cgroup. A small shell issues one fork at a time
    // so the workload does not depend on Node's startup threads or spawn errors.
    // The shell may exit when fork is denied; the kernel increments max first.
    let forks = sb
        .exec(ExecRequest {
            argv: vec![
                "/bin/sh".into(),
                "-c".into(),
                "printf 'forking\\n'; i=0; while [ \"$i\" -lt 64 ]; do sleep 60 & i=$((i+1)); done; wait".into(),
            ],
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    wait_counter(forks, &group.join("pids.events"), "max", before).await;
    sb.stop().await.unwrap();
    let memory = spec("memory pressure");
    let sb = backend.start(&memory).await.unwrap();
    let group = cgroup(&memory).await;
    let before = counter(&group.join("memory.events"), "oom_kill").await;
    let allocation = sb
        .exec(ExecRequest {
            argv: vec![
                "node".into(),
                "-e".into(),
                "let blocks=[];for(let i=0;i<1024;i++)blocks.push(Buffer.alloc(1024*1024,1));"
                    .into(),
            ],
            ..ExecRequest::default()
        })
        .await
        .unwrap();
    wait_counter(allocation, &group.join("memory.events"), "oom_kill", before).await;
    sb.stop().await.unwrap();
}

// Observe the kernel's enforcement, independently of Podman exec's exit delivery.
async fn cgroup(spec: &SandboxSpec) -> std::path::PathBuf {
    let output = tokio::process::Command::new("podman")
        .args([
            "inspect",
            "--format",
            "{{.State.Pid}}",
            &PodmanBackend::container_name(spec.account_id, spec.thread_id),
        ])
        .output()
        .await
        .unwrap();
    assert!(output.status.success());
    let pid = String::from_utf8(output.stdout).unwrap();
    let text = tokio::fs::read_to_string(format!("/proc/{}/cgroup", pid.trim()))
        .await
        .unwrap();
    let path = text
        .lines()
        .find_map(|line| line.strip_prefix("0::"))
        .unwrap();
    std::path::Path::new("/sys/fs/cgroup").join(path.trim_start_matches('/'))
}
async fn counter(path: &std::path::Path, field: &str) -> u64 {
    tokio::fs::read_to_string(path)
        .await
        .unwrap()
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(' ')?;
            (name == field).then(|| value.parse().unwrap())
        })
        .unwrap()
}
async fn wait_counter(
    mut probe: calternal_machine::sandbox::Exec,
    path: &std::path::Path,
    field: &str,
    before: u64,
) {
    let mut poll = tokio::time::interval(std::time::Duration::from_millis(20));
    let mut output = String::new();
    let mut exit = None;
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            tokio::select! {
                event = probe.events.recv() => match event {
                    Some(OutputEvent::Exit(info)) => { exit = Some(info); break; }
                    Some(OutputEvent::Data(_, bytes)) if output.len() < 128 => {
                        output.push_str(&String::from_utf8_lossy(&bytes[..bytes.len().min(128)]));
                    }
                    Some(_) => {}
                    None => break,
                },
                _ = poll.tick() => {
                    if counter(path, field).await > before { return true; }
                }
            }
        }
        // Both cgroup counters are cumulative. The kernel records the event
        // before the fork refusal or OOM-killed process is observable by Podman.
        counter(path, field).await > before
    })
    .await;
    assert!(
        matches!(result, Ok(true)),
        "kernel did not report {field}: initial={before}, probe_exit={exit:?}, probe_output={output:?}, timed_out={}",
        result.is_err(),
    );
}

#[tokio::test]
async fn development_worker_measurements() {
    if std::env::var_os("CALTERNAL_MEASURE_WORKERS").is_none() {
        eprintln!(
            "SKIP development measurements: set CALTERNAL_MEASURE_WORKERS=1 on the assigned dev VM"
        );
        return;
    }
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let Some(podman) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let direct = calternal_machine::sandbox::direct::DirectBackend::new(root.path());
    let source = "require('node:readline').createInterface({input:process.stdin}).on('line',line=>{const f=JSON.parse(line);console.log(JSON.stringify({protocol_version:2,invocation_id:f.invocation_id,message:{type:'complete',data:{pid:process.pid,node:process.version}}}));});";
    let mut rows = Vec::new();
    for backend in [&direct as &dyn Backend, &podman as &dyn Backend] {
        let mut cold = Vec::new();
        let mut calls = Vec::new();
        let mut memory = String::new();
        let mut node_version = String::new();
        for round in 0..5 {
            let spec = spec(source);
            let started = std::time::Instant::now();
            let sb = backend.start(&spec).await.unwrap();
            let mut exec = sb
                .exec(ExecRequest {
                    argv: vec![
                        if backend.name() == "podman" {
                            "node".into()
                        } else {
                            std::env::split_paths(&std::env::var_os("PATH").unwrap())
                                .map(|p| p.join("node"))
                                .find(|p| p.is_file())
                                .unwrap()
                                .to_string_lossy()
                                .into_owned()
                        },
                        "--permission".into(),
                        "--allow-fs-read=.".into(),
                        "--disallow-code-generation-from-strings".into(),
                        "--max-old-space-size=128".into(),
                        "worker.js".into(),
                    ],
                    interactive: true,
                    ..ExecRequest::default()
                })
                .await
                .unwrap();
            let response = exchange(&mut exec, 0).await;
            cold.push(started.elapsed().as_secs_f64() * 1000.0);
            for id in 1..=100 {
                let started = std::time::Instant::now();
                exchange(&mut exec, id).await;
                calls.push(started.elapsed().as_secs_f64() * 1000.0);
            }
            if round == 0 {
                node_version = response["message"]["data"]["node"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                memory = if backend.name() == "podman" {
                    let out = tokio::process::Command::new("podman")
                        .args([
                            "stats",
                            "--no-stream",
                            "--format",
                            "{{.MemUsage}}",
                            &PodmanBackend::container_name(spec.account_id, spec.thread_id),
                        ])
                        .output()
                        .await
                        .unwrap();
                    String::from_utf8(out.stdout).unwrap().trim().to_owned()
                } else {
                    let text = tokio::fs::read_to_string(format!(
                        "/proc/{}/status",
                        response["message"]["data"]["pid"]
                    ))
                    .await
                    .unwrap();
                    text.lines()
                        .find(|line| line.starts_with("VmRSS:"))
                        .unwrap()
                        .to_owned()
                };
            }
            sb.stop().await.unwrap();
        }
        cold.sort_by(f64::total_cmp);
        calls.sort_by(f64::total_cmp);
        rows.push(serde_json::json!({"backend":backend.name(),"node":node_version,"cold_ms_n5":cold,"operation_ms_p50":calls[249],"operation_ms_p95":calls[474],"operations":500,"idle_memory":memory}));
    }
    eprintln!("WORKER_MEASUREMENTS {}", serde_json::json!(rows));
}

async fn exchange(exec: &mut calternal_machine::sandbox::Exec, id: usize) -> serde_json::Value {
    let frame = serde_json::json!({"protocol_version":2,"invocation_id":format!("measure{id}"),"message":{"type":"invoke","operation":"measure.echo","input":{},"authority":{"account_id":calternal_types::AccountId::new(),"space_id":SpaceId::new(),"grant":"measurement"}}});
    let mut bytes = serde_json::to_vec(&frame).unwrap();
    bytes.push(b'\n');
    exec.input.as_ref().unwrap().send(bytes).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut output = vec![];
        while let Some(event) = exec.events.recv().await {
            match event {
                OutputEvent::Data(Stream::Stdout, bytes) => {
                    output.extend(bytes);
                    if output.last() == Some(&b'\n') {
                        return serde_json::from_slice(&output).unwrap();
                    }
                }
                OutputEvent::Exit(info) => panic!("worker measurement exited: {info:?}"),
                _ => {}
            }
        }
        panic!("worker output ended");
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn machine_restart_removes_only_its_own_ephemeral_workers() {
    let _serial = CONTAINER_TEST.lock().await;
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let Some(backend) = backend(root.path()).await else {
        return;
    };
    let _containers = PodmanGuard::new(root.path());
    let _other = PodmanGuard::new(other.path());
    let other_backend = PodmanBackend::new("podman", other.path(), worker::IMAGE);
    let first = spec("first machine");
    let second = spec("second machine");
    let _first = backend.start(&first).await.unwrap();
    let second_sb = other_backend.start(&second).await.unwrap();
    backend.initialize().await.unwrap();
    let exists = |spec: &SandboxSpec| {
        let name = PodmanBackend::container_name(spec.account_id, spec.thread_id);
        async move {
            tokio::process::Command::new("podman")
                .args(["container", "exists", &name])
                .status()
                .await
                .unwrap()
                .success()
        }
    };
    assert!(!exists(&first).await);
    assert!(
        exists(&second).await,
        "cleanup crossed the enrolled machine directory"
    );
    assert!(!root.path().join("workers").exists());
    second_sb.stop().await.unwrap();
}
