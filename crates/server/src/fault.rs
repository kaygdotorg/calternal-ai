//! Crash-point fault injection for durability tests. Compiled into debug
//! builds only; release builds ignore `CALTERNAL_FAULT` entirely.


/// Durable write boundaries available to disposable crash tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// SQLite committed the revision and receipt, before materialization.
    AfterCommit,
    /// Materialization is about to publish the new file bytes.
    BeforePlacement,
    /// File bytes were published, before placement bookkeeping finishes.
    AfterPlacement,
    /// All handler work finished, before the transport sends its reply.
    BeforeReply,
}

impl FaultPoint {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    fn name(self) -> &'static str {
        match self {
            Self::AfterCommit => "after_commit",
            Self::BeforePlacement => "before_placement",
            Self::AfterPlacement => "after_placement",
            Self::BeforeReply => "before_reply",
        }
    }
}

/// Aborts a debug process at the selected environment-controlled boundary, once armed.
#[cfg(debug_assertions)]
pub fn hit(point: FaultPoint) {
    use std::sync::OnceLock;
    static ARMED: OnceLock<Option<String>> = OnceLock::new();
    let armed = ARMED.get_or_init(|| std::env::var("CALTERNAL_FAULT").ok());
    if armed.as_deref() == Some(point.name()) {
        eprintln!("calternal: injected fault at {}; aborting", point.name());
        std::process::abort();
    }
}

/// Release-build no-op; environment variables cannot arm fault injection.
#[cfg(not(debug_assertions))]
#[inline(always)]
pub fn hit(_point: FaultPoint) {}
