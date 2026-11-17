//! Non-interactive provider commands for `calternal`, using the shared output module.
use crate::output::{CliError, Output};
use calternal_client::{CancellationToken, Client};
use calternal_core::{AgentInput, AgentMode, OperationRequest};
use clap::{Args as ClapArgs, Subcommand};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};

/// Connection arguments shared by agent operations and the generic operation CLI.
#[derive(Debug, Clone, ClapArgs)]
pub struct Connection {
    /// Server URL. HTTPS is required outside loopback by the shared client.
    #[arg(long, env = "CALTERNAL_URL", default_value = "http://127.0.0.1:3210")]
    pub base_url: String,
    /// Private credential file; a sandbox uses CALTERNAL_AGENT_TOKEN instead.
    #[arg(long, env = "CALTERNAL_TOKEN_FILE")]
    pub token_file: Option<PathBuf>,
    /// Stable retry identity, kept unchanged after an uncertain write.
    #[arg(long)]
    pub request_id: Option<String>,
}

/// Arguments to `calternal agent`; results use the common output module.
#[derive(Debug, ClapArgs)]
pub struct Args {
    /// Server and credential selection.
    #[command(flatten)]
    pub connection: Connection,
    /// Requested provider action.
    #[command(subcommand)]
    pub command: Command,
}

/// Manual provider lifecycle and access commands; no interactive prompts or TUI.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check the official program inside a sandbox; prints its own sign-in command.
    Check {
        /// Space UUID.
        #[arg(long)]
        space_id: String,
        /// Thread UUID, allocated by the person before the storage lane exists.
        #[arg(long)]
        thread_id: String,
    },
    /// Start a turn and stream its ordered text and tool steps.
    Run {
        /// Space UUID.
        #[arg(long)]
        space_id: String,
        /// Thread UUID.
        #[arg(long)]
        thread_id: String,
        /// Prompt preserved as typed; never treated as an instruction-layer override.
        #[arg(long)]
        prompt: String,
        /// Installed CLI model identifier.
        #[arg(long, default_value = "sonnet")]
        model: String,
        /// Permission mode: least_access, approve_for_me (default) or full_access.
        #[arg(long, default_value = "approve_for_me")]
        mode: String,
        /// Originating app prefix, for least-access mode.
        #[arg(long, default_value = "")]
        source_app: String,
    },
    /// Cancel your turn; Managers may also cancel another member's turn.
    Stop {
        /// Turn job UUID.
        job_id: String,
    },
    /// Resume an interrupted turn explicitly, then stream the new run.
    Resume {
        /// Turn job UUID.
        job_id: String,
    },
    /// Change permission mode, revoke the old token and pause for explicit resume.
    Mode {
        /// Turn job UUID.
        job_id: String,
        /// least_access, approve_for_me or full_access; requires a fresh passkey session.
        mode: String,
    },
    /// Approve a pending scope using a freshly verified person session.
    Approve {
        /// Turn job UUID.
        job_id: String,
        /// Access request UUID.
        approval_id: String,
    },
}

fn error(message: &str) -> CliError {
    CliError::new("agent", message)
        .hint("Run calternal agent --help; inspect the turn before resuming uncertain work.")
}
fn client(c: &Connection) -> Result<Client, CliError> {
    let token = if let Some(path) = &c.token_file {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|_| error("Cannot read the credential file."))?;
        if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 || meta.len() > 4096 {
            return Err(error(
                "Credential file must be a private regular file under 4 KiB.",
            ));
        }
        std::fs::read_to_string(path).map_err(|_| error("Cannot read the credential file."))?
    } else {
        std::env::var("CALTERNAL_AGENT_TOKEN")
            .map_err(|_| error("Choose --token-file, or run inside a thread sandbox."))?
    };
    Client::new(&c.base_url, token.trim())
        .map_err(|_| error("Cannot connect with these server settings."))
}

