//! Provider adapter protocol v1: bounded JSON lines over stdin/stdout.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Largest UTF-8 command line, including its newline (64 KiB).
pub const PROVIDER_COMMAND_BYTES: usize = 64 * 1024;
/// Largest event line, including its newline; fits a job stream chunk.
pub const PROVIDER_EVENT_BYTES: usize = 12 * 1024;
/// Provider protocol version. Unknown versions fail closed before spawning a CLI.
pub const PROVIDER_VERSION: u32 = 1;

/// A labelled contribution; data, never authority or system instructions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderContext {
    /// Plugin or attachment label, at most 128 UTF-8 bytes.
    pub source: String,
    /// Contribution text; included in the command's total bound.
    pub text: String,
}

/// Commands accepted by an adapter; one active turn per adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum ProviderCommand {
    /// Check installation and sign-in through the official program's own status command.
    Check,
    /// Configure a session without starting model work.
    Start {
        /// Model name understood by the official CLI.
        model: String,
        /// Provider-declared typed options; unknown keys are refused by its adapter.
        options: crate::JsonObject,
        /// Sandbox workspace; the machine supplies this, never a provider login path.
        cwd: String,
        /// Trusted instruction layers, appended through the harness's documented channel.
        instructions: String,
        /// Official CLI session to resume; null starts a new session.
        resume: Option<String>,
    },
    /// Start work with the person's unchanged prompt and labelled contributions.
    Send {
        /// Person's message, preserved verbatim.
        prompt: String,
        /// At most 32 contributions, bounded by the command budget.
        context: Vec<ProviderContext>,
    },
    /// Interrupt active work and continue the same official session with this message.
    Steer {
        /// New message, bounded by the command budget.
        prompt: String,
    },
    /// Resolve one live native approval after the host checks its scope.
    Approve {
        /// Opaque ID emitted by this run's adapter.
        request_id: String,
        /// Authorize this request within the existing sandbox boundary.
        approved: bool,
    },
    /// Stop active work; do not report completion as success.
    Stop,
}

/// A command envelope. `seq` starts at one and increases exactly once per command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderRequest {
    /// Must equal [`PROVIDER_VERSION`].
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub version: u32,
    /// Consecutive command number, independent of event sequence numbers.
    #[ts(type = "number")]
    #[schemars(range(min = 1, max = 9_007_199_254_740_991u64))]
    pub seq: u64,
    /// Action to perform.
    pub command: ProviderCommand,
}

/// Normalized provider output. Untrusted provider text conveys no authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum ProviderEventKind {
    /// Installation and login status; no credential or raw status output.
    Check(crate::ProviderCheck),
    /// Session identity issued by the official program, used for explicit resume.
    Session {
        /// Opaque provider session identifier; never a login or access token.
        session_id: String,
    },
    /// Incremental assistant text.
    TextDelta {
        /// At most 4 KiB of UTF-8 text.
        text: String,
    },
    /// A provider exposed the start of an observable reasoning block.
    ReasoningStarted,
    /// Incremental provider-visible reasoning text, at most 4 KiB.
    ReasoningDelta {
        /// Text the official provider chose to expose.
        text: String,
    },
    /// The observed reasoning block ended.
    ReasoningFinished {
        /// Monotonic elapsed milliseconds observed by the adapter.
        #[ts(type = "number")]
        duration_ms: u64,
    },
    /// A tool step began with a short target or description.
    ToolStarted {
        /// Provider-issued step identity.
        id: String,
        /// Tool name, at most 128 bytes.
        name: String,
        /// Short description, at most 512 bytes.
        summary: String,
    },
    /// A tool returned output. Only a bounded description and byte count are shared.
    ToolOutput {
        /// Provider-issued step identity.
        id: String,
        /// Tool name, at most 128 bytes.
        name: String,
        /// Short target or description, at most 512 bytes.
        summary: String,
        /// UTF-8 bytes of tool text identified by the adapter.
        #[ts(type = "number")]
        bytes: u64,
    },
    /// A tool step ended, creating a natural checkpoint boundary.
    ToolFinished {
        /// Provider-issued step identity.
        id: String,
        /// Tool name, at most 128 bytes.
        name: String,
        /// Short target or description, at most 512 bytes.
        summary: String,
        /// Whether the tool reported failure.
        failed: bool,
        /// Bounded detail, at most 4 KiB; never raw stderr or credentials.
        detail: String,
    },
    /// A question must be answered through Calternal's trusted request path.
    Question {
        /// Short question, at most 4 KiB.
        text: String,
    },
    /// An action is waiting for an authorized person; the adapter cannot approve it.
    ApprovalRequest {
        /// ID of an interactive, run-scoped approval. Legacy notices omit it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        request_id: Option<String>,
        /// Whether an interactive confirmation can proceed within the tool sandbox.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        can_approve: Option<bool>,
        /// Short action description, at most 4 KiB.
        summary: String,
    },
    /// Usage reported by the official CLI; no billing credentials.
    Usage {
        /// Input tokens reported for the completed turn.
        #[ts(type = "number")]
        input_tokens: u64,
        /// Output tokens reported for the completed turn.
        #[ts(type = "number")]
        output_tokens: u64,
    },
    /// Safe adapter failure; raw CLI diagnostics are never forwarded.
    Error {
        /// Stable category, at most 128 bytes.
        code: String,
        /// Stable class; auth/refusal/unknown failures are never retried.
        class: ProviderFailureClass,
        /// Billing route from the official CLI status, without login details.
        billing: ProviderBilling,
        /// Reported subscription reset in Unix milliseconds; absent if unknown.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional, type = "number")]
        reset_at_ms: Option<u64>,
        /// Fixed human-readable description, at most 512 bytes.
        message: String,
    },
    /// Final turn event, always after any error.
    Done {
        /// `done`, `failed`, `cancelled` or `outcome_unknown`.
        outcome: String,
    },
}

