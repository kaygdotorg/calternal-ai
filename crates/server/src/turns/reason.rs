//! The compact checkpoint projection shared by the thread index, detail and journal.
use calternal_core::JobState;
use serde_json::{Value, json};

const FIELDS: &[&str] = &[
    "placement",
    "error",
    "retry",
    "person_request",
    "interruption",
];

/// Copy only reason fields into the durable journal outbox and indexed list.
/// The expression is built when the schema or prepared statement is opened.
pub(crate) fn sql(prefix: &str) -> String {
    let fields = FIELDS
        .iter()
        .map(|field| format!("'{field}',json_extract({prefix}checkpoint,'$.{field}')"))
        .collect::<Vec<_>>()
        .join(",");
    format!("json_object({fields})")
}

/// Explain a turn from its existing checkpoint. No I/O or error-message parsing.
/// Shared-space viewers receive the state without the owner's private details.
pub(crate) fn describe(state: Option<JobState>, c: &Value, private: bool) -> Value {
    use JobState::*;
    let reason = match state {
        None => json!({"code":"idle"}),
        Some(Admitted) if !c["placement"].is_null() => c["placement"].clone(),
        Some(Admitted) => json!({"code":"queued"}),
        Some(Running) => Value::Null,
        Some(Waiting) => json!({"code":"waiting_on_person","request_id":c["person_request"]}),
        Some(RetryWaiting) => {
            let retry = &c["retry"];
            let rule = if retry["reason"] == "subscription_limit" {
                "subscription_reset"
            } else if retry["retry_count"].as_u64().unwrap_or(0)
                > super::retry::RETRY_SECONDS.len() as u64
            {
                "api_availability"
            } else {
                "provider_backoff"
            };
            json!({"code":"retrying","attempt":retry["retry_count"],"next_attempt_ms":retry["next_attempt_ms"],"rule":rule})
        }
        Some(Interrupted) => json!({
            "code":"interrupted",
            "resumable":true,
            "why":c["interruption"]["why"].as_str().unwrap_or("explicit_resume_required"),
            "since_ms":c["interruption"]["since_ms"],
        }),
        Some(Failed) if c["error"]["class"] == "authentication" => json!({
            "code":"provider_signed_out",
            "provider":c["error"]["provider"],
            "sign_in_command":c["error"]["sign_in_command"],
        }),
        Some(Failed) => {
            let class = match c["error"]["class"].as_str() {
                Some("transient") => "transient",
                Some("usage_limit") => "usage_limit",
                _ => "permanent",
            };
            json!({"code":"failed","class":class,"reset_at_ms":c["error"]["reset_at_ms"]})
        }
        Some(OutcomeUnknown) => {
            json!({"code":"interrupted","resumable":false,"why":"outcome_unknown"})
        }
        Some(Done) => json!({"code":"completed"}),
        Some(Cancelled) => json!({"code":"cancelled"}),
    };
    if private { reason } else { public(reason) }
}

/// Replace a private provider authentication event with its generic peer view.
/// Preserve stream sequence numbers and the provider event envelope.
pub(crate) fn public_event(mut data: Value) -> Value {
    if data["event"]["type"] == "error" && data["event"]["class"] == "authentication" {
        data["event"] = json!({
            "type":"error","class":"permanent","billing":"unknown",
            "code":"provider_failed","message":"The provider could not finish this turn."
        });
    }
    data
}

/// Strip account-private machine, provider login and person-request details.
pub(crate) fn public(mut reason: Value) -> Value {
    match reason["code"].as_str() {
        Some("provider_signed_out") => return json!({"code":"failed","class":"permanent"}),
        Some("machine_offline" | "no_machine") => return json!({"code":"queued"}),
        _ => {}
    }
    if reason["why"] == "machine_offline" {
        reason["why"] = json!("explicit_resume_required");
    }
    if let Some(fields) = reason.as_object_mut() {
        for name in ["request_id", "sign_in_command", "provider", "since_ms"] {
            fields.remove(name);
        }
    }
    reason
}
