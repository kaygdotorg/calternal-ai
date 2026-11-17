//! Command-line adapter for the disposable loopback preview; uses shared client validation and retry identities.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
use calternal_client::{CancellationToken, Client, ClientError};
use calternal_core::*;
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "cl",
    version,
    about = "Calternal localhost disposable-data development preview CLI"
)]
struct Cli {
    #[arg(
        long,
        env = "CALTERNAL_URL",
        default_value = "http://127.0.0.1:3210",
        global = true
    )]
    base_url: String,
    #[arg(
        long,
        env = "CALTERNAL_TOKEN_FILE",
        global = true,
        help = "Owner-only file containing a session credential or development bearer; tokens are never accepted in argv"
    )]
    token_file: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        help = "Stable request identity; reuse only with exactly the same operation and input"
    )]
    request_id: Option<String>,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Describe,
    #[command(alias = "note")]
    Notes {
        #[command(subcommand)]
        command: NotesCommand,
    },
    #[command(alias = "plugin")]
    Plugins {
        #[command(subcommand)]
        command: PluginsCommand,
    },
    Invoke {
        operation: String,
        #[arg(long)]
        input: String,
    },
}
#[derive(Args)]
struct Space {
    #[arg(long, alias = "space-id", default_value = "personal")]
    space: String,
}
#[derive(Args)]
struct ItemSpace {
    #[arg(
        long,
        alias = "space-id",
        default_value = "",
        help = "Optional compatibility space hint; item IDs resolve their owning space"
    )]
    space: String,
}
#[derive(Args)]
struct DocumentText {
    #[arg(
        long,
        conflicts_with = "markdown_file",
        required_unless_present = "markdown_file"
    )]
    markdown: Option<String>,
    #[arg(
        long,
        conflicts_with = "markdown",
        required_unless_present = "markdown",
        help = "Read UTF-8 Markdown from file, or - for stdin"
    )]
    markdown_file: Option<PathBuf>,
}
impl DocumentText {
    fn read(self) -> Result<String, String> {
        match (self.markdown, self.markdown_file) {
            (Some(text), _) => Ok(text),
            (_, Some(path)) => {
                use std::io::Read;
                let mut reader: Box<dyn Read> = if path.as_os_str() == "-" {
                    Box::new(std::io::stdin())
                } else {
                    Box::new(std::fs::File::open(path).map_err(|_| "cannot open Markdown file")?)
                };
                let mut text = String::new();
                reader
                    .by_ref()
                    .take(MAX_DOCUMENT_BYTES as u64 + 1)
                    .read_to_string(&mut text)
                    .map_err(|_| "cannot read UTF-8 Markdown")?;
                validate_markdown(&text).map_err(|e| e.to_string())?;
                Ok(text)
            }
            _ => Err("Markdown input is required".into()),
        }
    }
}
#[derive(Subcommand)]
enum NotesCommand {
    List {
        #[command(flatten)]
        scope: Space,
    },
    Get {
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
    },
    Create {
        #[command(flatten)]
        scope: Space,
        #[arg(long)]
        title: String,
        #[command(flatten)]
        text: DocumentText,
    },
    Update {
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
        #[arg(long)]
        base_revision: String,
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        text: DocumentText,
    },
    History {
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        cursor: Option<String>,
    },
}
#[derive(Subcommand)]
enum PluginsCommand {
    List {
        #[command(flatten)]
        scope: Space,
    },
    Enable {
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
    Disable {
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
    Preview {
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
}
fn load_token(file: Option<PathBuf>) -> Result<String, String> {
    if let Ok(token) = std::env::var("CALTERNAL_TOKEN")
        && !token.is_empty()
    {
        return Ok(token);
    }
    let file = file.ok_or("set CALTERNAL_TOKEN or CALTERNAL_TOKEN_FILE (or --token-file)")?;
    let metadata = std::fs::symlink_metadata(&file).map_err(|_| "cannot inspect token file")?;
    if !metadata.is_file() {
        return Err("token file must be a regular file, not a symlink".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err("token file must be owner-only with one link (chmod 600)".into());
        }
    }
    if metadata.len() > 4096 {
        return Err("token file is too large".into());
    }
    let token = std::fs::read_to_string(file).map_err(|_| "cannot read token file")?;
    Ok(token.trim_end_matches(['\n', '\r']).to_string())
}
fn request_for(command: Command, request_id: Option<String>) -> Result<PendingRequest, String> {
    let (operation, mut input) = match command {
        Command::Describe => return Err("describe is not an operation".into()),
        Command::Invoke { operation, input } => {
            let input: Value =
                serde_json::from_str(&input).map_err(|_| "--input must be valid JSON")?;
            (operation, input)
        }
        Command::Notes { command } => match command {
            NotesCommand::List { scope } => ("notes.list".into(), json!({"space_id":scope.space})),
            NotesCommand::Get { id, scope } => {
                ("notes.get".into(), json!({"space_id":scope.space,"id":id}))
            }
            NotesCommand::Create { scope, title, text } => (
                "notes.create".into(),
                json!({"space_id":scope.space,"title":title,"markdown":text.read()?}),
            ),
            NotesCommand::Update {
                id,
                scope,
                base_revision,
                title,
                text,
            } => {
                let mut input = json!({"space_id":scope.space,"id":id,"base_revision":base_revision,"markdown":text.read()?});
                if let Some(title) = title {
                    input["title"] = Value::String(title)
                }
                ("notes.update".into(), input)
            }
            NotesCommand::History {
                id,
                scope,
                limit,
                cursor,
            } => {
                let mut input = json!({"space_id":scope.space,"id":id});
                if let Some(limit) = limit {
                    input["limit"] = json!(limit)
                }
                if let Some(cursor) = cursor {
                    input["cursor"] = json!(cursor)
                }
                ("notes.history".into(), input)
            }
        },
        Command::Plugins { command } => match command {
            PluginsCommand::List { scope } => {
                ("plugins.list".into(), json!({"space_id":scope.space}))
            }
            PluginsCommand::Enable { plugin_id, scope } => (
                "plugins.enable".into(),
                json!({"space_id":scope.space,"plugin_id":plugin_id}),
            ),
            PluginsCommand::Disable { plugin_id, scope } => (
                "plugins.disable".into(),
                json!({"space_id":scope.space,"plugin_id":plugin_id}),
            ),
            PluginsCommand::Preview { plugin_id, scope } => (
                "plugins.preview".into(),
                json!({"space_id":scope.space,"plugin_id":plugin_id}),
            ),
        },
    };
    if matches!(
        operation.as_str(),
        "notes.get" | "notes.update" | "notes.history"
    ) && input.get("space_id").and_then(Value::as_str) == Some("")
    {
        input
            .as_object_mut()
            .expect("typed item input")
            .remove("space_id");
    }
    Client::pending(&operation, &input, request_id).map_err(|e| e.to_string())
}
fn report(request_id: Option<String>, error: ApiError) -> ExitCode {
    let result = Failure {
        request_id,
        ok: False,
        error,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&result).expect("error serialization")
    );
    ExitCode::FAILURE
}
#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let token = match load_token(cli.token_file) {
        Ok(token) => token,
        Err(message) => {
            return report(
                None,
                ApiError {
                    code: ErrorCode::Unauthorized,
                    message,
                    details: None,
                },
            );
        }
    };
    let client = match Client::new(&cli.base_url, &token) {
        Ok(client) => client,
        Err(e) => return report(None, e.api_error()),
    };
    if matches!(cli.command, Command::Describe) {
        return match client.describe_cancellable(&CancellationToken::new()).await {
            Ok(result) => {
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
                ExitCode::SUCCESS
            }
            Err(e) => report(None, e.api_error()),
        };
    }
    let pending = match request_for(cli.command, cli.request_id) {
        Ok(request) => request,
        Err(message) => {
            return report(
                None,
                ApiError {
                    code: ErrorCode::InvalidInput,
                    message,
                    details: None,
                },
            );
        }
    };
    let id = pending.request().request_id.clone();
    match client
        .execute_cancellable::<Value>(pending.request(), &CancellationToken::new())
        .await
    {
        Ok(data) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&OperationResponse::success(id, data)).unwrap()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            let mut api = error.api_error();
            if matches!(error, ClientError::Transport) {
                api.details=Some(serde_json::from_value(json!({"outcome":"unknown","retry":"reuse request_id with the identical operation and input"})).unwrap())
            }
            report(Some(id), api)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_token_argv() {
        assert!(Cli::try_parse_from(["cl", "--token", "secret", "describe"]).is_err());
    }
    #[test]
    fn update_requires_revision_and_text() {
        assert!(Cli::try_parse_from(["cl", "notes", "update", "id", "--markdown", "new"]).is_err());
    }
    #[test]
    fn omitted_optional_title_and_stable_retry_identity() {
        let cli = Cli::try_parse_from([
            "cl",
            "--request-id",
            "retry-1",
            "notes",
            "update",
            "11111111-1111-4111-8111-111111111111",
            "--base-revision",
            "opaque",
            "--markdown",
            "new",
        ])
        .unwrap();
        let request = request_for(cli.command, cli.request_id).unwrap();
        assert_eq!(request.request().request_id, "retry-1");
        assert!(!request.request().input.contains_key("title"));
    }
    #[test]
    fn generic_operation_does_not_need_new_core() {
        let cli = Cli::try_parse_from([
            "cl",
            "invoke",
            "future.read",
            "--input",
            r#"{"space_id":"personal"}"#,
        ])
        .unwrap();
        assert!(request_for(cli.command, None).is_ok());
    }
}
