//! Configuration and memory reservations for small guests.

use super::*;
use crate::protocol::{SMALL_VM_IDLE_SECONDS, SmallVmCapacity};

impl SmallRuntime {
    /// Read the small VM configuration without starting a VMM.
    pub(in crate::sandbox) async fn load(root: PathBuf, path: &Path) -> SandboxResult<Self> {
        let mut config = Config::load_profile(path, true).await?;
        let memory_limit_mib = config
            .resident_memory_mib
            .unwrap_or(config.memory_mib * config.max_guests as u32);
        config.max_guests = config
            .max_guests
            .min((memory_limit_mib / config.memory_mib) as usize);
        Ok(Self {
            state: MicrovmBackend::new(root.join("space-runtime"), config).0,
            slots: Mutex::default(),
            version: OnceCell::new(),
            memory_limit_mib,
        })
    }

    /// Report live reservations from the same semaphore that limits VM starts.
    /// Initialization failures leave the backend unavailable for placement.
    pub(in crate::sandbox) fn capacity(&self) -> Option<SmallVmCapacity> {
        self.state.owned.get()?;
        let config = &self.state.config;
        let resident_count = (config.max_guests - self.state.capacity.available_permits()) as u32;
        Some(SmallVmCapacity {
            idle_seconds: config.idle_seconds.unwrap_or(SMALL_VM_IDLE_SECONDS),
            memory_limit_bytes: u64::from(self.memory_limit_mib) << 20,
            max_residents: config.max_guests as u32,
            resident_count,
            resident_memory_bytes: u64::from(resident_count) * (u64::from(config.memory_mib) << 20),
        })
    }
}
