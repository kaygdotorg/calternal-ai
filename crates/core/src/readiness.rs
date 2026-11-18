//! Wire types for a person's machine and provider readiness.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// Read-only operation for machine and provider status.
pub const AGENT_READINESS: &str = "agent.readiness";

/// Read first-owner onboarding status for an instance machine.
pub const MACHINES_ONBOARDING: &str = "machines.onboarding";

/// Issue a script for enrolling a machine owned by the signed-in account.
pub const MACHINES_INSTALL: &str = "machines.install";

/// The enrollment script and the deadline for redeeming its one-time code.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MachineInstallResult {
    /// Readable shell script containing the private enrollment code.
    pub script: String,
    /// Unix milliseconds when the code expires if unused.
    #[ts(type = "number")]
    pub expires_ms: i64,
}

/// No caller-controlled scope or target for machine onboarding and enrollment.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MachineOnboardingInput {}

/// Enrollment and current placement availability for first-owner setup.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MachineOnboardingResult {
    /// An active enrollment exists on this installation.
    pub attached: bool,
    /// The selected machine is connected and can accept a sandbox placement.
    pub ready: bool,
    /// The selected machine UUID for the owner's explicit verification.
    pub machine_id: Option<String>,
}

/// Run an owner-authorized diagnostic on one enrolled machine.
pub const MACHINES_VERIFY: &str = "machines.verify";

/// Select the space for the diagnostic job and the exact enrolled machine.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MachineVerifyInput {
    /// A space where the caller can use plugins.
    pub space_id: String,
    /// Enrolled machine UUID. Pool machines require server settings authority.
    pub machine_id: String,
    /// Optional provider plugin. An empty or absent value checks Claude Code.
    #[serde(default)]
    pub provider_id: String,
}

/// The bounded result of one diagnostic. No job IDs or machine logs are returned.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct MachineVerifyResult {
    /// Whether the machine confirmed that its execution environment started.
    pub sandbox_started: bool,
    /// The provider's check, absent if it did not report an outcome.
    pub provider: Option<ProviderCheck>,
}

/// Select the space and optionally a saved thread's placement constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadinessInput {
    /// Space UUID. The person needs the space's plugins.use capability.
    pub space_id: String,
    /// Saved thread UUID, if readiness should be restricted by a thread pin.
    #[serde(default)]
    pub thread_id: Option<String>,
}

/// A bounded observation of the official provider program's own check.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderCheck {
    /// The provider program was found inside the machine's execution environment.
    pub installed: bool,
    /// The program reported an active login for this account.
    pub signed_in: bool,
    /// The program's own sign-in command, for display only.
    pub sign_in_command: String,
}

/// A machine's readiness for one provider in this space.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProviderReadiness {
    /// The provider and orchestrator are enabled for this space.
    pub enabled: bool,
    /// Recent provider status, or null until a fresh check is available.
    pub check: Option<ProviderCheck>,
    /// The machine has capacity and the enabled provider reports a login.
    pub ready: bool,
}

/// The effective execution environment available on a machine.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum MachineExecution {
    /// The machine runs turns in its configured sandbox.
    Sandboxed,
    /// The account chose execution without a container.
    Direct,
}

/// An eligible personal machine or anonymous pool option.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS)]
pub struct MachineReadiness {
    /// The machine has an active enrollment record.
    pub enrolled: bool,
    /// The server has a current connection from the machine.
    pub connected: bool,
    /// The machine is accepting additional placements.
    pub available: bool,
    /// How this machine runs turns.
    pub execution: MachineExecution,
    /// The cloud machine is using containers because KVM is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_fallback: Option<SandboxFallback>,
    /// Installed provider plugins and their account-scoped check results.
    pub providers: BTreeMap<String, ProviderReadiness>,
}

/// Why a machine is running a reduced sandbox configuration.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema, TS,
)]
#[serde(rename_all = "snake_case")]
pub enum SandboxFallback {
    /// The current machine user cannot create KVM guests, so the role uses Podman.
    KvmUnavailable,
}

/// Readiness for a machine owned by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
pub struct PersonalMachineReadiness {
    /// Stable enrolled machine UUID.
    pub machine_id: String,
    /// The owner's machine label.
    pub name: String,
    /// Last observed contact in Unix milliseconds, when known.
    #[ts(type = "number | null")]
    pub last_seen_ms: Option<i64>,
    /// Connection, execution and provider readiness.
    #[serde(flatten)]
    pub readiness: MachineReadiness,
}

/// One cached readiness snapshot. Pool options carry no host identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ReadinessResult {
    /// Nonrevoked machines enrolled to the requesting account.
    pub machines: Vec<PersonalMachineReadiness>,
    /// Anonymous options from the entitled pool, if any.
    pub pool: Vec<MachineReadiness>,
    /// Lifetime of one provider observation in milliseconds.
    #[ts(type = "number")]
    pub check_ttl_ms: u64,
}
