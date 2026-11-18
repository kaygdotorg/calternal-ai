//! Saved thread commands use the same manifest operations as every client.
mod reason;

use crate::{
    agent::{self, Connection},
    output::{CliError, Output, Reply},
};
use clap::{Args as ClapArgs, Subcommand};
use serde_json::{Value, json};

/// Non-interactive conversation command and connection options.
#[derive(Debug, ClapArgs)]
pub struct Args {
    /// Server and private credential selection, plus a stable retry identity.
    #[command(flatten)]
    pub connection: Connection,
    /// Requested operation; use --help for arguments.
    #[command(subcommand)]
    pub command: Command,
}
/// Conversation lifecycle; identifiers are full UUIDs independent of titles.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Start an empty saved conversation in a space.
    Create {
        /// Owning space UUID.
        #[arg(long)]
        space_id: String,
        /// Human-readable title, at most 256 UTF-8 bytes.
        #[arg(long)]
        title: String,
        /// Enabled provider plugin.
        #[arg(long, default_value = "calternal-claude-code")]
        provider: String,
        /// least_access, approve_for_me, or full_access.
        #[arg(long, default_value = "approve_for_me")]
        mode: String,
        /// Parent thread UUID; archived children remain discoverable here.
        #[arg(long)]
        parent_thread: Option<String>,
    },
    /// List conversation metadata without reading their message bodies.
    List {
        /// Owning space UUID.
        #[arg(long)]
        space_id: String,
        /// Include archived conversations.
        #[arg(long)]
        archived: bool,
        /// List all children, including archived children, of this parent UUID.
        #[arg(long)]
        parent_thread: Option<String>,
        /// Maximum rows, from 1 to 1000.
        #[arg(long, default_value_t = 100)]
        limit: u32,
        /// Continuation from the previous page, or - to read it from stdin.
        #[arg(long, value_parser = crate::parse_cursor, allow_hyphen_values = true)]
        cursor: Option<String>,
    },
    /// List installed models and accepted reasoning levels in a space.
    Models {
        /// Owning space UUID.
        #[arg(long)]
        space_id: String,
    },
    /// Read the Markdown conversation or a saved checkpoint addition.
    Get {
        /// Thread UUID.
        id: String,
        /// JSON data from a threads.checkpoint journal entry.
        #[arg(long, value_parser = |text: &str| text.parse::<Value>())]
        since: Option<Value>,
    },
    /// Save your message and admit a provider turn; reuse request-id after failure.
    Send {
        /// Thread UUID.
        id: String,
        /// Message as typed, at most 16 KiB.
        #[arg(long)]
        message: String,
        /// Provider change for this and later sends.
        #[arg(long)]
        provider: Option<String>,
        /// Model for this turn; omitted uses provider default.
        #[arg(long)]
        model: Option<String>,
        /// Explicit reasoning level for this turn. Omit to use the provider's settings.
        #[arg(long)]
        reasoning: Option<String>,
        /// Mode for this and later sends; applies directly without another passkey check.
        #[arg(long)]
        mode: Option<String>,
    },
    /// Cancel the thread's current turn, preserving its files and checkpoints.
    Stop {
        /// Thread UUID.
        id: String,
    },
    /// Archive a conversation; it remains addressable and visible under its parent.
    Archive {
        /// Thread UUID.
        id: String,
    },
    /// Give a conversation a new title and keep its ID and messages.
    Rename {
        /// Thread UUID.
        id: String,
        /// Human-readable title, at most 256 UTF-8 bytes.
        #[arg(long)]
        title: String,
    },
    /// Return a conversation to the ordinary list.
    Unarchive {
        /// Thread UUID.
        id: String,
    },
    /// Move a stopped conversation to its space's Trash and forget saved modes.
    Delete {
        /// Thread UUID.
        id: String,
    },
}
impl Command {
    fn operation(self) -> (&'static str, Value) {
        match self {
            Self::Create {
                space_id,
                title,
                provider,
                mode,
                parent_thread,
            } => {
                let mut v =
                    json!({"space_id":space_id,"title":title,"provider":provider,"mode":mode});
                if let Some(parent) = parent_thread {
                    v["parent_thread"] = json!(parent);
                }
                ("threads.create", v)
            }
            Self::List {
                space_id,
                archived,
                parent_thread,
                limit,
                cursor,
            } => {
                let mut v = json!({"space_id":space_id,"archived":archived,"limit":limit});
                if let Some(parent) = parent_thread {
                    v["parent_thread"] = json!(parent);
                }
                if let Some(cursor) = cursor {
                    v["cursor"] = json!(cursor);
                }
                ("threads.list", v)
            }
            Self::Models { space_id } => ("threads.models", json!({"space_id":space_id})),
            Self::Get { id, since } => {
                let mut input = json!({"id":id});
                if let Some(since) = since {
                    input["since"] = since;
                }
                ("threads.get", input)
            }
            Self::Stop { id } => ("threads.stop", json!({"id":id})),
            Self::Archive { id } => ("threads.archive", json!({"id":id})),
            Self::Rename { id, title } => ("threads.rename", json!({"id":id,"title":title})),
            Self::Unarchive { id } => ("threads.unarchive", json!({"id":id})),
            Self::Delete { id } => ("threads.delete", json!({"id":id})),
            Self::Send {
                id,
                message,
                provider,
                model,
                reasoning,
                mode,
            } => {
                let mut v = json!({"id":id,"message":message});
                for (key, value) in [
                    ("provider", provider),
                    ("model", model),
                    ("reasoning", reasoning),
                    ("mode", mode),
                ] {
                    if let Some(value) = value {
                        v[key] = json!(value);
                    }
                }
                ("threads.send", v)
            }
        }
    }
}
fn lifecycle(metadata: &Value) -> &'static str {
    if metadata["deleted"] == true {
        "deleted"
    } else if metadata["archived"] == true {
        "archived"
    } else {
        "active"
    }
}

