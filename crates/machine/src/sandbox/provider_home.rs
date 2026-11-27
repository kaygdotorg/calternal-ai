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
        let root = account
            .parent()
            .ok_or_else(|| SandboxError("missing account root".into()))?;
        root.join(sponsor.to_string())
            .join("spaces")
            .join(space.to_string())
            .join("provider-login")
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

pub(super) async fn prepare_harness(
    account: PathBuf,
    work: &Path,
    spec: &SandboxSpec,
    harness: &crate::protocol::HarnessSandbox,
    run_sign_in: Option<&Path>,
) -> SandboxResult<(PathBuf, PathBuf)> {
    let provider = &harness.provider_id;
    if spec.worker.is_some()
        || provider.is_empty()
        || provider.len() > 80
        || !provider
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(SandboxError("invalid harness sandbox".into()));
    }
    let space = spec
        .space_id
        .ok_or_else(|| SandboxError("missing sandbox space".into()))?;
    let home = if spec.provider_login {
        work.join("login-home")
    } else {
        account
            .join("spaces")
            .join(space.to_string())
            .join("provider-homes")
            .join(provider)
    };
    use crate::protocol::ProviderLoginScope;
    let scope_id = match harness.login_scope {
        ProviderLoginScope::Account(id) if id.as_uuid() == spec.account_id => id.as_uuid(),
        ProviderLoginScope::Account(_) => {
            return Err(SandboxError("provider login scope mismatches owner".into()));
        }
        ProviderLoginScope::Space(_) => {
            return Err(SandboxError(
                "space-scoped provider login is not enabled".into(),
            ));
        }
    };
    if let Some(login) = run_sign_in {
        if provider != "calternal-chat-claude-code" || spec.provider_login {
            return Err(SandboxError(
                "Operator sign-in is limited to Claude Code runs".into(),
            ));
        }
        let home = work.join("run-home");
        tokio::fs::create_dir_all(&home)
            .await
            .map_err(SandboxError::new)?;
        return Ok((home, login.to_owned()));
    }
    if harness.legacy_profile && !harness.disposable_profile {
        return prepare(account, work, spec).await;
    }
    let profiles = if harness.disposable_profile {
        work.join("smoke-profile")
    } else {
        account.join("provider-profiles").join(scope_id.to_string())
    };
    let login = profiles.join(provider);
    tokio::task::spawn_blocking(move || {
        for path in [&account, &profiles, &login, &home] {
            private::directory(path)?;
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
