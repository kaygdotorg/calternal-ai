//! Command-line adapter for the disposable loopback preview; uses shared client validation and retry identities.
//! Output (tables, `--json`, errors, exit codes) comes from `calternal_cli::output`; see `docs/cli.md`.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
mod render;
use calternal_cli::output::{CliError, Output, Reply};
use calternal_client::{CancellationToken, Client, ClientError};
use calternal_core::*;
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use std::{path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "cl",
    version,
    about = "Use Calternal's development preview from the command line",
    after_help = "Examples:\n  cl describe\n  cl notes list --space SPACE_UUID\n\nSee also: cl notes --help, cl plugins --help, cl invoke --help."
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
    #[arg(
        long,
        global = true,
        help = "Print one JSON document ({\"ok\":...}) instead of text; errors too, on stdout"
    )]
    json: bool,
    /// Hide the short next-step line shown after successful terminal output.
    #[arg(
        long,
        global = true,
        help = "Hide terminal hints (also CALTERNAL_NO_HINTS=1)"
    )]
    no_hints: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Drafts {
        #[command(subcommand)]
        command: calternal_cli::account_input::Drafts,
    },
    Pending {
        #[command(subcommand)]
        command: calternal_cli::account_input::Pending,
    },
    Requests {
        #[command(subcommand)]
        command: calternal_cli::requests::Command,
    },
    #[command(
        about = "Follow a space and print changes as they arrive",
        after_help = "Examples:\n  cl watch <space-id>\n  cl watch <space-id> --json\n\nSee also: cl describe --help."
    )]
    Watch {
        space: String,
        #[arg(
            long,
            help = "Bookmark cache directory exclusive to this server and account"
        )]
        bookmarks: Option<PathBuf>,
    },
    #[command(
        about = "Show your spaces and the operations available from this server",
        after_help = "Examples:\n  cl describe\n\nSee also: cl notes --help, cl plugins --help."
    )]
    Describe,
    #[command(
        alias = "note",
        about = "Create, read, and review notes in a space",
        after_help = "Examples:\n  cl notes list\n  cl notes create --title Plan --markdown-file plan.md\n\nSee also: cl notes list --help, cl plugins --help."
    )]
    Notes {
        #[command(subcommand)]
        command: NotesCommand,
    },
    #[command(
        alias = "plugin",
        about = "List and manage plugins enabled in a space",
        after_help = "Examples:\n  cl plugins list\n  cl plugins preview calternal-notes\n\nSee also: cl plugins list --help, cl notes --help."
    )]
    Plugins {
        #[command(subcommand)]
        command: PluginsCommand,
    },
    #[command(
        about = "Call an operation by its stable operation id",
        after_help = "Examples:\n  cl invoke notes.list --input '{\"space_id\":\"SPACE_UUID\"}'\n\nWith a thread credential, access.request gets its job ID from the credential. People must supply job_id.\n\nSee also: cl describe, cl notes --help."
    )]
    Invoke {
        /// Stable operation id from `cl describe`.
        operation: String,
        /// JSON input object matching the operation's input schema.
        #[arg(long, help = "JSON input object for the operation")]
        input: String,
    },
}
#[derive(Args)]
struct Space {
    /// Space UUID, as listed by `describe`.
    #[arg(long, alias = "space-id")]
    space: String,
}
#[derive(Args)]
struct ItemSpace {
    #[arg(
        long,
        alias = "space-id",
        default_value = "",
        help = "Optional space UUID; item IDs already identify their owning space"
    )]
    space: String,
}
#[derive(Args)]
struct DocumentText {
    #[arg(
        long,
        conflicts_with = "markdown_file",
        required_unless_present = "markdown_file",
        help = "Markdown content to save"
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
    #[command(
        about = "List notes in a space",
        after_help = "Examples:\n  cl notes list\n  cl notes list --space SPACE_UUID\n\nSee also: cl notes create, cl notes get."
    )]
    List {
        #[command(flatten)]
        scope: Space,
        /// Exact tag without a leading hash, including descendant tags.
        #[arg(long)]
        tag: Option<String>,
        /// Fetch the next page of up to 100 notes with the same space and tag.
        #[arg(long)]
        cursor: Option<String>,
    },
    #[command(
        about = "Show one note and its current revision",
        after_help = "Examples:\n  cl notes get NOTE_UUID\n\nSee also: cl notes list, cl notes update."
    )]
    Get {
        /// Note UUID.
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
    },
    #[command(
        about = "Create a note in a space",
        after_help = "Examples:\n  cl notes create --title Plan --markdown '# Plan'\n  cl notes create --title Plan --markdown-file plan.md\n\nSee also: cl notes list, cl notes get."
    )]
    Create {
        #[command(flatten)]
        scope: Space,
        /// Short title shown in note lists.
        #[arg(long)]
        title: String,
        #[command(flatten)]
        text: DocumentText,
    },
    #[command(
        about = "Replace a note's content against its current revision",
        after_help = "Examples:\n  cl notes update NOTE_UUID --base-revision r-2 --markdown-file plan.md\n\nSee also: cl notes get, cl notes history."
    )]
    Update {
        /// Note UUID.
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
        /// Revision returned by `cl notes get`; a stale value is rejected.
        #[arg(long)]
        base_revision: String,
        /// Replace the note title too.
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        text: DocumentText,
    },
    #[command(
        about = "List earlier saved versions of a note",
        after_help = "Examples:\n  cl notes history NOTE_UUID\n  cl notes history NOTE_UUID --limit 10\n\nSee also: cl notes get, cl notes update."
    )]
    History {
        /// Note UUID.
        id: String,
        #[command(flatten)]
        scope: ItemSpace,
        /// Maximum number of versions to show.
        #[arg(long)]
        limit: Option<u32>,
        /// Continue from the cursor printed by the previous page.
        #[arg(long)]
        cursor: Option<String>,
    },
}
#[derive(Subcommand)]
enum PluginsCommand {
    #[command(
        about = "List plugins installed in a space",
        after_help = "Examples:\n  cl plugins list\n  cl plugins list --space SPACE_UUID\n\nSee also: cl plugins enable, cl plugins preview."
    )]
    List {
        #[command(flatten)]
        scope: Space,
    },
    #[command(
        about = "Enable an installed plugin in a space",
        after_help = "Examples:\n  cl plugins enable calternal-notes\n\nSee also: cl plugins list, cl plugins preview."
    )]
    Enable {
        /// Plugin ID from `cl plugins list`.
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
    #[command(
        about = "Disable a plugin in a space",
        after_help = "Examples:\n  cl plugins disable calternal-notes\n\nSee also: cl plugins list, cl plugins enable."
    )]
    Disable {
        /// Plugin ID from `cl plugins list`.
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
    #[command(
        about = "Preview a plugin's view without changing anything",
        after_help = "Examples:\n  cl plugins preview calternal-notes\n\nSee also: cl plugins list, cl plugins enable."
    )]
    Preview {
        /// Plugin ID from `cl plugins list`.
        plugin_id: String,
        #[command(flatten)]
        scope: Space,
    },
}
fn token_error(message: &str) -> CliError {
    let human = match message {
        "cannot inspect the token file" => "Couldn't inspect the credential file.",
        "cannot read the token file" => "Couldn't read the credential file; check its permissions.",
        "the token file is too large" => "The credential file is larger than expected.",
        other => other,
    };
    CliError::new("unauthorized", message)
        .hint("Put the credential in a file readable only by you (chmod 600) and pass --token-file, or set CALTERNAL_TOKEN_FILE.")
        .human_message(human)
}
fn load_token(file: Option<PathBuf>) -> Result<String, CliError> {
    if let Ok(token) = std::env::var("CALTERNAL_TOKEN")
        && !token.is_empty()
    {
        return Ok(token);
    }
    let file = file.ok_or_else(|| {
        token_error(
            "no credential given: set CALTERNAL_TOKEN or CALTERNAL_TOKEN_FILE (or --token-file)",
        )
    })?;
    let metadata = std::fs::symlink_metadata(&file)
        .map_err(|_| token_error("cannot inspect the token file"))?;
    if !metadata.is_file() {
        return Err(token_error(
            "the token file must be a regular file, not a symlink",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.mode() & 0o077 != 0 || metadata.nlink() != 1 {
            return Err(token_error(
                "the token file must be owner-only with one link (chmod 600)",
            ));
        }
    }
    if metadata.len() > 4096 {
        return Err(token_error("the token file is too large"));
    }
    let token =
        std::fs::read_to_string(file).map_err(|_| token_error("cannot read the token file"))?;
    Ok(token.trim_end_matches(['\n', '\r']).to_string())
}
fn request_for(command: Command, request_id: Option<String>) -> Result<PendingRequest, String> {
    let (operation, mut input) = match command {
        Command::Drafts { command } => {
            let (op, input) = command.operation();
            (op.into(), input)
        }
        Command::Pending { command } => {
            let (op, input) = command
                .operation()
                .map_err(|_| "cannot read pending input; use --help")?;
            (op.into(), input)
        }
        Command::Requests { command } => {
            let (op, input) = command
                .operation()
                .map_err(|_| "--answer must be a JSON object")?;
            (op.into(), input)
        }
        Command::Watch { .. } => return Err("watch is a stream".into()),
        Command::Describe => return Err("describe is not an operation".into()),
        Command::Invoke { operation, input } => {
            let input: Value =
                serde_json::from_str(&input).map_err(|_| "--input must be valid JSON")?;
            (operation, input)
        }
        Command::Notes { command } => match command {
            NotesCommand::List { scope, tag, cursor } => {
                let mut input = json!({"space_id":scope.space});
                if let Some(tag) = tag {
                    input["tag"] = json!(tag);
                }
                if let Some(cursor) = cursor {
                    input["cursor"] = json!(cursor);
                }
                ("notes.list".into(), input)
            }
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
fn input_error(message: impl Into<String>) -> CliError {
    let message = message.into();
    let human = match message.as_str() {
        "--input must be valid JSON" => "The --input value is not valid JSON.".to_owned(),
        "cannot open Markdown file" => {
            "Couldn't open the Markdown file; check that its path exists and is readable."
                .to_owned()
        }
        "cannot read UTF-8 Markdown" => {
            "Couldn't read the Markdown file as UTF-8; check its contents.".to_owned()
        }
        "Markdown input is required" => {
            "Provide Markdown with `--markdown` or `--markdown-file`.".to_owned()
        }
        other => format!("The input could not be used: {other}"),
    };
    CliError::new("invalid_input", message)
        .hint("Run the command with --help to see the options it needs.")
        .human_message(human)
}
async fn run(cli: Cli, out: &Output) -> Result<Reply, CliError> {
    if let Command::Watch { space, bookmarks } = &cli.command {
        let token = cli
            .token_file
            .ok_or_else(|| input_error("--token-file is required"))?;
        return calternal_cli::watch::run(
            &cli.base_url,
            token,
            space.clone(),
            bookmarks.clone(),
            out,
            CancellationToken::new(),
        )
        .await;
    }
    let token = load_token(cli.token_file)?;
    let client = Client::new(&cli.base_url, &token).map_err(|e| {
        render::api_error(e.api_error(), None)
            .hint("Check --base-url (it must be a loopback http URL) and the credential.")
    })?;
    if matches!(cli.command, Command::Describe) {
        return match client.describe_cancellable(&CancellationToken::new()).await {
            Ok(result) => Ok(render::describe(&result)),
            Err(e) => Err(render::api_error(e.api_error(), None)),
        };
    }
    let pending = request_for(cli.command, cli.request_id).map_err(input_error)?;
    let id = pending.request().request_id.clone();
    let operation = pending.request().operation.clone();
    match client
        .execute_cancellable::<Value>(pending.request(), &CancellationToken::new())
        .await
    {
        Ok(data) => Ok(render::operation(out, &operation, &id, data)),
        Err(error) => {
            let api = error.api_error();
            Err(if matches!(error, ClientError::Transport) {
                render::transport_unknown(api, id)
            } else {
                render::api_error(api, Some(id))
            })
        }
    }
}
fn main() -> ExitCode {
    let cli = match calternal_cli::output::parse_args::<Cli>() {
        Ok(cli) => cli,
        Err(code) => return ExitCode::from(code as u8),
    };
    // Detect the time zone before the runtime starts its threads.
    let out = Output::detect_with_options(cli.json, cli.no_hints);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let result = runtime.block_on(run(cli, &out));
    ExitCode::from(out.finish(result) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_token_argv() {
        assert!(Cli::try_parse_from(["cl", "--token", "secret", "describe"]).is_err());
    }
    #[test]
    fn no_hints_is_global_and_command_help_has_examples_and_see_also() {
        let space = calternal_types::SpaceId::new().to_string();
        let cli =
            Cli::try_parse_from(["cl", "notes", "list", "--space", &space, "--no-hints"]).unwrap();
        assert!(cli.no_hints);
        let commands: &[&[&str]] = &[
            &["cl", "--help"],
            &["cl", "describe", "--help"],
            &["cl", "notes", "--help"],
            &["cl", "notes", "list", "--help"],
            &["cl", "notes", "get", "ID", "--help"],
            &["cl", "notes", "create", "--help"],
            &["cl", "notes", "update", "ID", "--help"],
            &["cl", "notes", "history", "ID", "--help"],
            &["cl", "plugins", "--help"],
            &["cl", "plugins", "list", "--help"],
            &["cl", "plugins", "enable", "ID", "--help"],
            &["cl", "plugins", "disable", "ID", "--help"],
            &["cl", "plugins", "preview", "ID", "--help"],
            &["cl", "invoke", "--help"],
        ];
        for args in commands {
            let help = Cli::try_parse_from(args.iter().copied())
                .err()
                .expect("--help exits before running the command")
                .to_string();
            assert!(help.contains("Examples:"), "{args:?}: {help}");
            assert!(help.contains("See also:"), "{args:?}: {help}");
        }
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
