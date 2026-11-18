//! Host-installed TAP policy and bounded attachment leases.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::net::Ipv4Addr;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Deserialize;

use super::{SandboxError, SandboxResult};

#[derive(Deserialize)]
struct Policy {
    slots: Vec<Tap>,
}

#[derive(Clone, Deserialize)]
pub(super) struct Tap {
    pub(super) name: String,
    pub(super) address: Ipv4Addr,
    pub(super) gateway: Ipv4Addr,
    pub(super) prefix: u8,
    pub(super) dns: Ipv4Addr,
}

/// One assigned TAP kept until the Firecracker child exits.
pub(super) struct Lease {
    tap: Tap,
    pool: Arc<Pool>,
}
impl Lease {
    pub(super) fn tap(&self) -> &Tap {
        &self.tap
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.pool
            .slots
            .lock()
            .expect("TAP slots")
            .push(self.tap.clone());
    }
}

pub(super) struct Pool {
    slots: Mutex<Vec<Tap>>,
}

impl Pool {
    /// A deployment can omit networking entirely. An explicitly configured path
    /// must match a root-installed policy for the current network namespace.
    pub(super) async fn load(
        path: Option<PathBuf>,
        max_guests: usize,
    ) -> SandboxResult<Option<Arc<Self>>> {
        let Some(path) = path else { return Ok(None) };
        tokio::task::spawn_blocking(move || {
            read_policy(&path, max_guests)
                .map(|policy| {
                    Some(Arc::new(Self {
                        slots: Mutex::new(policy.slots),
                    }))
                })
                .map_err(|error| SandboxError(format!("microVM egress is closed: {error}")))
        })
        .await
        .map_err(SandboxError::new)?
    }

    pub(super) fn lease(self: &Arc<Self>) -> SandboxResult<Lease> {
        let tap = self
            .slots
            .lock()
            .expect("TAP slots")
            .pop()
            .ok_or_else(|| SandboxError("no installed guest TAP is available".into()))?;
        Ok(Lease {
            tap,
            pool: Arc::clone(self),
        })
    }
}

fn root_file(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags((rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32)
        .open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 || meta.len() > limit {
        return Err(io::Error::other(
            "the policy and its installation receipt must be bounded root-owned files",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit as usize {
        return Err(io::Error::other(
            "the network policy exceeds its size bound",
        ));
    }
    Ok(bytes)
}

fn read_policy(path: &Path, max_guests: usize) -> io::Result<Policy> {
    let bytes = root_file(path, 64 * 1024)?;
    let receipt = root_file(&path.with_extension("installed"), 256)?;
    let inode = fs::metadata("/proc/self/ns/net")?.ino();
    let boot = fs::read_to_string("/proc/sys/kernel/random/boot_id")?;
    use std::fmt::Write as _;
    let mut digest = String::new();
    for byte in ring::digest::digest(&ring::digest::SHA256, &bytes).as_ref() {
        write!(digest, "{byte:02x}").expect("format SHA-256");
    }
    if receipt != format!("{} {inode} {digest}\n", boot.trim()).as_bytes() {
        return Err(io::Error::other(
            "install the declared firewall policy in this network namespace",
        ));
    }
    let policy: Policy = serde_json::from_slice(&bytes)?;
    let mut names = HashSet::new();
    let mut addresses = HashSet::new();
    if policy.slots.len() != max_guests
        || policy.slots.iter().any(|slot| {
            !slot.name.starts_with("calvm")
                || slot.name.len() > 15
                || !slot
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || !names.insert(&slot.name)
                || !addresses.insert(slot.address)
                || !addresses.insert(slot.gateway)
                || !(24..=30).contains(&slot.prefix)
        })
    {
        return Err(io::Error::other(
            "install one unique TAP and address pair per configured guest slot",
        ));
    }
    Ok(policy)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn explicitly_missing_policy_prevents_networked_startup() {
        let dir = tempfile::tempdir().unwrap();
        assert!(super::Pool::load(None, 1).await.unwrap().is_none());
        let result = super::Pool::load(Some(dir.path().join("missing.json")), 1).await;
        assert!(matches!(result, Err(error) if error.0.contains("egress is closed")));
    }

    #[tokio::test]
    async fn policy_without_operator_receipt_cannot_open_a_tap() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("policy.json");
        std::fs::write(&path, b"{\"slots\":[]}").unwrap();
        assert!(super::Pool::load(Some(path), 1).await.is_err());
    }
}