/// Render server thread data with shared width adaptation, stable JSON and hints.
/// Cost: O(returned rows or document bytes); no I/O. Unknown shapes fail visibly.
pub fn reply(out: &Output, data: Value) -> Reply {
    if let Some(providers) = data["providers"].as_array() {
        let mut result = Reply::new(data.clone());
        if providers.is_empty() {
            result = result.line("No providers are enabled in this space.");
        }
        for (index, provider) in providers.iter().enumerate() {
            if index != 0 {
                result = result.blank();
            }
            let rows = provider["models"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|model| {
                    let field = |key: &str| model[key].as_str().unwrap_or("").to_owned();
                    let levels = model["reasoning_levels"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(", ");
                    vec![
                        field("id"),
                        field("name"),
                        if levels.is_empty() {
                            "Provider default".into()
                        } else {
                            levels
                        },
                        if model["id"] == provider["default_model"] {
                            "Yes"
                        } else {
                            "No"
                        }
                        .into(),
                    ]
                })
                .collect();
            result = result
                .fields([
                    ("Provider", provider["name"].as_str().unwrap_or("")),
                    ("Provider ID", provider["id"].as_str().unwrap_or("")),
                ])
                .table(
                    &["MODEL ID", "MODEL", "REASONING", "DEFAULT"],
                    rows,
                    "No models declared.",
                );
        }
        return result.hint("Next: calternal threads send --help");
    }
    if let Some(rows) = data["threads"].as_array() {
        let rows = rows
            .iter()
            .map(|r| {
                let metadata = &r["metadata"];
                let field = |key: &str| metadata[key].as_str().unwrap_or("—").to_owned();
                vec![
                    field("title"),
                    r["turn_state"].as_str().unwrap_or("idle").into(),
                    reason::summary(&r["turn_reason"]).unwrap_or("—").into(),
                    lifecycle(metadata).into(),
                    metadata["message_count"].as_u64().unwrap_or(0).to_string(),
                    r["id"].as_str().unwrap_or("").into(),
                    field("provider"),
                    field("model"),
                    field("mode"),
                    field("last_activity_at"),
                    field("parent_thread"),
                ]
            })
            .collect();
        let has_next = data["next_cursor"].is_string();
        return Reply::new(data)
            .table(
                &[
                    "TITLE",
                    "TURN",
                    "TURN REASON",
                    "STATE",
                    "MESSAGES",
                    "ID",
                    "PROVIDER",
                    "MODEL",
                    "MODE",
                    "LAST ACTIVITY",
                    "PARENT",
                ],
                rows,
                "No threads in this space.",
            )
            .page_cursor()
            .hint(if has_next {
                "Next: repeat calternal threads list with the same filters and --cursor."
            } else {
                "Next: calternal threads create --help"
            });
    }
    if let Some(delta) = data.get("delta") {
        return Reply::new(data.clone())
            .fields([
                ("Thread", delta["id"].as_str().unwrap_or("").to_owned()),
                (
                    "Base revision",
                    delta["base_revision"].as_str().unwrap_or("").to_owned(),
                ),
                (
                    "Revision",
                    delta["revision"].as_str().unwrap_or("").to_owned(),
                ),
                (
                    "Body offset (bytes)",
                    delta["offset"].as_u64().unwrap_or(0).to_string(),
                ),
            ])
            .blank()
            .text(delta["markdown"].as_str().unwrap_or(""))
            .hint(
                "Next: use calternal threads get without --since to reload the full conversation.",
            );
    }
    let thread = &data["thread"];
    let fields = vec![
        ("Thread", thread["id"].as_str().unwrap_or("").to_owned()),
        (
            "Title",
            thread["metadata"]["title"]
                .as_str()
                .unwrap_or("")
                .to_owned(),
        ),
        (
            "Address",
            thread["address"].as_str().unwrap_or("").to_owned(),
        ),
        ("State", lifecycle(&thread["metadata"]).into()),
    ];
    let mut result = Reply::new(data.clone()).fields(fields);
    let metadata = &thread["metadata"];
    for (label, key) in [
        ("Provider", "provider"),
        ("Model", "model"),
        ("Permission mode", "mode"),
        ("Last activity", "last_activity_at"),
        ("Parent", "parent_thread"),
    ] {
        if let Some(value) = metadata[key].as_str() {
            result = result.fields([(label, value)]);
        }
    }
    if let Some(count) = metadata["message_count"].as_u64() {
        result = result.fields([("Messages", count.to_string())]);
    }
    if let Some(state) = thread["turn_state"].as_str() {
        result = result.fields([("Thread turn state", state)]);
    }
    result = reason::details(out, result, &thread["turn_reason"]);
    if let Some(job) = data.get("job") {
        result = result.fields([
            ("Turn", job["id"].as_str().unwrap_or("")),
            ("Turn state", job["state"].as_str().unwrap_or("")),
        ]);
    }
    if let Some(text) = thread["markdown"].as_str() {
        result = result.blank().text(text);
    }
    result.hint(
        "Next: calternal threads --help; retry an uncertain send with its original --request-id.",
    )
}
/// Execute one operation on the shared client and output surface. Credential reads
/// run on a blocking worker. JSON is a single document, including caller-safe errors.
pub fn main(args: Args, out: &Output) -> i32 {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("thread runtime");
    let result: Result<Reply, CliError> = runtime.block_on(async {
        let connection = args.connection;
        let settings = connection.clone();
        let client = tokio::task::spawn_blocking(move || agent::client(&settings))
            .await
            .map_err(|_| CliError::new("internal", "Credential loading failed."))??;
        let (operation, input) = args.command.operation();
        agent::call(&client, operation, input, connection.request_id)
            .await
            .map(|value| reply(out, value))
    });
    out.finish(result)
}
