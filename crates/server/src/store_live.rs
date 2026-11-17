//! Startup-only content handle extraction for request-independent live streams.
use super::*;
impl Store {
    /// Clones the generic service once during router construction. Cost: one Arc
    /// increment, no database work, journal bridge or fsync. The router holds the
    /// handle so live requests never take the installation-wide adapter mutex.
    pub fn shared_content(&self) -> std::sync::Arc<Content> {
        self.content.clone()
    }
}
