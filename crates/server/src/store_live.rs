//! Startup-only content handle extraction for request-independent live streams.
use super::*;
impl Store {
    /// Create a cheap account-input handle for startup wiring; no I/O or locks.
    pub fn account_inputs(&self) -> crate::account_input::Inputs {
        crate::account_input::Inputs::new(&self.appdata, Default::default())
            .with_content(self.content.clone())
    }

    /// Clones the generic service once during router construction. Cost: one Arc
    /// increment, no database work, journal bridge or fsync. The router holds the
    /// handle so live requests never take the installation-wide adapter mutex.
    pub fn shared_content(&self) -> std::sync::Arc<Content> {
        self.content.clone()
    }
}
