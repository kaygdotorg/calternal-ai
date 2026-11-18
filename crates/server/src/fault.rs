//! Crash-point fault injection for durability tests. Compiled into debug
//! builds only; release builds ignore `CALTERNAL_FAULT` entirely.

/// Durable write boundaries available to disposable crash tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// Thread message and replay intent placed, before durable turn admission.
    AfterThreadMessage,
    /// Content revision and publication intent committed, before journal publication.
    AfterCommit,
    /// The journal receipt and event committed before the outbox was retired.
    AfterJournalCommit,
    /// Materialization is about to publish the new file bytes.
    BeforePlacement,
    /// File bytes were published, before placement bookkeeping finishes.
    AfterPlacement,
    /// All handler work finished, before the transport sends its reply.
    BeforeReply,
    /// SQLite witnessed the erasure, before the recovery marker is published.
    ErasureAuthorityCommitted,
    /// Owner authority and pending erasure are durably published.
    ErasureRecorded,
    /// Machine erasures are durably queued, before removing the host key or trees.
    ErasureMachinesQueued,
    /// In-flight owner writes have drained and the key was erased.
    ErasureKeyRemoved,
    /// All owner paths were removed before the cleanup receipt.
    ErasureStorageRemoved,
}

impl FaultPoint {
    #[cfg_attr(not(debug_assertions), allow(dead_code))]
    fn name(self) -> &'static str {
        match self {
            Self::AfterThreadMessage => "after_thread_message",
            Self::AfterCommit => "after_commit",
            Self::AfterJournalCommit => "after_journal_commit",
            Self::BeforePlacement => "before_placement",
            Self::AfterPlacement => "after_placement",
            Self::BeforeReply => "before_reply",
            Self::ErasureRecorded => "erasure_recorded",
            Self::ErasureMachinesQueued => "erasure_machines_queued",
            Self::ErasureAuthorityCommitted => "erasure_authority_committed",
            Self::ErasureKeyRemoved => "erasure_key_removed",
            Self::ErasureStorageRemoved => "erasure_storage_removed",
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