async fn call(
    client: &Client,
    operation: &str,
    input: Value,
    rid: Option<String>,
) -> Result<Value, CliError> {
    let request = OperationRequest {
        request_id: rid.unwrap_or_else(|| {
            format!(
                "agent-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            )
        }),
        operation: operation.into(),
        input: serde_json::from_value(input).map_err(|_| error("Input must be a JSON object."))?,
    };
    client
        .execute_cancellable::<Value>(&request, &CancellationToken::new())
        .await
        .map_err(|e| {
            let api = e.api_error();
            let code = serde_json::to_value(api.code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_else(|| "internal".into());
            let mut failure = CliError::new(code, api.message)
                .request_id(request.request_id.clone())
                .hint("Check current access and inspect the turn before retrying.");
            if let Some(details) = api.details {
                failure = failure.details(json!(details));
            }
            failure
        })
}

fn mode(value: &str) -> Result<AgentMode, CliError> {
    match value {
        "least_access" => Ok(AgentMode::LeastAccess),
        "approve_for_me" => Ok(AgentMode::ApproveForMe),
        "full_access" => Ok(AgentMode::FullAccess),
        _ => Err(error(
            "Mode must be least_access, approve_for_me or full_access.",
        )),
    }
}

/// Execute an agent command in a fresh runtime; no provider credentials are loaded.
/// Streaming JSON is one compact event per line, including terminal and error events.
/// Returns a process exit code; it never launches an interactive sign-in flow.
pub fn main(args: Args, out: &Output) -> i32 {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("agent runtime");
    match runtime.block_on(run(args, out)) {
        Ok(()) => 0,
        Err(e) => {
            out.agent_failure(&e);
            1
        }
    }
}

async fn run(args: Args, out: &Output) -> Result<(), CliError> {
    let connection = args.connection.clone();
    let client = tokio::task::spawn_blocking(move || client(&connection))
        .await
        .map_err(|_| error("Credential loading failed."))??;
    let mut input = AgentInput::default();
    let (operation, stream) = match args.command {
        Command::Check {
            space_id,
            thread_id,
        } => {
            input.space_id = space_id;
            input.thread_id = thread_id;
            ("agent.check", true)
        }
        Command::Run {
            space_id,
            thread_id,
            prompt,
            model,
            mode: m,
            source_app,
        } => {
            input.space_id = space_id;
            input.thread_id = thread_id;
            input.prompt = prompt;
            input.model = model;
            input.mode = mode(&m)?;
            input.source_app = source_app;
            ("agent.run", true)
        }
        Command::Stop { job_id } => {
            input.job_id = job_id;
            ("agent.stop", false)
        }
        Command::Resume { job_id } => {
            input.job_id = job_id;
            ("agent.resume", true)
        }
        Command::Mode { job_id, mode: m } => {
            input.job_id = job_id;
            input.mode = mode(&m)?;
            ("agent.mode", false)
        }
        Command::Approve {
            job_id,
            approval_id,
        } => {
            input.job_id = job_id;
            input.approval_id = approval_id;
            ("access.approve", false)
        }
    };
    let result = call(&client, operation, json!(input), args.connection.request_id).await?;
    out.agent_event(&json!({"type":"job","job":result["job"]}));
    if !stream {
        return Ok(());
    }
    let id = result["job"]["id"]
        .as_str()
        .ok_or_else(|| error("Server did not return a turn ID."))?
        .to_owned();
    let (mut epoch, mut next) = (String::new(), 0u64);
    let mut requests = std::collections::HashSet::new();
    let mut retry = Value::Null;
    loop {
        let value = call(
            &client,
            "agent.events",
            json!({"job_id":id,"epoch":epoch,"after":next}),
            None,
        )
        .await?;
        if let Some(pending) = value["requests"].as_array() {
            for r in pending {
                if r["approved"] == false && requests.insert(r["id"].to_string()) {
                    out.agent_event(&json!({"type":"approval_request","request":r}));
                }
            }
        }
        if value["reset"] == true {
            out.agent_event(&json!({"type":"reset","checkpoint":value["job"]["checkpoint"]}));
        }
        if let Some(events) = value["events"].as_array() {
            for event in events {
                out.agent_event(&event["data"]);
            }
        }
        epoch = value["epoch"].as_str().unwrap_or("").into();
        next = value["next"].as_u64().unwrap_or(next);
        if value["job"]["checkpoint"]["retry"] != retry {
            retry = value["job"]["checkpoint"]["retry"].clone();
            if !retry.is_null() {
                out.agent_event(&json!({"type":"retry_waiting","retry":retry}));
            }
        }
        let state = value["job"]["state"].as_str().unwrap_or("failed");
        if !matches!(state, "admitted" | "running" | "waiting" | "retry_waiting") {
            out.agent_event(&json!({"type":"end","job":value["job"],"hint":"Use calternal agent resume <job-id> for an interrupted turn."}));
            return if state == "done" {
                Ok(())
            } else {
                Err(error(
                    "Turn ended without success; inspect its state and checkpoint.",
                ))
            };
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Generic operation/discovery entry for agents and people. Uses the same
/// credential loader and client as manual turns, with standard structured output.
pub fn invoke(
    connection: Connection,
    operation: Option<String>,
    input: String,
    out: &Output,
) -> i32 {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("operation runtime");
    let result = runtime.block_on(async {
        let selected = connection.clone();
        let client = tokio::task::spawn_blocking(move || client(&selected))
            .await
            .map_err(|_| error("Credential loading failed."))??;
        let value = if let Some(op) = operation {
            call(
                &client,
                &op,
                serde_json::from_str(&input).map_err(|_| error("Input must be JSON."))?,
                connection.request_id,
            )
            .await?
        } else {
            serde_json::to_value(
                client
                    .describe_cancellable(&CancellationToken::new())
                    .await
                    .map_err(|_| error("Cannot discover operations."))?,
            )
            .map_err(|_| error("Cannot decode discovery."))?
        };
        Ok(crate::output::Reply::new(value.clone())
            .text(serde_json::to_string_pretty(&value).unwrap_or_default()))
    });
    out.finish(result)
}
