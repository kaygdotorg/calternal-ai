//! Non-interactive notes, tags and Trash commands over the shared operation client.
use crate::{
    agent::{self, Connection},
    output::{CliError, Output, Reply},
};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

/// Space selection for the Notes, tag and Trash indexes.
#[derive(Debug, Args)]
pub struct Space {
    /// UUID of the space to query.
    #[arg(long)]
    space: String,
    /// Fetch the next page of up to 100 rows. Pass - to read the cursor from stdin.
    #[arg(long, value_parser = crate::parse_cursor)]
    cursor: Option<String>,
}

impl Space {
    fn input(self) -> Value {
        let mut input = json!({"space_id":self.space});
        if let Some(cursor) = self.cursor {
            input["cursor"] = json!(cursor);
        }
        input
    }
}

/// Notes commands address items by their stable UUID.
#[derive(Debug, Subcommand)]
pub enum Notes {
    /// List active notes, optionally under a tag.
    #[command(
        after_help = "Example: calternal notes list --space SPACE --tag work\nSee also: calternal tags list"
    )]
    List {
        /// Owning space.
        #[command(flatten)]
        scope: Space,
        /// Exact tag name without a hash, including descendants.
        #[arg(long)]
        tag: Option<String>,
    },
    /// Find active notes by title prefix.
    #[command(
        after_help = "Example: calternal notes lookup --space SPACE Plan\nSee also: calternal notes list"
    )]
    Lookup {
        /// Owning space.
        #[command(flatten)]
        scope: Space,
        /// Literal title prefix.
        prefix: String,
    },
    /// Move a note to the space's Trash.
    #[command(
        after_help = "Example: calternal notes delete NOTE_UUID\nSee also: calternal notes restore, calternal trash list"
    )]
    Delete {
        /// Global note UUID.
        id: String,
    },
    /// Restore a note from the space's Trash.
    #[command(
        after_help = "Example: calternal notes restore NOTE_UUID\nSee also: calternal trash list"
    )]
    Restore {
        /// Global note UUID.
        id: String,
    },
    /// Change a note's heading and filename at its current revision.
    #[command(
        after_help = "Example: calternal notes rename NOTE_UUID --base-revision REV --title Plan\nSee also: calternal notes list"
    )]
    Rename {
        /// Global note UUID.
        id: String,
        /// Revision shown by the note list.
        #[arg(long)]
        base_revision: String,
        /// New title.
        #[arg(long)]
        title: String,
    },
}

impl Notes {
    /// Build a request for the same operation used by the other clients.
    pub fn operation(self) -> (&'static str, Value) {
        match self {
            Self::List { scope, tag } => {
                let mut input = scope.input();
                if let Some(tag) = tag {
                    input["tag"] = json!(tag);
                }
                ("notes.list", input)
            }
            Self::Lookup { scope, prefix } => {
                let mut input = scope.input();
                input["prefix"] = json!(prefix);
                ("notes.lookup", input)
            }
            Self::Delete { id } => ("notes.delete", json!({"id":id})),
            Self::Restore { id } => ("notes.restore", json!({"id":id})),
            Self::Rename {
                id,
                base_revision,
                title,
            } => (
                "notes.rename",
                json!({"id":id,"base_revision":base_revision,"title":title}),
            ),
        }
    }
}

/// Browse active tag names and their note counts.
#[derive(Debug, Subcommand)]
pub enum Tags {
    /// List the tags in a space.
    #[command(
        after_help = "Example: calternal tags list --space SPACE\nSee also: calternal tags lookup"
    )]
    List {
        /// Owning space.
        #[command(flatten)]
        scope: Space,
    },
    /// Find a literal, case-sensitive name prefix.
    #[command(
        after_help = "Example: calternal tags lookup --space SPACE work/\nSee also: calternal notes list"
    )]
    Lookup {
        /// Owning space.
        #[command(flatten)]
        scope: Space,
        /// Tag name prefix without a hash.
        prefix: String,
    },
}

impl Tags {
    /// Build a bounded index query.
    pub fn operation(self) -> (&'static str, Value) {
        match self {
            Self::List { scope } => ("tags.list", scope.input()),
            Self::Lookup { scope, prefix } => {
                let mut input = scope.input();
                input["prefix"] = json!(prefix);
                ("tags.lookup", input)
            }
        }
    }
}

/// Inspect and permanently remove items in a space's Trash.
#[derive(Debug, Subcommand)]
pub enum Trash {
    /// List trashed notes.
    #[command(
        after_help = "Example: calternal trash list --space SPACE\nSee also: calternal notes restore"
    )]
    List {
        /// Owning space.
        #[command(flatten)]
        scope: Space,
    },
    /// Permanently delete a trashed note and its versions.
    #[command(
        after_help = "Example: calternal trash purge NOTE_UUID\nSee also: calternal trash list, calternal notes restore"
    )]
    Purge {
        /// Global note UUID.
        id: String,
    },
}

