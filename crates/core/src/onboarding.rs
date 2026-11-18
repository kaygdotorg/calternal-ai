//! Wire models for the signed-in person's first-run completion state.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Read whether the signed-in account has completed onboarding.
pub const ONBOARDING_STATE: &str = "onboarding.state";
/// Record that the signed-in account has completed onboarding.
pub const ONBOARDING_FINISH: &str = "onboarding.finish";

/// Both operations derive their account from the authenticated session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OnboardingInput {}

/// Durable completion status. Accounts without a completion record start unfinished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct OnboardingState {
    /// True once this account has finished onboarding.
    pub finished: bool,
}
