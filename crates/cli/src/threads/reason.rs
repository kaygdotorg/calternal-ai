//! Human wording for the thread API's stable reason codes.
use crate::output::{Output, Reply};
use serde_json::Value;

/// Short reason for the adaptive list column, without loading turn details.
pub(super) fn summary(reason: &Value) -> Option<&'static str> {
    Some(match reason["code"].as_str()? {
        "no_machine" => "No machine enrolled",
        "machine_offline" => "Machine offline",
        "waiting_for_capacity" => "Waiting for capacity",
        "provider_signed_out" => "Provider signed out",
        "waiting_on_person" => "Waiting for a person",
        "retrying" => "Retry scheduled",
        "thread_full" => "Thread size limit reached",
        "failed" => "Turn failed",
        "interrupted" => "Turn interrupted",
        _ => return None,
    })
}

/// Present the details that explain how to proceed without parsing error text.
/// Cost is constant in the size of a thread or provider log.
pub(super) fn details(out: &Output, mut reply: Reply, reason: &Value) -> Reply {
    if let Some(summary) = summary(reason) {
        reply = reply.fields([("Reason", summary)]);
    }
    for (label, key) in [
        ("Provider", "provider"),
        ("Sign-in command", "sign_in_command"),
        ("Request", "request_id"),
    ] {
        if let Some(value) = reason[key].as_str() {
            reply = reply.fields([(label, value)]);
        }
    }
    if let Some(message) = reason["message"].as_str() {
        reply = reply.fields([("Details", message)]);
    }
    if reason["code"] == "interrupted"
        && let Some(why) = reason["why"].as_str()
    {
        let message = match why {
            "machine_offline" => "The machine stopped responding.",
            "server_restarted" => "The server restarted.",
            "retry_disabled" => "Automatic retry was disabled.",
            "access_changed" => "Turn access changed.",
            "permissions_changed" => "The thread permission mode changed.",
            "placement_timeout" => "The wait for a machine expired.",
            "execution_timeout" => "The turn time limit expired.",
            "outcome_unknown" => "The provider exited without a definite outcome.",
            _ => "The turn stopped at its saved checkpoint.",
        };
        reply = reply.fields([("Interruption", message)]);
    }
    if let Some(resumable) = reason["resumable"].as_bool() {
        reply = reply.fields([("Can resume", if resumable { "Yes" } else { "No" })]);
    }
    if let Some(class) = reason["class"].as_str() {
        let label = match class {
            "transient" => "Temporary provider error",
            "usage_limit" => "Provider usage limit",
            _ => "Permanent failure",
        };
        reply = reply.fields([("Failure", label)]);
    }
    if let Some(rule) = reason["rule"].as_str() {
        let label = match rule {
            "subscription_reset" => "Wait for subscription reset",
            "api_availability" => "Check API provider every three hours",
            _ => "Retry after a short wait",
        };
        reply = reply.fields([("Retry rule", label)]);
    }
    for (label, key) in [("Attempt", "attempt"), ("Queue position", "queue_position")] {
        if let Some(value) = reason[key].as_u64() {
            reply = reply.fields([(label, value.to_string())]);
        }
    }
    for (label, key) in [
        ("Last contact", "since_ms"),
        ("Next attempt", "next_attempt_ms"),
        ("Provider reset", "reset_at_ms"),
    ] {
        if let Some(value) = reason[key].as_i64() {
            reply = reply.fields([(label, out.local_ms(value))]);
        }
    }
    reply
}
