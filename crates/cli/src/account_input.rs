//! Private input CLI shared by the host binary and cl; all rendering uses Output.
use crate::output::{CliError, Output, Reply};
use calternal_client::{CancellationToken, Client};
use clap::{Args, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Network and stable request options for account input commands.
#[derive(Args)]
pub struct Connection {
    /// Loopback server URL.
    #[arg(
        long,
        global = true,
        env = "CALTERNAL_URL",
        default_value = "http://127.0.0.1:3210"
    )]
    pub base_url: String,
    /// Owner-only credential file, never a token argument.
    #[arg(long, global = true, env = "CALTERNAL_TOKEN_FILE")]
    pub token_file: Option<PathBuf>,
    /// Stable identity for retrying this operation.
    #[arg(long, global = true)]
    pub request_id: Option<String>,
}
/// Draft commands; each targets the authenticated account only.
#[derive(Subcommand)]
pub enum Drafts {
    /// Save an unsent composition; the server's last accepted write wins.
    Save {
        /// Target space UUID.
        #[arg(long)]
        space: String,
        /// Thread or item UUID.
        item: String,
        /// UTF-8 text; at most the document byte limit.
        #[arg(long)]
        text: String,
    },
    /// List this account's currently authorized drafts.
    List,
    /// Read a composition and its server revision.
    Get {
        /// Target space UUID.
        #[arg(long)]
        space: String,
        /// Thread or item UUID.
        item: String,
    },
    /// Erase a composition.
    Delete {
        /// Target space UUID.
        #[arg(long)]
        space: String,
        /// Thread or item UUID.
        item: String,
    },
}
/// Visible pending submissions; old input requires explicit confirmation.
#[derive(Subcommand)]
pub enum Pending {
    /// Queue a stable envelope from a JSON file (or - for stdin).
    Queue {
        /// PendingQueueInput file: space_id, item_id, created_at_ms, request.
        file: PathBuf,
    },
    /// Replace an envelope before any submission attempt.
    Edit {
        /// PendingQueueInput file retaining its request ID and target.
        file: PathBuf,
    },
    /// List pending envelopes and requests awaiting confirmation.
    List,
    /// Cancel an envelope that has not been attempted.
    Cancel {
        /// Stable request ID of the queued write.
        id: String,
    },
    /// Confirm and send input held for more than seven days.
    Confirm {
        /// Stable request ID of the queued write.
        id: String,
    },
    /// Attempt delivery with current access and the original request ID.
    Submit {
        /// Stable request ID of the queued write.
        id: String,
    },
}
impl Drafts {
    /// Convert one CLI command to its shared operation; no effects or authority.
    pub fn operation(self) -> (&'static str, Value) {
        match self {
            Self::Save { space, item, text } => (
                "drafts.save",
                json!({"space_id":space,"item_id":item,"text":text}),
            ),
            Self::List => ("drafts.list", json!({})),
            Self::Get { space, item } => ("drafts.get", json!({"space_id":space,"item_id":item})),
            Self::Delete { space, item } => {
                ("drafts.delete", json!({"space_id":space,"item_id":item}))
            }
        }
    }
}
impl Pending {
    /// Convert a command, reading at most 2 MiB for a queued envelope. File errors
    /// are caller-safe; JSON is validated by Client before any network effects.
    pub fn operation(self) -> Result<(&'static str, Value), CliError> {
        fn read(path: PathBuf) -> Result<Value, CliError> {
            use std::io::Read;
            let reader: Box<dyn Read> = if path.as_os_str() == "-" {
                Box::new(std::io::stdin())
            } else {
                Box::new(
                    std::fs::File::open(path)
                        .map_err(|_| CliError::usage("cannot open pending input file"))?,
                )
            };
            let mut bytes = Vec::new();
            reader
                .take(calternal_core::MAX_BODY_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| CliError::usage("cannot read pending input"))?;
            if bytes.len() > calternal_core::MAX_BODY_BYTES {
                return Err(CliError::usage("pending input exceeds 2 MiB"));
            }
            serde_json::from_slice(&bytes).map_err(|_| {
                CliError::usage(
                    "pending input must be a JSON object; see calternal pending queue --help",
                )
            })
        }
        Ok(match self {
            Self::Queue { file } => ("pending.queue", read(file)?),
            Self::Edit { file } => ("pending.edit", read(file)?),
            Self::List => ("pending.list", json!({})),
            Self::Cancel { id } => ("pending.cancel", json!({"id":id})),
            Self::Confirm { id } => ("pending.confirm", json!({"id":id})),
            Self::Submit { id } => ("pending.submit", json!({"id":id})),
        })
    }
}
/// Execute one private input command with the shared client gate and output.
/// Cost: one bounded credential read and up to three retry-safe HTTP attempts.
/// Call outside a runtime: CLI file reads finish before starting worker threads.
pub fn run(
    connection: Connection,
    operation: &str,
    input: Value,
    out: &Output,
) -> Result<Reply, CliError> {
    use std::os::unix::fs::MetadataExt;
    let token_file = connection.token_file.ok_or_else(|| {
        CliError::usage("provide --token-file or CALTERNAL_TOKEN_FILE")
            .hint("Use an owner-only credential file; see --help.")
    })?;
    let m = std::fs::symlink_metadata(&token_file)
        .map_err(|_| CliError::usage("cannot inspect token file"))?;
    if !m.is_file() || m.mode() & 0o077 != 0 || m.nlink() != 1 || m.len() > 4096 {
        return Err(CliError::usage(
            "token file must be owner-only, regular, at most 4096 bytes; chmod 600",
        ));
    }
    let token = std::fs::read_to_string(token_file)
        .map_err(|_| CliError::usage("cannot read token file"))?;
    let client = Client::new(&connection.base_url, token.trim()).map_err(error)?;
    let pending = Client::pending(operation, &input, connection.request_id).map_err(error)?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let data: Value = rt
        .block_on(client.execute_cancellable(pending.request(), &CancellationToken::new()))
        .map_err(error)?;
    Ok(render(out, operation, &pending.request().request_id, data))
}
fn error(e: calternal_client::ClientError) -> CliError {
    let api = e.api_error();
    CliError::new(serde_json::to_value(api.code).unwrap().as_str().unwrap_or("failed"),api.message).hint("Check the target and credential; run this command with --help. Keep the original request ID when a send outcome is uncertain.")
}
/// Render records in an adaptive table and labelled fields; JSON retains every
/// response field. Cost: bounded row formatting proportional to input size.
pub fn render(_out: &Output, operation: &str, id: &str, data: Value) -> Reply {
    let reply = Reply::new(data.clone()).request_id(id);
    let field = |v: &Value, k: &str| v[k].as_str().unwrap_or("").to_owned();
    let reply = if let Some(rows) = data["records"].as_array() {
        reply.table(
            &["ID", "SPACE", "STATE", "REVISION"],
            rows.iter()
                .map(|v| {
                    vec![
                        field(v, "id"),
                        field(v, "space_id"),
                        field(v, "state"),
                        field(v, "revision"),
                    ]
                })
                .collect(),
            "No saved input.",
        )
    } else if data["record"].is_object() {
        let r = &data["record"];
        reply
            .fields([
                ("id", field(r, "id")),
                ("space", field(r, "space_id")),
                ("item", field(r, "item_id")),
                ("state", field(r, "state")),
                ("revision", field(r, "revision")),
            ])
            .text(field(r, "text"))
    } else {
        reply.line("Done.")
    };
    reply.hint(if operation.starts_with("drafts.") {
        "Next: calternal drafts --help"
    } else {
        "Next: calternal pending --help"
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn list_widths_and_json_keep_private_record_shapes() {
        let data = json!({"records":[{"id":"11111111-1111-4111-8111-111111111111","space_id":"22222222-2222-4222-8222-222222222222","item_id":"33333333-3333-4333-8333-333333333333","revision":"44444444-4444-4444-8444-444444444444","created_at_ms":1,"text":"unsent","request":null,"state":"draft"}]});
        for width in [40, 80, 120] {
            let out = Output::fixed_terminal(false, width, time::UtcOffset::UTC, false);
            let rendered = out.render(&render(&out, "drafts.list", "request", data.clone()));
            assert!(rendered.lines().all(|l| l.chars().count() <= width));
            assert!(!rendered.contains("Next:"));
        }
        let out = Output::fixed(true, None, time::UtcOffset::UTC);
        let rendered = out.render(&render(&out, "drafts.list", "request", data));
        let parsed: Value = serde_json::from_str(&rendered).unwrap();
        crate::testing::assert_shape("calternal drafts list", &parsed);
        assert_eq!(parsed["data"]["records"][0]["text"], "unsent");
    }
}