impl Trash {
    /// Build the Trash operation request. Authority is enforced by the server.
    pub fn operation(self) -> (&'static str, Value) {
        match self {
            Self::List { scope } => ("trash.list", scope.input()),
            Self::Purge { id } => ("trash.purge", json!({"id":id})),
        }
    }
}

fn text<'a>(row: &'a Value, key: &str) -> &'a str {
    row[key].as_str().unwrap_or("")
}

/// Format Notes operation data for adaptive terminal and unchanged JSON delivery.
pub fn reply(out: &Output, op: &str, data: Value) -> Reply {
    let reply = Reply::new(data.clone());
    if let Some(tags) = data["tags"].as_array() {
        return reply
            .table(
                &["TAG", "NOTES"],
                tags.iter()
                    .map(|tag| vec![text(tag, "name").into(), tag["count"].to_string()])
                    .collect(),
                "No matching tags.",
            )
            .hint("Next: calternal notes list --space SPACE --tag TAG.")
            .page_cursor();
    }
    if let Some(notes) = data["notes"].as_array() {
        let time = if op == "trash.list" {
            "DELETED"
        } else {
            "UPDATED"
        };
        let hint = if op == "trash.list" {
            "Next: calternal notes restore ID."
        } else {
            "Next: calternal notes --help."
        };
        let mut columns = vec!["ID", "TITLE", time, "REVISION"];
        if op == "notes.lookup" {
            columns.push("LINK TARGET");
        }
        return reply
            .table(
                &columns,
                notes
                    .iter()
                    .map(|note| {
                        let mut row = vec![
                            text(note, "id").into(),
                            text(note, "title").into(),
                            out.local_rfc3339(text(note, "updated_at")),
                            text(note, "revision").into(),
                        ];
                        if op == "notes.lookup" {
                            row.push(text(note, "link_target").into());
                        }
                        row
                    })
                    .collect(),
                if op == "trash.list" {
                    "Trash is empty."
                } else {
                    "No matching notes."
                },
            )
            .hint(hint)
            .page_cursor();
    }
    if op == "trash.purge" {
        return reply
            .fields([("ID", text(&data, "id")), ("State", "Permanently deleted")])
            .hint("Next: calternal trash list --space SPACE.");
    }
    let note = &data["note"];
    let (state, hint) = match op {
        "notes.delete" => ("In Trash", "Next: calternal notes restore ID."),
        "notes.restore" => ("Restored", "Next: calternal notes list --space SPACE."),
        _ => ("Renamed", "Next: calternal notes list --space SPACE."),
    };
    let mut reply = reply
        .fields([
            ("ID", text(note, "id")),
            ("Space", text(note, "space_id")),
            ("Title", text(note, "title")),
            ("Revision", text(note, "revision")),
            ("State", state),
        ])
        .fields([("Updated", out.local_rfc3339(text(note, "updated_at")))]);
    if let Some(markdown) = note["markdown"].as_str() {
        reply = reply.blank().text(markdown);
    }
    reply.hint(hint)
}

/// Run one command using the common credential loader and typed operation gate.
pub fn run(connection: Connection, operation: &str, input: Value, out: &Output) -> i32 {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("operation runtime");
    out.finish(runtime.block_on(async {
        let selected = connection.clone();
        let client = tokio::task::spawn_blocking(move || agent::client(&selected))
            .await
            .map_err(|_| CliError::new("internal", "Credential loading failed."))??;
        let caller_id = connection.request_id;
        let pending = calternal_client::Client::pending(operation, &input, caller_id)
            .map_err(|e| failure(e, None))?;
        let rid = &pending.request().request_id;
        let value = client
            .execute_cancellable::<Value>(
                pending.request(),
                &calternal_client::CancellationToken::new(),
            )
            .await
            .map_err(|e| failure(e, Some(rid)))?;
        Ok(reply(out, operation, value).request_id(rid))
    }))
}

fn failure(e: calternal_client::ClientError, rid: Option<&str>) -> CliError {
    let api = e.api_error();
    let code = serde_json::to_value(api.code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "internal".into());
    let mut failure = CliError::new(code, api.message);
    if let Some(details) = api.details {
        failure = failure.details(json!(details));
    }
    if let Some(rid) = rid {
        failure = failure.request_id(rid).hint(if api.code == calternal_core::ErrorCode::Conflict {
            "Refresh the note list and check the current revision before retrying.".to_owned()
        } else if api.code == calternal_core::ErrorCode::NotFound {
            "Check the note or space UUID. Use notes list or trash list to find it.".to_owned()
        } else {
            format!("If the result is uncertain, repeat the identical command with --request-id {rid}.")
        });
    }
    failure
}

#[cfg(test)]
mod tests;
