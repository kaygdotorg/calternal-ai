//! Space-scoped diagnostics for installed packages.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Last observed state of an installed package in one space.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum PluginReadiness {
    /// Disabled in this space.
    Disabled,
    /// A manifest or bundle verification problem prevents execution.
    Unavailable,
    /// The enabled worker starts on its first invocation.
    NotStarted,
    /// Native package or a worker whose last start completed.
    Ready,
    /// The last startup or running worker failed.
    Failed,
}

/// One caller-safe runtime failure from this space.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct PluginRuntimeFailure {
    /// Operation attempted, when the failure belongs to an invocation.
    pub operation: Option<String>,
    /// Stable reason code without private input or output values.
    pub reason: String,
    /// When the host observed the failure, in Unix milliseconds.
    #[ts(type = "number")]
    pub at_ms: i64,
}

/// Installed-package diagnostics exposed to authorized space managers.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct PluginDiagnostics {
    /// Explicit space UUID.
    pub space_id: String,
    /// Installed package ID.
    pub plugin_id: String,
    /// Revision inspected by this read.
    pub revision: u32,
    /// Last observed readiness in this space.
    pub readiness: PluginReadiness,
    /// Caller-safe reason when the package is unavailable or failed.
    pub reason: Option<String>,
    /// Up to eight recent failures, newest first.
    pub recent_errors: Vec<PluginRuntimeFailure>,
    /// Latest 32 distinct Class B hostnames attempted in this space over seven days.
    pub fetch_hosts: Vec<String>,
    /// Up to 4 KiB from the last failed worker's own stderr in this space.
    pub stderr_tail: Option<String>,
}
