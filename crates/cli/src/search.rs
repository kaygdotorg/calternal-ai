//! Non-interactive palette and picker commands over the existing operation client.
use crate::output::Reply;
use calternal_core::{LookupInput, SearchInput, SearchOperation};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

/// Paging and a space filter; without a space, search readable spaces.
#[derive(Debug, Args)]
pub struct Filters {
    /// Restrict results to this space UUID.
    #[arg(long)]
    space: Option<String>,
    /// Maximum rows (1–50).
    #[arg(long)]
    limit: Option<u32>,
    /// Continue a page. Pass - to read the cursor from stdin.
    #[arg(long, value_parser = crate::parse_cursor)]
    cursor: Option<String>,
}

/// Inputs for a picker limited to the selected space.
#[derive(Debug, Args)]
pub struct LocalPicker {
    /// Current space UUID.
    #[arg(long)]
    space: String,
    /// Match this display-name prefix.
    #[arg(default_value = "")]
    prefix: String,
}

/// Full-text search, title and mention lookup, and palette discovery.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Search titles first, then body text.
    Query {
        /// Text to search.
        query: String,
        /// Restrict to a manifest kind. Repeat for multiple kinds.
        #[arg(long = "kind")]
        kinds: Vec<String>,
        #[command(flatten)]
        /// Scope and paging options.
        filters: Filters,
    },
    /// Find items by literal title prefix.
    Items {
        /// Beginning of the title.
        prefix: String,
        #[command(flatten)]
        /// Scope and paging options.
        filters: Filters,
    },
    /// Find threads by literal title prefix.
    Threads {
        /// Beginning of the title.
        prefix: String,
        #[command(flatten)]
        /// Scope and paging options.
        filters: Filters,
    },
    /// Find mentionable accounts and groups.
    People(LocalPicker),
    /// List skill names and summaries.
    Skills(LocalPicker),
    /// List enabled plugin commands and Settings sections.
    Manifests {
        /// Current space UUID.
        #[arg(long)]
        space: String,
    },
}

impl Command {
    /// Build the same typed operation input and identifier used by other clients.
    pub fn operation(self) -> (SearchOperation, Value) {
        match self {
            Self::Query {
                query,
                kinds,
                filters,
            } => (
                SearchOperation::Query,
                json!(SearchInput {
                    query,
                    kinds,
                    space_id: filters.space,
                    limit: filters.limit,
                    cursor: filters.cursor,
                }),
            ),
            Self::Items {
                ref prefix,
                ref filters,
            }
            | Self::Threads {
                ref prefix,
                ref filters,
            } => {
                let op = if matches!(self, Self::Threads { .. }) {
                    SearchOperation::Threads
                } else {
                    SearchOperation::Items
                };
                (
                    op,
                    json!(LookupInput {
                        prefix: prefix.clone(),
                        space_id: filters.space.clone(),
                        limit: filters.limit,
                        cursor: filters.cursor.clone(),
                    }),
                )
            }
            Self::People(picker) => picker.operation(SearchOperation::People),
            Self::Skills(picker) => picker.operation(SearchOperation::Skills),
            Self::Manifests { space } => (SearchOperation::Manifests, json!({"space_id":space})),
        }
    }
}

impl LocalPicker {
    fn operation(self, op: SearchOperation) -> (SearchOperation, Value) {
        (op, json!({"space_id":self.space,"prefix":self.prefix}))
    }
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

fn rows<'a>(data: &'a Value, field: &str) -> impl Iterator<Item = &'a Value> {
    data[field].as_array().into_iter().flatten()
}

fn table_rows(data: &Value, field: &str, keys: &[&str]) -> Vec<Vec<String>> {
    rows(data, field)
        .map(|row| keys.iter().map(|key| s(row, key).to_owned()).collect())
        .collect()
}

/// Render adaptive tables or the unchanged operation response under JSON output.
pub fn reply(op: SearchOperation, data: Value) -> Reply {
    let reply = Reply::new(data.clone()).hint("Next: calternal search --help.");
    match op {
        SearchOperation::Query | SearchOperation::Threads | SearchOperation::Items => {
            let with_excerpt = op == SearchOperation::Query;
            let with_message =
                with_excerpt && rows(&data, "hits").any(|hit| hit["message_id"].is_string());
            let mut headers = vec!["TITLE", "KIND", "ADDRESS", "SPACE"];
            if with_excerpt {
                headers.push("EXCERPT");
            }
            if with_message {
                headers.extend(["MESSAGE ID", "SENT"]);
            }
            let hits = rows(&data, "hits")
                .map(|hit| {
                    let mut row = vec![
                        s(hit, "title").into(),
                        s(hit, "kind").into(),
                        s(hit, "address").into(),
                        s(hit, "space_id").into(),
                    ];
                    if with_excerpt {
                        let excerpt: String =
                            rows(hit, "excerpt").map(|chunk| s(chunk, "text")).collect();
                        row.push(excerpt.split_whitespace().collect::<Vec<_>>().join(" "));
                    }
                    if with_message {
                        row.extend([s(hit, "message_id").into(), s(hit, "message_at").into()]);
                    }
                    row
                })
                .collect();
            let reply = reply.table(&headers, hits, "No matching items.");
            let reply = if data["partial"] == true {
                reply.line("An index reached its coverage limit. Some items may be missing.")
            } else {
                reply
            };
            reply.page_cursor()
        }
        SearchOperation::People => reply.table(
            &["NAME", "TYPE", "ID"],
            table_rows(&data, "people", &["name", "kind", "id"]),
            "No matching people or groups.",
        ),
        SearchOperation::Skills => reply.table(
            &["SKILL", "DESCRIPTION"],
            table_rows(&data, "skills", &["name", "description"]),
            "No matching skills.",
        ),
        SearchOperation::Manifests => {
            let mut reply = reply;
            let mut found = false;
            for menu in rows(&data, "manifests") {
                found = true;
                let commands = rows(menu, "commands")
                    .filter(|command| {
                        !rows(menu, "settings_sections")
                            .any(|section| section["id"] == command["id"])
                    })
                    .map(|command| {
                        let words = rows(command, "words")
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ");
                        let action = command["op"].as_str().unwrap_or_else(|| s(command, "go"));
                        vec![
                            s(command, "label").into(),
                            s(command, "id").into(),
                            s(command, "group").into(),
                            action.into(),
                            words,
                        ]
                    })
                    .collect();
                reply = reply
                    .fields([
                        ("Plugin", s(menu, "plugin")),
                        ("Version", s(menu, "version")),
                    ])
                    .table(
                        &["COMMAND", "ID", "GROUP", "ACTION", "PHRASES"],
                        commands,
                        "No commands.",
                    )
                    .table(
                        &["SETTINGS", "ID", "ADDRESS"],
                        table_rows(menu, "settings_sections", &["label", "id", "go"]),
                        "No Settings sections.",
                    );
            }
            if found {
                reply
            } else {
                reply.line("No enabled plugins contribute to the palette.")
            }
        }
    }
}

#[cfg(test)]
mod tests;
