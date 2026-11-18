//! Wire contracts for plugin jobs: durable host records of long-running plugin work.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Lifecycle of a job. `admitted`, `running` and `waiting` are active; the rest are
/// final and never change again.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Durably recorded and queued for a run.
    Admitted,
    /// A run invocation is executing in the plugin's worker.
    Running,
    /// Parked without a process until its child jobs report, a signal arrives or a deadline passes.
    Waiting,
    /// Machine or server went silent; only explicit resume may restart this turn.
    Interrupted,
    /// No provider process is held; checkpoint.retry records the durable next attempt.
    RetryWaiting,
    /// Finished with a result.
    Done,
    /// Finished without a result.
    Failed,
    /// Cancelled; later output from its run is discarded.
    Cancelled,
    /// Interrupted by a server restart while its effects could not be safely repeated.
    OutcomeUnknown,
}

impl JobState {
    /// Whether the state is final. Cost: O(1).
    pub fn is_final(self) -> bool {
        matches!(
            self,
            Self::Done | Self::Failed | Self::Cancelled | Self::OutcomeUnknown
        )
    }
}

/// Input for `jobs.get` and `jobs.cancel`. Possessing a job ID grants nothing: a job
/// in a space the caller cannot read answers exactly like a missing one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct JobInput {
    /// Host-issued job UUID.
    pub job_id: String,
}

/// Public summary of a parked plugin job with no signal payload or credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct JobWait {
    /// Why the job is waiting, supplied by the worker at the checkpoint.
    pub reason: String,
    /// Unix milliseconds when the wait ends if no signal arrives.
    #[ts(type = "number")]
    pub deadline_ms: i64,
}

/// What a client may see of a job: its durable record, never its stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct JobView {
    /// Host-issued job UUID.
    pub id: String,
    /// Plugin that runs the job.
    pub plugin_id: String,
    /// Space UUID that owns the job.
    pub space_id: String,
    /// Current lifecycle state.
    pub state: JobState,
    /// Parent job UUID for a child job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parent_id: Option<String>,
    /// Public summary while a plugin job waits for a signal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub wait: Option<JobWait>,
    /// Last durable checkpoint written by the plugin, or null.
    pub checkpoint: serde_json::Value,
    /// Stream sequence number of the first chunk after that checkpoint.
    pub checkpoint_seq: u64,
    /// Final result, a caller-safe wait rejection, or null.
    pub result: serde_json::Value,
}

/// Result for `jobs.get` and `jobs.cancel`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct JobResult {
    /// The job after the operation.
    pub job: JobView,
}
