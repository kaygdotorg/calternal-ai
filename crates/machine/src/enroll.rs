//! Enrollment: exchange a one-time code for a machine identity.
//!
//! The key pair is generated here; only the public key is sent. The code is
//! single-use and expires (the server stores only its hash). Credentials are
//! written 0600 before this returns.

use std::path::Path;
use std::time::Duration;

use crate::identity::{Credentials, MachineKey};
use crate::link::{ConnectAddress, Link};
use crate::protocol::{EnrolRequest, EnrolResponse};
use crate::{Error, Result};

/// Enrol this machine with `server` using `code`, saving credentials in `dir`.
///
/// Refuses when `dir` already holds credentials, so a second enrollment never
/// silently replaces a working identity; delete the file to enroll again.
pub async fn enroll(
    server: &str,
    code: &str,
    dir: &Path,
    name: &str,
    direct: bool,
) -> Result<Credentials> {
    enroll_at(server, code, dir, name, direct, None).await
}

pub(crate) async fn enroll_at(
    server: &str,
    code: &str,
    dir: &Path,
    name: &str,
    direct: bool,
    address: Option<&ConnectAddress>,
) -> Result<Credentials> {
    if Credentials::path(dir).exists() {
        return Err(Error::Protocol(format!(
            "already enrolled: {} exists",
            Credentials::path(dir).display()
        )));
    }
    let key = MachineKey::generate()?;
    let link = Link::with_connect_address(server, address)?;
    let req = EnrolRequest {
        code: code.trim().to_owned(),
        public_key: key.public_key_b64(),
        name: name.chars().take(64).collect(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        direct,
    };
    let resp: EnrolResponse = link
        .post(
            "/api/v1/machine/enroll",
            &req,
            false,
            Duration::from_secs(30),
        )
        .await?;
    let creds = Credentials {
        server: link.base().to_owned(),
        machine_id: resp.machine_id,
        owner: resp.owner,
        key,
    };
    creds.save(dir)?;
    Ok(creds)
}

/// This computer's host name, for the machine label. Falls back to "machine".
pub fn host_name() -> String {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOSTNAME").ok())
        .unwrap_or_else(|| "machine".into())
}