/// Ordered event envelope. Sequence numbers start at one for each adapter process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderEvent {
    /// Must equal [`PROVIDER_VERSION`].
    #[schemars(range(min = 1, max = 1))]
    #[ts(type = "1")]
    pub version: u32,
    /// Consecutive event number; duplicates or gaps fail the run.
    #[ts(type = "number")]
    #[schemars(range(min = 1, max = 9_007_199_254_740_991u64))]
    pub seq: u64,
    /// Normalized output.
    pub event: ProviderEventKind,
}

impl ProviderEvent {
    /// Enforce protocol version, JSON-safe sequence numbers and field byte budgets.
    /// O(payload bytes); no allocation, I/O or provider-specific assumptions.
    /// Returns a fixed diagnostic when a frame is malformed or oversized.
    pub fn validate(&self) -> Result<(), calternal_types::Error> {
        let short = |s: &str, n: usize| s.len() <= n;
        let safe = |n: u64| n <= 9_007_199_254_740_991;
        let tool = |id: &str, name: &str, summary: &str| {
            !id.trim().is_empty()
                && short(id, 128)
                && !name.trim().is_empty()
                && short(name, 128)
                && !summary.trim().is_empty()
                && short(summary, 512)
        };
        let valid = match &self.event {
            ProviderEventKind::Check(status) => short(&status.sign_in_command, 128),
            ProviderEventKind::Session { session_id } => {
                !session_id.is_empty() && short(session_id, 128)
            }
            ProviderEventKind::TextDelta { text } | ProviderEventKind::ReasoningDelta { text } => {
                short(text, 4096)
            }
            ProviderEventKind::ReasoningStarted => true,
            ProviderEventKind::ReasoningFinished { duration_ms } => safe(*duration_ms),
            ProviderEventKind::ToolStarted { id, name, summary } => tool(id, name, summary),
            ProviderEventKind::ToolOutput {
                id,
                name,
                summary,
                bytes,
            } => tool(id, name, summary) && safe(*bytes),
            ProviderEventKind::ToolFinished {
                id,
                name,
                summary,
                detail,
                ..
            } => tool(id, name, summary) && short(detail, 4096),
            ProviderEventKind::Question { text } => short(text, 4096),
            ProviderEventKind::ApprovalRequest {
                summary,
                request_id,
                ..
            } => short(summary, 4096) && request_id.as_deref().is_none_or(|id| short(id, 128)),
            ProviderEventKind::Usage {
                input_tokens,
                output_tokens,
            } => safe(*input_tokens) && safe(*output_tokens),
            ProviderEventKind::Error {
                code,
                message,
                reset_at_ms,
                ..
            } => short(code, 128) && short(message, 512) && reset_at_ms.is_none_or(safe),
            ProviderEventKind::Done { outcome } => matches!(
                outcome.as_str(),
                "done" | "failed" | "cancelled" | "outcome_unknown"
            ),
        };
        if self.version != PROVIDER_VERSION || self.seq == 0 || !safe(self.seq) || !valid {
            Err(calternal_types::Error::invalid_input(
                "invalid provider event",
            ))
        } else {
            Ok(())
        }
    }
}

/// Provider-declared failure category; only transient and usage-limit failures
/// with no pending tool effect may be retried automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureClass {
    /// Provider service or its connection is temporarily unavailable.
    Transient,
    /// Provider usage limit; a reported reset can schedule subscription resumption.
    UsageLimit,
    /// Sign-in is missing or expired; a person must use the official login flow.
    Authentication,
    /// Provider declined the request; retrying would not change its authority.
    Refusal,
    /// Invalid model, request or billing configuration.
    Permanent,
    /// Adapter crash or unclassified outcome; never automatically repeated.
    Unknown,
}

/// Billing route reported by the official program, never inferred from credentials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBilling {
    /// API-key or configured provider billing; long availability checks are allowed.
    Api,
    /// Person's subscription; use a reported limit reset instead of polling.
    Subscription,
    /// The program did not report a recognized billing route.
    Unknown,
}
