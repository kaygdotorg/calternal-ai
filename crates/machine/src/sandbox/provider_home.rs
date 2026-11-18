//! Per-space homes and the machine-local Claude login directory.
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use calternal_fsio::{beneath::Folder, private};

use super::{SandboxError, SandboxResult};
use crate::protocol::SandboxSpec;

const CREDENTIALS: &str = ".credentials.json";
const MAX_CREDENTIAL_BYTES: u64 = 1024 * 1024;

pub(super) async fn prepare(
    account: PathBuf,
    work: &Path,
    spec: &SandboxSpec,
) -> SandboxResult<(PathBuf, PathBuf)> {
    if spec.worker.is_some() {
        return Ok((work.to_owned(), PathBuf::new()));
    }
    let space = spec
        .space_id
        .ok_or_else(|| SandboxError("missing sandbox space".into()))?;
    let home = if spec.provider_login {
        work.join("login-home")
    } else {
        account.join("spaces").join(space.to_string()).join("home")
    };
    let shared = spec.shared_provider_sponsor;
    let login = if let Some(sponsor) = shared {
        let root = account.parent().ok_or_else(|| SandboxError("missing account root".into()))?;
        root.join(sponsor.to_string()).join("spaces").join(space.to_string()).join("provider-login")
    } else {
        account.join("provider-login")
    };
    tokio::task::spawn_blocking(move || {
        private::directory(&account)?;
        private::directory(&login)?;
        private::directory(&home)?;
        if shared.is_none() {
            import_existing(&account, &login)?;
        }
        Ok::<_, io::Error>((home, login))
    })
    .await
    .map_err(SandboxError::new)?
    .map_err(SandboxError::new)
}

fn import_existing(account: &Path, login: &Path) -> io::Result<()> {
    let target = login.join(CREDENTIALS);
    if target.try_exists()? {
        return Ok(());
    }
    let legacy = match Folder::open(account)
        .and_then(|root| root.child("home", false))
        .and_then(|dir| dir.child(".claude", false))
        .and_then(|dir| dir.open_file(CREDENTIALS))
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut bytes = Vec::new();
    legacy
        .take(MAX_CREDENTIAL_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_CREDENTIAL_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "provider credential file exceeds limit",
        ));
    }
    let Ok(legacy) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return Ok(());
    };
    let Some(claude_login) = legacy
        .get("claudeAiOauth")
        .filter(|value| value.is_object())
    else {
        return Ok(());
    };
    // Older homes may also hold credentials for MCP servers and plugins.
    let bytes = serde_json::to_vec(&serde_json::json!({"claudeAiOauth":claude_login}))?;
    match private::publish(&target, &bytes) {
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        result => result,
    }
}
