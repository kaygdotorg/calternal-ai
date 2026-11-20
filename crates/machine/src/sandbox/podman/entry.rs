//! Start one contributed entry without inheriting the native workspace or home.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use calternal_plugin_host::entry::{self, EntryProgram};
use std::{io::Write, os::unix::fs::OpenOptionsExt, path::Path};

const ENTRY_MOUNT: &str = "/calternal-entry";

impl PodmanBackend {
    /// Verify the staged image and package asset before binding the entry. The
    /// caller owns attempt admission, timeout enforcement and verified teardown.
    /// Costs one image inspection, one bounded hash/write and two engine calls.
    pub(super) async fn launch_entry(
        &self,
        ticket: Uuid,
        program: &EntryProgram,
        live: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> SandboxResult<(PodmanSandbox, Exec)> {
        if self.guest || self.kvm_fallback || ticket.is_nil() || !live() {
            return Err(SandboxError("entry placement is unavailable".into()));
        }
        let input = entry::encode_invoke(&program.invocation).map_err(SandboxError::new)?;
        let input = String::from_utf8(input).map_err(SandboxError::new)?;
        let image_id =
            verified_image(&self.podman, &program.image_source, &program.image_digest).await?;
        // The outer carrier caps this field before parsing. Decode only once.
        if program.entry_base64.len() > super::super::worker::MAX_SOURCE_BYTES.div_ceil(3) * 4 {
            return Err(SandboxError("entry asset exceeds launch limit".into()));
        }
        let bytes = STANDARD
            .decode(&program.entry_base64)
            .map_err(|_| SandboxError("invalid entry asset encoding".into()))?;
        let root = self.canonical_root().await?;
        // Podman uses colon delimiters in bind mounts. Refuse a path that
        // cannot name the staged inode exactly in the engine argument.
        if !root.to_str().is_some_and(|path| !path.contains(':')) {
            return Err(SandboxError("invalid entry staging root".into()));
        }
        let dir = stage_entry(root.join("entries"), program.entry_sha256.clone(), bytes).await?;
        let asset = dir.join("entry");

        let cidfile = dir.join("container.id");
        let mount = format!("{}:{ENTRY_MOUNT}:ro", asset.display());
        let label = ownership::root_label(&root);
        let name = format!(
            "calternal-{}",
            dir.file_name()
                .expect("private entry name")
                .to_string_lossy()
        );
        let attempt_label = format!("org.calternal.entry-ticket={ticket}");
        let created = podman(
            &self.podman,
            &[
                "create",
                "--interactive",
                "--pull=never",
                "--network=none",
                "--init",
                "--pid=private",
                "--ipc=private",
                "--cap-drop=all",
                "--security-opt=no-new-privileges",
                "--read-only",
                "--read-only-tmpfs=false",
                "--log-driver=none",
                "--http-proxy=false",
                "--no-healthcheck",
                "--image-volume=ignore",
                "--memory=256m",
                "--memory-swap=256m",
                "--pids-limit=64",
                "--cpus=1",
                "--timeout=60",
                "--stop-timeout=0",
                "--workdir=/",
                "--entrypoint",
                ENTRY_MOUNT,
                "--volume",
                &mount,
                "--cidfile",
                &cidfile.to_string_lossy(),
                "--label",
                &label,
                "--label",
                &attempt_label,
                "--name",
                &name,
                &image_id,
            ],
        )
        .await;
        let container = match created {
            Ok(container) => container,
            Err(_) => {
                ownership::remove_named(&self.podman, &root, &name).await?;
                remove_stage(&dir).await?;
                return Err(SandboxError("cannot create entry sandbox".into()));
            }
        };
        let mut command = tokio::process::Command::new(&self.podman);
        command.args(["start", "--attach", "--interactive", &container]);
        command.as_std_mut().process_group(0);
        let sandbox = PodmanSandbox {
            podman: self.podman.clone(),
            container,
            dir,
            clients: Mutex::new(HashSet::new()),
            worker: true,
            broker: None,
        };
        if !live() {
            sandbox.stop().await?;
            return Err(SandboxError("entry placement was fenced".into()));
        }
        let spawned = tokio::task::spawn_blocking(move || {
            if !live() {
                return Err(SandboxError("entry placement was fenced".into()));
            }
            super::super::spawn_streaming(command, input, false, true)
        })
        .await
        .map_err(SandboxError::new)
        .and_then(|result| result);
        match spawned {
            Ok((exec, pid)) => {
                if let Some(pid) = pid {
                    sandbox.clients.lock().expect("clients").insert(pid as i32);
                }
                Ok((sandbox, exec))
            }
            Err(error) => {
                sandbox.stop().await?;
                Err(error)
            }
        }
    }
}

// Inspect only the exact repository-qualified manifest digest. Run the
// local config ID from that inspection so alias changes cannot race launch.
async fn verified_image(engine: &Path, source: &str, digest: &str) -> SandboxResult<String> {
    if source.is_empty()
        || source.len() > 256
        || !source
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"./_:-".contains(&byte))
        || !source.contains('/')
        || !digest.strip_prefix("sha256:").is_some_and(is_hash)
    {
        return Err(SandboxError("invalid entry image pin".into()));
    }
    let reference = format!("{source}@{digest}");
    let inspect = podman(
        engine,
        &[
            "image",
            "inspect",
            "--format",
            "{{.Id}}\n{{range .RepoDigests}}{{println .}}{{end}}",
            "--",
            &reference,
        ],
    )
    .await
    .map_err(|_| SandboxError("entry image pin is not provisioned".into()))?;
    let mut lines = inspect.lines();
    let id = lines
        .next()
        .filter(|id| is_hash(id) || id.strip_prefix("sha256:").is_some_and(is_hash));
    if let Some(id) = id
        && lines.any(|known| known == reference)
    {
        return Ok(if id.starts_with("sha256:") {
            id.to_owned()
        } else {
            format!("sha256:{id}")
        });
    }
    Err(SandboxError("entry image source or digest changed".into()))
}

fn is_hash(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

// The staged inode is private and exclusive. It is discarded on every
// verified exit or reconciled on restart, so it needs no content flush.
async fn stage_entry(
    parent: PathBuf,
    digest: String,
    entry_bytes: Vec<u8>,
) -> SandboxResult<PathBuf> {
    if entry_bytes.is_empty()
        || entry_bytes.len() > super::super::worker::MAX_SOURCE_BYTES
        || !is_hash(&digest)
    {
        return Err(SandboxError("invalid entry asset pin".into()));
    }
    tokio::task::spawn_blocking(move || {
        let hash = ring::digest::digest(&ring::digest::SHA256, &entry_bytes);
        let digits = b"0123456789abcdef";
        let encoded = hash
            .as_ref()
            .iter()
            .flat_map(|byte| [digits[(byte >> 4) as usize], digits[(byte & 15) as usize]]);
        if !encoded.eq(digest.bytes()) {
            return Err(SandboxError("entry asset digest changed".into()));
        }
        calternal_fsio::private::directory(&parent).map_err(SandboxError::new)?;
        let dir = tempfile::Builder::new()
            .prefix("entry-")
            .tempdir_in(parent)
            .map_err(SandboxError::new)?;
        std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o555)
            .open(dir.path().join("entry"))
            .and_then(|mut file| file.write_all(&entry_bytes))
            .map_err(SandboxError::new)?;
        Ok(dir.keep())
    })
    .await
    .map_err(SandboxError::new)?
}

async fn remove_stage(dir: &Path) -> SandboxResult<()> {
    match tokio::fs::remove_dir_all(dir).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SandboxError::new(error)),
    }
}
