//! Trusted plugin registry and Node worker execution.
//!
//! Only plugins listed in the compiled-in `trusted-plugins.json` can load. The
//! browser can never supply code, paths or URLs. Manifests are read once at
//! startup and kept immutable in memory. Worker bundle SHA-256 digests are
//! pinned in the registry and checked again before every spawn.
//!
//! The worker child process is NOT an OS security sandbox. It is a bounded
//! process for repository-shipped trusted code: fresh process per call,
//! cleared environment, Node permission model limiting fs reads to the
//! bundle directory (defense in depth only), stdin/stdout/stderr caps and a
//! timeout with kill + reap.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use calternal_core::{
    MAX_WORKER_OUTPUT_BYTES, PROTOCOL_VERSION, PluginManifest, SummarizeResult, WorkerErrorCode,
    WorkerOperation, WorkerRequest, WorkerResource, WorkerResponse, parse_worker_response,
    validate_extension_view, validate_manifest, validate_worker_request,
};
use serde::Deserialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

use crate::error::HostError;
use crate::store::sha256_hex;

pub const WORKER_STDIN_MAX: usize = 2 * 1024 * 1024;
pub const WORKER_STDERR_MAX: usize = 16 * 1024;
pub const WORKER_TIMEOUT: Duration = Duration::from_secs(3);
const MIN_NODE: (u32, u32) = (22, 13);

