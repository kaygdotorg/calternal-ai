//! Non-interactive commands for the signed-in person's pending requests.
use crate::output::{CliError, Reply};
use clap::Subcommand;
use serde_json::{Value, json};

/// Read or resolve one account's requests.
#[derive(Subcommand)]
pub enum Command {
    /// List the requests waiting on you.
    List,
    /// Answer a question with a JSON object of form field values.
    Answer {
        /// Request UUID from the list.
        id: String,
        /// Submitted form values as JSON.
        #[arg(long)]
        answer: String,
    },
    /// Approve using an installation session with a fresh passkey proof.
    Approve {
        /// Request UUID from the list.
        id: String,
        /// How long to allow the action.
        #[arg(long, value_parser = ["once", "this-thread"])]
        lifetime: String,
    },
    /// Decline a pending approval.
    Deny {
        /// Request UUID from the list.
        id: String,
    },
}

impl Command {
    /// Convert command arguments to the shared operation envelope. Parses answer
    /// JSON once and returns a usage error for non-object input.
    pub fn operation(self) -> Result<(&'static str, Value), CliError> {
        Ok(match self {
            Self::List => ("requests.list", json!({})),
            Self::Answer { id, answer } => {
                let answer: calternal_core::JsonObject = serde_json::from_str(&answer)
                    .map_err(|_| CliError::usage("--answer must be a JSON object"))?;
                ("requests.answer", json!({"id":id,"answer":answer}))
            }
            Self::Approve { id, lifetime } => {
                let lifetime: calternal_core::ApprovalLifetime =
                    serde_json::from_value(json!(lifetime.replace('-', "_")))
                        .map_err(|_| CliError::usage("choose once or this-thread"))?;
                ("requests.approve", json!({"id":id,"lifetime":lifetime}))
            }
            Self::Deny { id } => ("requests.deny", json!({"id":id})),
        })
    }
}

/// Render the bounded request list or decision using the shared terminal and
/// machine-readable output. Cost: one pass over the returned rows.
pub fn reply(data: Value, rid: &str) -> Reply {
    let reply = Reply::new(data.clone()).request_id(rid);
    if let Some(items) = data["requests"].as_array() {
        let rows = items
            .iter()
            .map(|item| {
                let detail = &item["detail"];
                let kind = detail["kind"].as_str().unwrap_or("");
                let summary = match kind {
                    "approval" => detail["action"]["operation"]
                        .as_str()
                        .or_else(|| detail["app"].as_str()),
                    "secret" => detail["label"].as_str(),
                    "question" => detail["label"].as_str(),
                    _ => None,
                };
                vec![
                    kind.to_owned(),
                    item["id"].as_str().unwrap_or("").to_owned(),
                    item["space_id"].as_str().unwrap_or("Personal").to_owned(),
                    summary.unwrap_or("").to_owned(),
                    detail["lifetimes"]
                        .as_array()
                        .map_or_else(String::new, |choices| {
                            choices
                                .iter()
                                .filter_map(Value::as_str)
                                .map(|value| value.replace('_', "-"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }),
                ]
            })
            .collect();
        reply
            .table(
                &["KIND", "ID", "SPACE", "DETAIL", "LIFETIMES"],
                rows,
                "No requests waiting on you.",
            )
            .hint("Next: calternal requests --help")
    } else {
        reply
            .table(
                &["ID", "STATUS"],
                vec![vec![
                    data["id"].as_str().unwrap_or("").to_owned(),
                    data["status"].as_str().unwrap_or("").to_owned(),
                ]],
                "No request resolved.",
            )
            .hint("Next: calternal requests list")
    }
}
