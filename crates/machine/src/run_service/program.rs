use crate::egress::EgressPolicy;
use std::path::{Component, Path, PathBuf};

use super::RunError;

/// How the host resolved the run's data scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunScope {
    /// The current chat session, fixed to its target list when the run starts.
    Session,
}

/// One validated program binding from an installed plugin manifest.
#[derive(Clone, Debug)]
pub struct ProgramBinding {
    /// Installed plugin identity.
    pub(super) plugin_id: String,
    /// Program ID selected by the trusted host route.
    pub(super) program_id: String,
    /// Canonical plugin bundle root.
    pub(super) plugin_root: PathBuf,
    /// Relative executable or image path declared in the plugin manifest.
    pub(super) relative_path: PathBuf,
    /// Whether the manifest binding names an image.
    pub(super) image: bool,
    /// Exact outbound hosts declared by the installed plugin.
    pub(super) policy: EgressPolicy,
}

impl ProgramBinding {
    /// Bind a program ID to a plugin-relative executable or image path and the
    /// exact outbound hosts taken from the host-validated manifest.
    pub fn new(
        plugin_id: impl Into<String>,
        program_id: impl Into<String>,
        plugin_root: impl Into<PathBuf>,
        relative_path: impl Into<PathBuf>,
        image: bool,
        outbound_hosts: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> Result<Self, RunError> {
        let plugin_id = plugin_id.into();
        let program_id = program_id.into();
        let relative_path = relative_path.into();
        if !manifest_id(&plugin_id) || !manifest_id(&program_id) || !plugin_path(&relative_path) {
            return Err(RunError::InvalidProgram);
        }
        let policy = EgressPolicy::new(outbound_hosts)?;
        Ok(Self {
            plugin_id,
            program_id,
            plugin_root: plugin_root.into(),
            relative_path,
            image,
            policy,
        })
    }

    /// Program declaration ID used for host-side run selection.
    pub fn id(&self) -> &str {
        &self.program_id
    }
}

fn manifest_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn plugin_path(path: &Path) -> bool {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }
    path.components().all(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        let Some(name) = name.to_str() else {
            return false;
        };
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    })
}
