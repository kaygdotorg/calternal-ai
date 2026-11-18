//! Stage a verified executable next to the running one. The connection owns the
//! drain decision, and publishes the replacement only after backend shutdown.
use std::{
    io::Write,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use calternal_fsio::beneath::{Folder, TempFile};
use ring::digest::{Context, SHA256};
use tokio::io::AsyncWriteExt;

use crate::{
    Error, Result,
    link::{ConnectAddress, Link},
    release::{self, MAX_BINARY},
};

pub(crate) struct Staged {
    file: TempFile,
    target: PathBuf,
}

impl Staged {
    pub(crate) fn publish(self) -> Result<PathBuf> {
        let name = binary_name(&self.target)?;
        self.file.replace(name)?;
        Ok(self.target)
    }
}

pub(crate) fn restart(target: &Path) -> Result<()> {
    let error = std::process::Command::new(target)
        .args(std::env::args_os().skip(1))
        .exec();
    Err(error.into())
}

fn binary_name(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("installed executable path"))
}

fn invalid(what: &str) -> Error {
    Error::Protocol(format!("invalid machine release {what}"))
}

fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub(crate) async fn stage(server: &str, address: Option<&ConnectAddress>) -> Result<Staged> {
    let current = release::build_id().ok_or_else(|| invalid("build ID"))?;
    let key = release::public_key().ok_or_else(|| invalid("public key"))?;
    let link = Link::release_link(server, address)?;
    let target = std::env::current_exe()?;
    loop {
        match check(&link, current, &platform(), key, &target).await {
            Ok(Some(staged)) => return Ok(staged),
            Ok(None) => (),
            Err(error) => tracing::warn!(%error, "machine update check failed"),
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn small(link: &Link, path: &str, limit: usize) -> Result<Vec<u8>> {
    let mut response = link.release_get(path).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(invalid("metadata size"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn check(
    link: &Link,
    current: &str,
    platform: &str,
    key: &str,
    target: &Path,
) -> Result<Option<Staged>> {
    let root = release::PATH;
    let version = match small(link, &format!("{root}/version"), 81).await {
        Ok(version) => version,
        Err(error) if error.status() == Some(404) => return Ok(None),
        Err(error) => return Err(error),
    };
    let version = std::str::from_utf8(&version)
        .ok()
        .and_then(|v| v.strip_suffix('\n'))
        .filter(|v| release::valid_id(v))
        .ok_or_else(|| invalid("version"))?;
    if version == current {
        return Ok(None);
    }

    let prefix = format!("{root}/{version}/{platform}");
    let manifest = small(link, &format!("{prefix}/manifest"), 512).await?;
    let signature = small(link, &format!("{prefix}/manifest.sig"), 4096).await?;
    let key = key.to_owned();
    let (manifest, file) = {
        let target = target.to_owned();
        let version = version.to_owned();
        let platform = platform.to_owned();
        tokio::task::spawn_blocking(move || {
            verify(&key, &manifest, &signature)?;
            let manifest = parse(&manifest, &version, &platform)?;
            let parent = target.parent().ok_or_else(|| invalid("executable path"))?;
            let folder = Folder::open_existing(parent)?;
            let name = binary_name(&target)?;
            folder.check_replacement(name)?;
            Ok::<_, Error>((manifest, folder.create_temp(name)?))
        })
        .await
        .map_err(|_| invalid("verification task"))??
    };
    let mut output = file;
    let writer = output.file().try_clone()?;
    let mut writer = tokio::fs::File::from_std(writer);
    let mut response = link.release_get(&format!("{prefix}/calternal")).await?;
    let mut received = 0;
    let mut digest = Context::new(&SHA256);
    while let Some(chunk) = response.chunk().await? {
        received += chunk.len() as u64;
        if received > manifest.size {
            return Err(invalid("binary size"));
        }
        digest.update(&chunk);
        writer.write_all(&chunk).await?;
    }
    writer.flush().await?;
    if received != manifest.size || digest.finish().as_ref() != manifest.digest {
        return Err(invalid("binary digest or size"));
    }
    writer.set_permissions(std::fs::Permissions::from_mode(0o700)).await?;
    drop(writer);
    Ok(Some(Staged { file: output, target: target.to_owned() }))
}

struct Manifest {
    size: u64,
    digest: [u8; 32],
}

fn parse(data: &[u8], version: &str, platform: &str) -> Result<Manifest> {
    let text = std::str::from_utf8(data).map_err(|_| invalid("manifest encoding"))?;
    let mut lines = text
        .strip_suffix('\n')
        .ok_or_else(|| invalid("manifest framing"))?
        .split('\n');
    if lines.next() != Some("calternal-machine-release-v1")
        || lines.next() != Some(version)
        || lines.next() != Some(platform)
    {
        return Err(invalid("manifest version or platform"));
    }
    let size = lines.next()
        .filter(|n| !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|n| n.parse::<u64>().ok())
        .filter(|n| (1..=MAX_BINARY).contains(n))
        .ok_or_else(|| invalid("manifest size"))?;
    let hash = lines.next().filter(|h| h.len() == 64).ok_or_else(|| invalid("manifest digest"))?;
    if lines.next().is_some() || !hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(invalid("manifest fields"));
    }
    let mut digest = [0; 32];
    for (i, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hash[i * 2..i * 2 + 2], 16).map_err(|_| invalid("manifest digest"))?;
    }
    Ok(Manifest { size, digest })
}

fn verify(key: &str, manifest: &[u8], signature: &[u8]) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let allowed = dir.path().join("allowed_signers");
    let sig = dir.path().join("manifest.sig");
    std::fs::write(&allowed, format!("calternal-release {key}\n"))?;
    std::fs::write(&sig, signature)?;
    let mut verify = std::process::Command::new("ssh-keygen")
        .args(["-Y", "verify", "-I", "calternal-release", "-n", "calternal-machine-release-v1", "-f"])
        .arg(allowed)
        .arg("-s")
        .arg(sig)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let written = verify.stdin.take().expect("piped stdin").write_all(manifest);
    let result = verify.wait()?;
    if written.is_ok() && result.success() {
        Ok(())
    } else {
        Err(invalid("signature"))
    }
}
