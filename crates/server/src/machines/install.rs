//! A personal, single-use installer rendered from the standalone machine script.
use super::*;
use crate::auth::Directory;
use calternal_core::MachineInstallResult;
use serde_json::{Value, json};

const INSTALLER: &str = include_str!("../../../../scripts/install-machine.sh");
const TTL: Duration = Duration::from_secs(15 * 60);

/// Issue a personal code and render the installer. The script and code are
/// delivered only to this request's person session. One protected code write.
pub(crate) async fn script<D: Directory>(
    state: crate::api::Shared<D>,
    machines: Arc<MachineService>,
    account: AccountId,
) -> Result<Value, Error> {
    let server = state
        .directory
        .http_auth()
        .ok_or_else(|| Error::unavailable("configure the public origin before enrolling machines"))?
        .config
        .origin_header();
    let image = std::env::var("CALTERNAL_MACHINE_INSTALL_IMAGE").map_err(|_| {
        Error::unavailable("Ask the server administrator to configure the standard sandbox image.")
    })?;
    if image.is_empty()
        || !image
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"./:@_-".contains(&b))
    {
        return Err(Error::unavailable(
            "The configured sandbox image name is invalid.",
        ));
    }
    blocking(move || {
        let (code, expires_ms) = machines.issue_code_for(Owner::Account { id: account }, TTL)?;
        let quote = |value: &str| format!("'{}'", value.replace('\'', "'\\''"));
        let script = INSTALLER.replace(
            "server= code= image=",
            &format!(
                "server={} code={} image={}",
                quote(&server),
                quote(&code),
                quote(&image)
            ),
        );
        Ok(json!(MachineInstallResult { script, expires_ms }))
    })
    .await
}
