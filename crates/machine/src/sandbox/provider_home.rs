//! Per-space homes and machine-local provider profiles.
use std::io;
use std::path::{Path, PathBuf};

use calternal_fsio::private;

use super::{SandboxError, SandboxResult};
use crate::protocol::SandboxSpec;

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
            calternal_plugin_host::shared_tools::import_legacy_profile(&account, &login)?;
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