const REGISTRY_JSON: &str = include_str!("../trusted-plugins.json");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryFile {
    plugins: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryEntry {
    pub id: String,
    pub version: String,
    /// Relative to --plugin-root.
    pub manifest: String,
    /// Relative to --plugin-root; None for view-only plugins.
    pub worker: Option<String>,
    pub worker_sha256: Option<String>,
    pub default_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub entry: RegistryEntry,
    pub manifest: PluginManifest,
    /// Absolute worker path + directory, when the plugin has a worker.
    pub worker: Option<(PathBuf, PathBuf)>,
    /// Why the plugin cannot execute, if it cannot (digest mismatch etc).
    pub unavailable: Option<String>,
}

pub struct Registry {
    pub plugins: Vec<LoadedPlugin>,
}

fn rel_path_ok(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && Path::new(p)
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

pub fn registry_entries() -> Vec<RegistryEntry> {
    let file: RegistryFile =
        serde_json::from_str(REGISTRY_JSON).expect("compiled-in trusted-plugins.json is valid");
    file.plugins
}

impl Registry {
    /// Load manifests for all trusted entries. Plugins whose manifest is
    /// missing or invalid are skipped with a warning; plugins whose worker is
    /// missing or whose digest differs load but are marked unavailable.
    pub fn load(plugin_root: &Path) -> Self {
        let entries = registry_entries();
        let mut plugins: Vec<LoadedPlugin> = Vec::new();
        for entry in &entries {
            match load_one(plugin_root, entry) {
                Ok(p) => {
                    if let Some(why) = &p.unavailable {
                        tracing::warn!(plugin = %entry.id, reason = %why, "trusted plugin unavailable");
                    } else {
                        tracing::info!(plugin = %entry.id, version = %entry.version, "trusted plugin registered");
                    }
                    plugins.push(p)
                }
                Err(why) => {
                    tracing::warn!(plugin = %entry.id, reason = %why, "trusted plugin not loaded")
                }
            }
        }
        // Exact dependency version validation against loaded plugins.
        let snapshot: Vec<(String, String)> = plugins
            .iter()
            .map(|p| (p.manifest.id.clone(), p.manifest.version.clone()))
            .collect();
        for p in &mut plugins {
            for dep in &p.manifest.dependencies {
                if !snapshot
                    .iter()
                    .any(|(id, v)| *id == dep.id && *v == dep.version)
                {
                    p.unavailable.get_or_insert_with(|| {
                        format!("dependency {}@{} is not registered", dep.id, dep.version)
                    });
                }
            }
        }
        Registry { plugins }
    }

    pub fn get(&self, id: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|p| p.manifest.id == id)
    }

    /// Which loaded plugin declares an operation id.
    pub fn owner_of_operation(&self, op: &str) -> Option<&LoadedPlugin> {
        self.plugins
            .iter()
            .find(|p| p.manifest.operations.iter().any(|o| o.id == op))
    }
}

fn load_one(root: &Path, entry: &RegistryEntry) -> Result<LoadedPlugin, String> {
    if !rel_path_ok(&entry.manifest) {
        return Err("registry manifest path invalid".into());
    }
    let mpath = root.join(&entry.manifest);
    let meta = std::fs::symlink_metadata(&mpath).map_err(|e| format!("manifest: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("manifest is not a regular file".into());
    }
    if meta.len() > 256 * 1024 {
        return Err("manifest too large".into());
    }
    let bytes = std::fs::read(&mpath).map_err(|e| format!("manifest: {e}"))?;
    let manifest: PluginManifest = serde_json::from_slice(&bytes)
        .map_err(|e| format!("manifest does not match frozen shape: {e}"))?;
    validate_manifest(&manifest).map_err(|e| format!("manifest invalid: {e}"))?;
    if manifest.id != entry.id || manifest.version != entry.version {
        return Err(format!(
            "manifest id/version {}@{} does not match registry {}@{}",
            manifest.id, manifest.version, entry.id, entry.version
        ));
    }
    let mut unavailable = None;
    let worker = match &entry.worker {
        None => None,
        Some(w) => {
            if !rel_path_ok(w) {
                return Err("registry worker path invalid".into());
            }
            let wpath = root.join(w);
            match std::fs::canonicalize(&wpath) {
                Ok(abs) => {
                    let dir = abs.parent().map(Path::to_path_buf).unwrap_or_default();
                    if let Err(why) = verify_digest(&abs, entry.worker_sha256.as_deref()) {
                        unavailable = Some(why);
                    }
                    Some((abs, dir))
                }
                Err(_) => {
                    unavailable = Some("worker bundle not built".into());
                    None
                }
            }
        }
    };
    Ok(LoadedPlugin {
        entry: entry.clone(),
        manifest,
        worker,
        unavailable,
    })
}

pub fn file_sha256(path: &Path) -> Result<String, String> {
    let meta = std::fs::symlink_metadata(path).map_err(|e| format!("worker: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("worker bundle is not a regular file".into());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("worker: {e}"))?;
    Ok(sha256_hex(&bytes))
}

fn verify_digest(path: &Path, expected: Option<&str>) -> Result<(), String> {
    let actual = file_sha256(path)?;
    match expected {
        Some(e) if e.eq_ignore_ascii_case(&actual) => Ok(()),
        Some(_) => Err("worker bundle digest does not match trusted registry".into()),
        None => Err("worker bundle digest not pinned in trusted registry".into()),
    }
}

// ------------------------------------------------------------------ node

pub const MAX_CONCURRENT_WORKERS: usize = 4;

#[derive(Debug, Clone)]
pub struct NodeRuntime {
    pub path: PathBuf,
    pub version: String,
    /// Global admission cap: callers never queue; saturation is `unavailable`.
    pub permits: std::sync::Arc<tokio::sync::Semaphore>,
}

/// Resolve and verify an absolute Node executable (>= 22.13 for --permission).
pub async fn resolve_node(explicit: Option<&Path>) -> Result<NodeRuntime, String> {
    let candidate = match explicit {
        Some(p) => p.to_path_buf(),
        None => {
            let path_var = std::env::var_os("PATH").ok_or("PATH not set; pass --node")?;
            std::env::split_paths(&path_var)
                .map(|d| d.join("node"))
                .find(|p| p.is_file())
                .ok_or("node not found on PATH; pass --node")?
        }
    };
    let abs = std::fs::canonicalize(&candidate).map_err(|e| format!("node: {e}"))?;
    let out = tokio::process::Command::new(&abs)
        .arg("--version")
        .env_clear()
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| format!("node --version: {e}"))?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let nums: Vec<u32> = v
        .trim_start_matches('v')
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    if nums.len() < 2 || (nums[0], nums[1]) < MIN_NODE {
        return Err(format!(
            "node {v} is too old; need >= {}.{}",
            MIN_NODE.0, MIN_NODE.1
        ));
    }
    Ok(NodeRuntime {
        path: abs,
        version: v,
        permits: std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_WORKERS)),
    })
}

// ---------------------------------------------------------------- worker

pub async fn run_summarize(
    node: &NodeRuntime,
    plugin: &LoadedPlugin,
    resource: WorkerResource,
) -> Result<SummarizeResult, HostError> {
    if let Some(why) = &plugin.unavailable {
        return Err(HostError::unavailable(format!("plugin unavailable: {why}")));
    }
    let Some((worker_path, worker_dir)) = &plugin.worker else {
        return Err(HostError::unavailable("plugin has no worker"));
    };
    // Nonblocking admission; the permit is released on every exit path.
    let _permit = node.permits.clone().try_acquire_owned().map_err(|_| {
        HostError::unavailable("extension workers are busy; retry shortly")
            .with_details(serde_json::json!({ "reason": "worker_capacity" }))
    })?;
    // Re-verify the pinned digest immediately before spawning.
    verify_digest(worker_path, plugin.entry.worker_sha256.as_deref()).map_err(|why| {
        tracing::warn!(plugin = %plugin.manifest.id, reason = %why, "refusing to spawn worker");
        HostError::unavailable("plugin bundle failed verification")
    })?;

    let mut idb = [0u8; 12];
    getrandom::fill(&mut idb).map_err(|_| HostError::internal("randomness unavailable"))?;
    let invocation_id = format!("inv-{}", hex::encode(idb));
    let request = WorkerRequest {
        protocol_version: PROTOCOL_VERSION,
        invocation_id: invocation_id.clone(),
        operation: WorkerOperation::Summarize,
        resource,
    };
    validate_worker_request(&request)?;
    let stdin_bytes = serde_json::to_vec(&request).map_err(|_| HostError::internal("encode"))?;
    if stdin_bytes.len() > WORKER_STDIN_MAX {
        return Err(HostError::new(
            calternal_core::ErrorCode::TooLarge,
            "worker input exceeds 2 MiB",
        ));
    }

    let output = spawn_bounded(node, worker_path, worker_dir, &stdin_bytes).await?;
    let response = parse_worker_response(&output, &invocation_id).map_err(|e| {
        tracing::warn!(plugin = %plugin.manifest.id, error = %e, "worker returned invalid output");
        HostError::unavailable("extension returned invalid output")
    })?;
    match response {
        WorkerResponse::Success(s) => {
            validate_extension_view(&s.data.view, &plugin.manifest).map_err(|e| {
                tracing::warn!(plugin = %plugin.manifest.id, error = %e, "worker view rejected");
                HostError::unavailable("extension returned an invalid view")
            })?;
            Ok(s.data)
        }
        WorkerResponse::Failure(f) => Err(match f.error.code {
            WorkerErrorCode::InvalidInput => {
                HostError::invalid_input("extension rejected the input")
            }
            WorkerErrorCode::Internal => HostError::unavailable("extension failed"),
        }),
    }
}

#[derive(Debug)]
enum PipeError {
    Overflow,
    Io,
}

/// Read to EOF, failing as soon as `cap` is exceeded.
async fn read_capped<R: AsyncRead + Unpin>(mut r: R, cap: usize) -> Result<Vec<u8>, PipeError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let n = r.read(&mut chunk).await.map_err(|_| PipeError::Io)?;
        if n == 0 {
            return Ok(buf);
        }
        if buf.len() + n > cap {
            return Err(PipeError::Overflow);
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

async fn spawn_bounded(
    node: &NodeRuntime,
    worker: &Path,
    dir: &Path,
    input: &[u8],
) -> Result<Vec<u8>, HostError> {
    let mut cmd = tokio::process::Command::new(&node.path);
    cmd.arg("--permission")
        .arg(format!("--allow-fs-read={}", dir.display()))
        .arg("--disallow-code-generation-from-strings")
        .arg(worker)
        .env_clear()
        .env("LANG", "C.UTF-8")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|e| {
        tracing::error!(error = %e, "failed to spawn worker");
        HostError::unavailable("extension worker could not start")
    })?;
    let mut stdin = child.stdin.take().expect("piped");
    let stdout = child.stdout.take().expect("piped");
    let stderr = child.stderr.take().expect("piped");

    let io = async {
        let write = async {
            // EPIPE (worker exited early) is reported via exit status/output.
            let _ = stdin.write_all(input).await;
            let _ = stdin.shutdown().await;
            drop(stdin);
            Ok::<(), PipeError>(())
        };
        // try_join: the first overflow/IO error returns immediately so the
        // child is killed without waiting for the other pipe or the timeout.
        tokio::try_join!(
            write,
            read_capped(stdout, MAX_WORKER_OUTPUT_BYTES),
            read_capped(stderr, WORKER_STDERR_MAX)
        )
    };
    let (out, err) = match tokio::time::timeout(WORKER_TIMEOUT, io).await {
        Ok(Ok(((), out, err))) => (out, err),
        Ok(Err(e)) => {
            let _ = child.kill().await; // kill + reap
            tracing::warn!(error = ?e, "worker output exceeded cap or failed; killed");
            return Err(HostError::unavailable("extension output exceeded limit"));
        }
        Err(_) => {
            let _ = child.kill().await; // kill + reap
            tracing::warn!("worker timed out; killed");
            return Err(HostError::unavailable("extension timed out"));
        }
    };
    let status = match tokio::time::timeout(Duration::from_millis(500), child.wait()).await {
        Ok(Ok(s)) => s,
        _ => {
            let _ = child.kill().await;
            return Err(HostError::unavailable("extension did not exit"));
        }
    };
    if !err.is_empty() {
        // Never log stderr content: it may echo private note text.
        tracing::debug!(stderr_bytes = err.len(), "worker wrote diagnostics");
    }
    if !status.success() {
        tracing::warn!(status = ?status.code(), "worker exited nonzero");
        return Err(HostError::unavailable("extension failed"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_core::{False, PluginOperation};

    struct Tmp(PathBuf);
    impl Drop for Tmp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn manifest() -> PluginManifest {
        PluginManifest {
            id: "notes-insights".into(),
            version: "0.1.0".into(),
            api_version: 1,
            name: "Insights".into(),
            description: "test".into(),
            dependencies: vec![],
            capabilities: vec!["notes.read".into()],
            operations: vec![PluginOperation {
                id: "notes-insights.summarize".into(),
                description: "stats".into(),
                mutates: False,
            }],
            views: vec![],
        }
    }

    /// Worker script body; `$OUT` receives the request object as `req`.
    fn plugin(tag: &str, script: &str) -> (Tmp, LoadedPlugin) {
        let mut b = [0u8; 6];
        getrandom::fill(&mut b).unwrap();
        let dir = std::env::temp_dir().join(format!("calternal-worker-{tag}-{}", hex::encode(b)));
        std::fs::create_dir_all(&dir).unwrap();
        let w = dir.join("worker.js");
        let src = format!(
            "let s='';process.stdin.setEncoding('utf8');process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{{const req=JSON.parse(s);{script}}});"
        );
        std::fs::write(&w, src).unwrap();
        let w = std::fs::canonicalize(&w).unwrap();
        let digest = file_sha256(&w).unwrap();
        let lp = LoadedPlugin {
            entry: RegistryEntry {
                id: "notes-insights".into(),
                version: "0.1.0".into(),
                manifest: "x".into(),
                worker: Some("x".into()),
                worker_sha256: Some(digest),
                default_enabled: false,
            },
            manifest: manifest(),
            worker: Some((w.clone(), w.parent().unwrap().to_path_buf())),
            unavailable: None,
        };
        (Tmp(dir), lp)
    }

    fn resource() -> WorkerResource {
        WorkerResource {
            id: "11111111-1111-4111-8111-111111111111".into(),
            space_id: "personal".into(),
            revision: "v1.x".into(),
            title: "T".into(),
            markdown: "hello world\nsecond line\n".into(),
        }
    }

    const OK_REPLY: &str = r#"const view={genui_version:1,root:{type:'stack',id:'root',children:[{type:'stat',id:'w',label:'Words',value:String(req.resource.markdown.split(/\s+/).filter(Boolean).length)},{type:'text',id:'t',text:TEXT}]}};process.stdout.write(JSON.stringify({protocol_version:1,invocation_id:req.invocation_id,ok:true,data:{stats:{words:4,characters:24,lines:2},view}}));"#;

    fn ok_with_text(text_expr: &str) -> String {
        format!("const TEXT={text_expr};{OK_REPLY}")
    }

    async fn node() -> NodeRuntime {
        resolve_node(None)
            .await
            .expect("node >= 22.13 on PATH for tests")
    }

    #[tokio::test]
    async fn success_with_cleared_env_and_fs_confinement() {
        let script = ok_with_text(
            "JSON.stringify(Object.keys(process.env).sort())+'|'+(()=>{try{require('fs').readFileSync('/etc/hostname');return 'read'}catch(e){return e.code}})()",
        );
        let (_t, lp) = plugin("ok", &script);
        let r = run_summarize(&node().await, &lp, resource())
            .await
            .expect("success");
        assert_eq!(r.stats.words, 4);
        let calternal_core::GenUiNode::Stack { children, .. } = &r.view.root else {
            panic!()
        };
        let calternal_core::GenUiNode::Text { text, .. } = &children[1] else {
            panic!()
        };
        assert_eq!(text, r#"["LANG"]|ERR_ACCESS_DENIED"#);
    }

    async fn expect_err(tag: &str, script: &str, code: calternal_core::ErrorCode) -> HostError {
        let (_t, lp) = plugin(tag, script);
        let e = run_summarize(&node().await, &lp, resource())
            .await
            .expect_err("should fail");
        assert_eq!(e.code, code, "{tag}: {}", e.message);
        e
    }

    #[tokio::test]
    async fn malformed_and_mismatched_outputs_rejected() {
        use calternal_core::ErrorCode::*;
        expect_err("garbage", "process.stdout.write('not json')", Unavailable).await;
        expect_err(
            "trailing",
            &format!("const TEXT='x';{OK_REPLY}process.stdout.write('{{}}');"),
            Unavailable,
        )
        .await;
        expect_err(
            "idmismatch",
            &ok_with_text("'x'").replace(
                "invocation_id:req.invocation_id",
                "invocation_id:'inv-other'",
            ),
            Unavailable,
        )
        .await;
        expect_err(
            "version",
            &ok_with_text("'x'").replace("protocol_version:1", "protocol_version:2"),
            Unavailable,
        )
        .await;
        // Hostile GenUI: duplicate ids.
        expect_err(
            "dupid",
            &ok_with_text("'x'").replace("id:'t'", "id:'w'"),
            Unavailable,
        )
        .await;
        // Undeclared/mutating button action.
        expect_err(
            "action",
            &ok_with_text("'x'").replace(
                "{type:'text',id:'t',text:TEXT}",
                "{type:'button',id:'b',label:'Go',action:{operation:'notes.update',input:{}}}",
            ),
            Unavailable,
        )
        .await;
        expect_err(
            "unknownfield",
            &ok_with_text("'x'").replace("text:TEXT}", "text:TEXT,html:'<b>'}"),
            Unavailable,
        )
        .await;
        expect_err(
            "exit",
            &format!("{}process.exitCode=3;", ok_with_text("'x'")),
            Unavailable,
        )
        .await;
        expect_err(
            "workererr",
            "process.stdout.write(JSON.stringify({protocol_version:1,invocation_id:req.invocation_id,ok:false,error:{code:'invalid_input',message:'no'}}))",
            InvalidInput,
        )
        .await;
    }

    #[tokio::test]
    async fn timeout_and_overflow_kill_promptly() {
        let t0 = std::time::Instant::now();
        let e = expect_err(
            "hang",
            "setInterval(()=>{},1000)",
            calternal_core::ErrorCode::Unavailable,
        )
        .await;
        assert!(e.message.contains("timed out"));
        assert!(t0.elapsed() < WORKER_TIMEOUT + Duration::from_secs(2));
        let t1 = std::time::Instant::now();
        let e = expect_err(
            "flood",
            "process.stdout.write('x'.repeat(300*1024));setInterval(()=>{},1000)",
            calternal_core::ErrorCode::Unavailable,
        )
        .await;
        assert!(e.message.contains("exceeded"), "{}", e.message);
        assert!(
            t1.elapsed() < Duration::from_secs(2),
            "overflow should kill before timeout"
        );
        expect_err(
            "errflood",
            "process.stderr.write('y'.repeat(64*1024));setInterval(()=>{},1000)",
            calternal_core::ErrorCode::Unavailable,
        )
        .await;
    }

    #[tokio::test]
    async fn admission_cap_rejects_when_saturated_and_releases() {
        let mut n = node().await;
        n.permits = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        let (_t1, hang) = plugin("caphang", "setInterval(()=>{},1000)");
        let (_t2, ok) = plugin("capok", &ok_with_text("'x'"));
        let n2 = n.clone();
        let first = tokio::spawn(async move { run_summarize(&n2, &hang, resource()).await });
        tokio::time::sleep(Duration::from_millis(300)).await;
        let busy = run_summarize(&n, &ok, resource())
            .await
            .expect_err("saturated");
        assert_eq!(busy.details.unwrap()["reason"], "worker_capacity");
        let e = first.await.unwrap().expect_err("timeout");
        assert!(e.message.contains("timed out"));
        // Capacity released after timeout; also released after an error.
        run_summarize(&n, &ok, resource())
            .await
            .expect("released after timeout");
        let (_t3, bad) = plugin("capbad", "process.stdout.write('junk')");
        run_summarize(&n, &bad, resource()).await.expect_err("bad");
        run_summarize(&n, &ok, resource())
            .await
            .expect("released after error");
    }

    #[tokio::test]
    async fn digest_mismatch_refuses_spawn() {
        let (_t, lp) = plugin("digest", &ok_with_text("'x'"));
        let (w, _) = lp.worker.clone().unwrap();
        std::fs::write(&w, "process.stdout.write('tampered')").unwrap();
        let e = run_summarize(&node().await, &lp, resource())
            .await
            .expect_err("refused");
        assert_eq!(e.code, calternal_core::ErrorCode::Unavailable);
        assert!(e.message.contains("verification"));
    }
}
