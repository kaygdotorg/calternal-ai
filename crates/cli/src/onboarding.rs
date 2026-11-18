//! Account onboarding commands for both CLI entry points.

use crate::output::Reply;
use calternal_core::{ONBOARDING_FINISH, ONBOARDING_STATE};
use clap::Subcommand;
use serde_json::Value;

/// Read or record completion for the signed-in account.
#[derive(Subcommand)]
pub enum Onboarding {
    /// Show whether your account has finished onboarding.
    State,
    /// Mark your account's onboarding finished.
    Finish,
}

impl Onboarding {
    /// Return the stable operation ID called with empty input.
    pub fn operation(self) -> &'static str {
        match self {
            Self::State => ONBOARDING_STATE,
            Self::Finish => ONBOARDING_FINISH,
        }
    }
}

/// Label completion in terminal mode while retaining the exact JSON response.
pub fn reply(data: Value, request_id: &str) -> Reply {
    let (status, hint) = match data["finished"].as_bool() {
        Some(true) => ("Finished", "Next: calternal describe"),
        Some(false) => (
            "Unfinished",
            "Next: calternal onboarding finish once setup is complete.",
        ),
        None => ("Unavailable", "Next: calternal onboarding state --json"),
    };
    Reply::new(data)
        .request_id(request_id)
        .fields([("Onboarding", status)])
        .hint(hint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn onboarding_status_has_the_documented_json_shape() {
        let out = crate::output::Output::fixed(true, None, time::UtcOffset::UTC);
        for (command, finished) in [("state", false), ("finish", true)] {
            let rendered = out.render(&reply(json!({"finished":finished}), "request"));
            let doc = serde_json::from_str(&rendered).unwrap();
            crate::testing::assert_shape(&format!("calternal onboarding {command}"), &doc);
        }
    }
}
