//! Non-interactive palette and picker commands over the existing operation client.
use crate::output::Reply;
use calternal_core::SearchOperation;
use clap::{Args, Subcommand};
use serde_json::{Value, json};

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

/// Space-scoped mention, skill and palette lookups.
#[derive(Debug, Subcommand)]
pub enum Command {
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
