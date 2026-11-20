//! Size and rate limits read from the pinned Plugin API v2 catalogue.
use std::sync::OnceLock;

/// Shared contribution envelope, spool, and producer budgets. The host and
/// durable writer use the same pinned values so their limits cannot diverge.
#[derive(Debug, serde::Deserialize)]
pub struct ContributionLimits {
    /// Maximum encoded JSON bytes per frame, including LF.
    pub encoded_frame_bytes: i64,
    /// Maximum retained encoded frame bytes for one call.
    pub encoded_spool_bytes: i64,
    /// Encoded bytes per second permitted for one attempt.
    pub encoded_bytes_per_second_per_attempt: i64,
    /// Producer frames per second permitted for one attempt.
    pub frames_per_second_per_attempt: i64,
    /// Encoded bytes per second permitted for one space.
    pub encoded_bytes_per_second_per_space: i64,
    /// Producer frames per second permitted for one space.
    pub frames_per_second_per_space: i64,
}

/// Return the pinned budget set. Initial use parses the bundled catalogue
/// once. Later uses read an immutable process-local value without I/O.
pub fn contribution_limits() -> &'static ContributionLimits {
    static LIMITS: OnceLock<ContributionLimits> = OnceLock::new();
    LIMITS.get_or_init(|| {
        let catalogue: serde_json::Value =
            serde_json::from_str(include_str!("../../../contract/host-catalogue-v2.json"))
                .expect("pinned contribution catalogue");
        serde_json::from_value(catalogue["extension_points"]["calls"]["limits"].clone())
            .expect("pinned contribution limits")
    })
}
