//! Sidebar labels for the existing durable job lifecycle.
use calternal_core::JobState;

/// Project the durable job lifecycle to the thread UI label. Cost: O(1).
pub(super) fn label(state: JobState) -> &'static str {
    match state {
        JobState::Admitted | JobState::RetryWaiting => "queued",
        JobState::Running => "running",
        JobState::Waiting => "waiting",
        JobState::Interrupted => "interrupted",
        JobState::Failed | JobState::OutcomeUnknown => "failed",
        JobState::Done | JobState::Cancelled => "idle",
    }
}
