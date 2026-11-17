//! Manual provider turns, before the Threads file-storage step.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Thread permission mode; every change revokes the previous thread token.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Thread, workspace and originating app only; other scopes require approval.
    LeastAccess,
    /// Current space access, reviewed by rules; high-risk classes require approval.
    #[default]
    ApproveForMe,
    /// Current space access without approvals, independently of sandboxing.
    FullAccess,
}

/// Input union for manual-turn operations. Each operation rejects irrelevant fields.
/// UUIDs are full resource identifiers; model work never starts without a space check.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(default, deny_unknown_fields)]
pub struct AgentInput {
    /// Explicit retry of a failed workspace discard; never implied by status polling.
    pub retry_failed: bool,

    /// Owning space UUID, required for check/run, otherwise derived from the job.
    pub space_id: String,
    /// Machine UUID for owner-controlled execution settings.
    pub machine_id: String,
    /// Fenced lease UUID for explicit machine-level resumption.
    pub lease_id: String,
    /// Whether an account-owned machine runs directly; pool machines refuse this.
    pub direct: bool,
    /// Thread UUID for check/run; no thread file is created in step one.
    pub thread_id: String,
    /// Job UUID for events/stop/resume/mode/access operations.
    pub job_id: String,
    /// Person's prompt; at most 16 KiB, required for run.
    pub prompt: String,
    /// Labelled context contributions, bounded by the provider command budget.
    pub context: Vec<crate::ProviderContext>,
    /// Official CLI model; empty uses `sonnet`.
    pub model: String,
    /// Options declared by the selected provider, bounded by the command budget.
    pub options: crate::JsonObject,
    /// Enabled provider plugin; empty selects calternal-claude-code.
    pub provider_id: String,
    /// Permission mode; defaults to Approve for me.
    pub mode: AgentMode,
    /// Per-thread recovery override; absent uses the account defaults.
    pub retry_settings: Option<AgentRetrySettings>,
    /// Originating app operation prefix; empty grants no originating-app access.
    pub source_app: String,
    /// Requested app prefix for an access request.
    pub app: String,
    /// Whether the requested grant allows mutations as well as reads.
    pub write: bool,
    /// Approval UUID for a freshly verified person to approve.
    pub approval_id: String,
    /// Stream epoch last seen, empty on first read.
    pub epoch: String,
    /// Next job sequence requested; zero on first read.
    #[ts(type = "number")]
    pub after: u64,
}

/// Manual provider operations. This table feeds discovery, policy and instruction indexes.
pub const AGENT_OPERATIONS: &[(&str, &str, bool)] = &[
    (
        "agent.settings",
        "Read or set your account defaults or thread retry settings",
        true,
    ),
    (
        "machines.resume",
        "Explicitly resume your interrupted machine lease",
        true,
    ),
    (
        "machines.set",
        "Set direct or sandboxed execution on your own machine",
        true,
    ),
    (
        "agent.check",
        "Check the selected provider installation and sign-in in your sandbox",
        true,
    ),
    (
        "agent.run",
        "Send a prompt to the selected provider in a machine sandbox",
        true,
    ),
    (
        "agent.events",
        "Read ordered live turn events and the durable checkpoint",
        false,
    ),
    (
        "agent.discard",
        "Explicitly discard a stopped turn workspace, retaining its account home",
        true,
    ),
    (
        "agent.stop",
        "Cancel your turn, or a turn you can manage",
        true,
    ),
    (
        "agent.resume",
        "Explicitly resume an interrupted turn from its checkpoint",
        true,
    ),
    (
        "agent.mode",
        "Change a thread permission mode and rotate its token",
        true,
    ),
    (
        "access.request",
        "Ask for a named app scope in this thread",
        true,
    ),
    (
        "access.approve",
        "Approve a scope with a freshly verified person session",
        true,
    ),
];

/// Declared risk class, evaluated in addition to capabilities and token scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum OperationRisk {
    /// Reads data without changing durable state.
    Read,
    /// Changes that ordinary application history can undo.
    Reversible,
    /// Effects that cannot reliably be undone; reviewer seam may advise here only.
    Irreversible,
    /// Communicates or publishes outside the space; approval is scoped.
    External,
    /// Expands authority or changes access; always requires a person.
    WidensAccess,
}

/// Risk for a built-in operation; plugin risks come directly from their manifests.
/// This classification feeds discovery and the agent reviewer, never substitutes
/// for current capabilities. O(operation-name bytes), no I/O.
pub fn builtin_operation_risk(id: &str, mutates: bool) -> OperationRisk {
    match id {
        "spaces.set_role" | "spaces.transfer" | "plugins.enable" | "agent.mode"
        | "access.approve" | "access.request" | "machines.set" => OperationRisk::WidensAccess,
        "agent.discard"
        | "spaces.delete"
        | "spaces.remove_member"
        | "spaces.leave"
        | "secrets.revoke" => OperationRisk::Irreversible,
        _ if !mutates => OperationRisk::Read,
        _ => OperationRisk::Reversible,
    }
}

/// Typed automatic provider recovery settings; manifest defaults apply per account
/// and a manual turn may override them. Neither permits machine re-placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AgentRetrySettings {
    /// Retry explicit transient provider errors, with no pending tool effect.
    pub provider_errors: bool,
    /// Resume subscription usage limits at the reset reported by the provider.
    pub subscription_limits: bool,
}
