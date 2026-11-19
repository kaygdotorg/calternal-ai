//! Validate the operator configuration before opening VM resources.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{GuestSize, SandboxError, SandboxResult};

/// Operator-owned configuration read once when starting the machine role.
/// CPU and memory apply to a whole account guest. The disk size applies to
/// newly created account disks and is stored in the disk itself.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Absolute executable path for the Firecracker release build.
    pub firecracker: PathBuf,
    /// Absolute path of the verified immutable guest image directory.
    pub image_directory: PathBuf,
    /// SHA-256 of the image's fixed-order digest manifest, in lowercase hex.
    pub image_digest: String,
    /// Path to the ext4 filesystem formatter on the host.
    pub mkfs_ext4: PathBuf,
    /// Virtual CPUs per account, between 1 and 32.
    pub vcpus: u8,
    /// Guest RAM in MiB. Must be a supported account guest size.
    pub memory_mib: u32,
    /// Logical data disk ceiling per account in MiB, between 1024 and 65536.
    pub disk_mib: u32,
    /// Prebooted guests without user data, between 0 and 2.
    pub warm_guests: usize,
    /// Maximum VMMs, including idle and unassigned guests, between 1 and 64.
    pub max_guests: usize,
    /// Absolute operator policy path for a networked guest. Omit for offline guests.
    #[serde(default)]
    pub network_policy: Option<PathBuf>,
}

impl Config {
    /// Read and validate the small operator configuration on a blocking task.
    /// Disk and VM resources are untouched until backend initialization.
    pub async fn load(path: &Path) -> SandboxResult<Self> {
        Self::load_profile(path, false).await
    }

    /// Read configuration and enforce the resource and network limits of the selected guest profile.
    pub(super) async fn load_profile(path: &Path, small: bool) -> SandboxResult<Self> {
        let file = tokio::fs::read(path).await.map_err(SandboxError::new)?;
        let config: Self = serde_json::from_slice(&file).map_err(SandboxError::new)?;
        if !config.firecracker.is_absolute()
            || !config.image_directory.is_absolute()
            || !config.mkfs_ext4.is_absolute()
            || config
                .network_policy
                .as_ref()
                .is_some_and(|path| !path.is_absolute())
            || !(1..=32).contains(&config.vcpus)
            || if small {
                config.memory_mib != 256
                    || config.vcpus != 2
                    || config.network_policy.is_some()
                    || config.warm_guests != 0
            } else {
                GuestSize::from_mib(config.memory_mib).is_none()
            }
            || !(1024..=65536).contains(&config.disk_mib)
            || !(1..=64).contains(&config.max_guests)
            || config.warm_guests > 2
            || config.warm_guests >= config.max_guests
        {
            return Err(SandboxError(
                "invalid microVM paths or resource limits".into(),
            ));
        }
        Ok(config)
    }
}
