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
    /// Occurrence and cursor staged in SQLite, before commit.
    AutomationBeforeQueueCommit,
    /// Occurrence and cursor committed together.
    AutomationAfterQueueCommit,
    /// Outbox claim and machine selection committed.
    AutomationAfterClaim,
    /// Stable thread created, before writing the prompt.
    AutomationAfterThread,
    /// Prompt and replay intent committed, before job admission.
    AutomationAfterPrompt,
    /// Provider job committed, before publishing the thread admission.
    AutomationAfterJob,
    /// Thread admission published, before releasing the scheduler outbox.
    AutomationAfterAdmission,
    /// Outbox links to the thread and job, before scheduling the turn.
    AutomationAfterRelease,
    /// First turn attempt claimed.
    AutomationAfterTurnClaim,
    /// Machine lease committed, before the job knows its lease ID.
    AutomationAfterMachineLease,
    /// Job checkpoint records the lease ID.
    AutomationAfterLeaseCheckpoint,
    /// Durable machine placement is visible before the job publishes it.
    AutomationAfterMachinePlacement,
    /// Job records its placement before issuing the agent token.
    AutomationAfterPlacement,
    /// Agent token issued before dispatching machine execution.
    AutomationAfterToken,
    /// Execution authorized and about to be queued for the machine.
    AutomationBeforeExec,
    /// Execution command queued for machine delivery.
    AutomationAfterExec,
    /// Output from the dispatched machine exec reached the native turn.
    AutomationAfterMachineOutput,
    /// The stopped installation is locked before examining its stores.
    UpgradeLocked,
    /// All selected store versions were checked.
    UpgradeVersionsChecked,
    /// One snapshot file was flushed before the intent was published.
    UpgradeCopied,
    /// The restore point and preparing intent are durable.
    UpgradeIntent,
    /// The candidate migration transactions finished.
    UpgradeMigrated,
    /// The schema and recovery witness checks passed.
    UpgradeVerified,
    /// The cutover decision is durable and permits only code rollback.
    UpgradeCommitted,
    /// One original file or absent sidecar was restored.
    UpgradeRestoreFile,
    /// The original data is durable before retiring the restore point.
    UpgradeRestored,
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
            Self::AutomationBeforeQueueCommit => "automation_before_queue_commit",
            Self::AutomationAfterQueueCommit => "automation_after_queue_commit",
            Self::AutomationAfterClaim => "automation_after_claim",
            Self::AutomationAfterThread => "automation_after_thread",
            Self::AutomationAfterPrompt => "automation_after_prompt",
            Self::AutomationAfterJob => "automation_after_job",
            Self::AutomationAfterAdmission => "automation_after_admission",
            Self::AutomationAfterRelease => "automation_after_release",
            Self::AutomationAfterTurnClaim => "automation_after_turn_claim",
            Self::AutomationAfterMachineLease => "automation_after_machine_lease",
            Self::AutomationAfterLeaseCheckpoint => "automation_after_lease_checkpoint",
            Self::AutomationAfterMachinePlacement => "automation_after_machine_placement",
            Self::AutomationAfterPlacement => "automation_after_placement",
            Self::AutomationAfterToken => "automation_after_token",
            Self::AutomationBeforeExec => "automation_before_exec",
            Self::AutomationAfterExec => "automation_after_exec",
            Self::AutomationAfterMachineOutput => "automation_after_machine_output",
            Self::UpgradeLocked => "upgrade_locked",
            Self::UpgradeVersionsChecked => "upgrade_versions_checked",
            Self::UpgradeCopied => "upgrade_copied",
            Self::UpgradeIntent => "upgrade_intent",
            Self::UpgradeMigrated => "upgrade_migrated",
            Self::UpgradeVerified => "upgrade_verified",
            Self::UpgradeCommitted => "upgrade_committed",
            Self::UpgradeRestoreFile => "upgrade_restore_file",
            Self::UpgradeRestored => "upgrade_restored",
        }
    }
}

/// Aborts a debug process at the selected environment-controlled boundary, once armed.
#[cfg(debug_assertions)]
pub fn hit(point: FaultPoint) {
    if let Some(stage) = CONTENT_HANDOFF.get()
        && matches!(
            point,
            FaultPoint::AfterCommit | FaultPoint::AfterJournalCommit
        )
    {
        // The existing content fault callbacks run on the same blocking
        // worker as the enclosing automation mutation.
        abort_if_armed(&format!("automation_{stage}_{}", point.name()));
    }
    abort_if_armed(point.name());
}

#[cfg(debug_assertions)]
fn abort_if_armed(name: &str) {
    use std::sync::OnceLock;
    static ARMED: OnceLock<Option<String>> = OnceLock::new();
    let armed = ARMED.get_or_init(|| std::env::var("CALTERNAL_FAULT").ok());
    if armed.as_deref() == Some(name) {
        eprintln!("calternal: injected fault at {name}; aborting");
        std::process::abort();
    }
}

/// Release-build no-op; environment variables cannot arm fault injection.
#[cfg(not(debug_assertions))]
#[inline(always)]
pub fn hit(_point: FaultPoint) {}

/// Inject a turn-side crash only for a native automation job.
pub(crate) fn automation_turn(input: &serde_json::Value, point: FaultPoint) {
    if input["thread_request"]["automation_run"].is_object() {
        hit(point);
    }
}

#[cfg(debug_assertions)]
std::thread_local! {
    static CONTENT_HANDOFF: std::cell::Cell<Option<&'static str>> = const { std::cell::Cell::new(None) };
}

/// Select the automation mutation for the existing content fault boundaries.
pub(crate) fn automation_content<T>(stage: &'static str, run: impl FnOnce() -> T) -> T {
    #[cfg(debug_assertions)]
    {
        struct Reset(Option<&'static str>);
        impl Drop for Reset {
            fn drop(&mut self) {
                CONTENT_HANDOFF.set(self.0);
            }
        }
        let _reset = Reset(CONTENT_HANDOFF.replace(Some(stage)));
        run()
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = stage;
        run()
    }
}
