//! Crash-point fault injection for durability tests. Compiled into debug
//! builds only; release builds ignore `CALTERNAL_FAULT` entirely.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    AfterCommit,
    BeforePlacement,
    AfterPlacement,
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

#[cfg(not(debug_assertions))]
#[inline(always)]
pub fn hit(_point: FaultPoint) {}
