//! Non-interactive provider commands for `calternal`, using the shared output module.
use crate::output::{CliError, Output};
use calternal_client::{CancellationToken, Client};
use calternal_core::{AgentInput, AgentMode, OperationRequest};
/// Operation used by the machine CLI's bounded diagnostic.
pub use calternal_core::{MACHINES_INSTALL, MACHINES_ONBOARDING, MACHINES_VERIFY};
use clap::{Args as ClapArgs, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;

mod readiness;

pub use readiness::reply as readiness_reply;

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
    /// Show cached machine and provider status for your account.
    #[command(after_help = "Example: calternal agent readiness --space-id SPACE_UUID
See also: calternal agent check --help")]
    Readiness {
        /// Space UUID.
        #[arg(long)]
        space_id: String,
        /// Optional saved thread UUID for its placement constraint.
        #[arg(long, default_value = "")]
        thread_id: String,
    },
    /// Check the official program inside a sandbox; prints its own sign-in command.
    Check {
        /// Provider plugin ID; omitted uses the server default.
        #[arg(long, default_value = "")]
        provider: String,
        /// Space UUID.
        #[arg(long)]
        space_id: String,
        /// Optional thread UUID to associate with the check request.
        #[arg(long, default_value = "")]
        thread_id: String,
        /// Refresh the provider on this account-owned machine.
        #[arg(long, default_value = "", conflicts_with = "pool")]
        machine_id: String,
        /// Refresh the provider on an available entitled pool machine.
        #[arg(long)]
        pool: bool,
    },
    /// Start a turn and stream its ordered text and tool steps.
    Run {
        /// Provider plugin ID; omitted uses the server default.
        #[arg(long, default_value = "")]
        provider: String,
        /// Space UUID.
        #[arg(long)]
        space_id: String,
        /// Thread UUID.
        #[arg(long)]
        thread_id: String,
        /// Prompt preserved as typed; never treated as an instruction-layer override.
        #[arg(long)]
        prompt: String,
        /// Installed CLI model identifier; omitted uses the provider manifest default.
        #[arg(long, default_value = "")]
        model: String,
        /// Permission mode: least_access, approve_for_me (default) or full_access.
        #[arg(long, default_value = "approve_for_me")]
        mode: String,
        /// Originating app prefix, for least-access mode.
        #[arg(long, default_value = "")]
        source_app: String,
    },
    /// Explicitly discard a stopped workspace; account login home is preserved.
    Discard {
        /// Turn job UUID.
        job_id: String,
        /// Retry a machine-reported cleanup failure after inspecting active work.
        #[arg(long)]
        retry: bool,
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
}

fn error(message: &str) -> CliError {
    CliError::new("agent", message)
        .hint("Run calternal agent --help; inspect the turn before resuming uncertain work.")
}
pub(crate) fn client(c: &Connection) -> Result<Client, CliError> {
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

pub(crate) async fn call(
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
        input: serde_json::from_value(input).map_err(|_| {
            CliError::new("invalid_input", "Input /: expected a JSON object.")
                .hint("Pass an object with the fields listed by `calternal describe --json`.")
        })?,
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
            let reason = api.details.as_ref().and_then(|d| d.get("reason")).and_then(Value::as_str);
            let space_not_found = api.code == calternal_types::ErrorCode::NotFound
                && api.message == "space not found";
            let hint = if space_not_found {
                "Check the space shown by plugin dev or spaces.list. In SSH, expand SPACE_ID in the shell where you sourced the env file."
            } else if api.code == calternal_types::ErrorCode::InvalidInput {
                "Correct the field shown above. Check the plugin manifest or run `calternal describe --json` for host operation schemas."
            } else if reason == Some("worker_busy") {
                "Retry when the current invocations finish. For a write, repeat with the same request ID."
            } else if operation == MACHINES_VERIFY {
                "Use a person token and check the machine UUID and connection before retrying."
            } else if operation.starts_with("agent.") {
                "Check current access and inspect the turn before retrying."
            } else {
                "Check that the operation is available in the selected space and that your account can use it."
            };
            if space_not_found {
                if let Some(space) = request.input.get("space_id").and_then(Value::as_str) {
                    let source = if std::env::var("SPACE_ID").as_deref() == Ok(space) {
                        "--input (matches the SPACE_ID environment variable)"
                    } else {
                        "--input"
                    };
                    let selected = if space.is_empty() {
                        "empty space ID".to_owned()
                    } else if let Ok(id) = calternal_types::SpaceId::parse(space) {
                        format!("space {id}")
                    } else {
                        "invalid space ID".to_owned()
                    };
                    return CliError::new(code, format!("Space not found for {selected} from {source}."))
                        .request_id(request.request_id.clone())
                        .details(json!({"space_id":space, "space_source":source}))
                        .hint(hint);
                }
            }
            let mut failure = CliError::new(code, api.message)
                .request_id(request.request_id.clone())
                .hint(hint);
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
        Command::Readiness {
            space_id,
            thread_id,
        } => {
            let mut target = json!({"space_id":space_id});
            if !thread_id.is_empty() {
                target["thread_id"] = json!(thread_id);
            }
            let value = call(
                &client,
                calternal_core::AGENT_READINESS,
                target,
                args.connection.request_id,
            )
            .await?;
            out.success(&readiness_reply(out, value));
            return Ok(());
        }
        Command::Check {
            provider,
            space_id,
            thread_id,
            machine_id,
            pool,
        } => {
            input.machine_id = machine_id;
            input.pool = pool;
            input.provider_id = provider;
            input.space_id = space_id;
            input.thread_id = thread_id;
            ("agent.check", true)
        }
        Command::Run {
            provider,
            space_id,
            thread_id,
            prompt,
            model,
            mode: m,
            source_app,
        } => {
            input.provider_id = provider;
            input.space_id = space_id;
            input.thread_id = thread_id;
            input.prompt = prompt;
            input.model = model;
            input.mode = mode(&m)?;
            input.source_app = source_app;
            ("agent.run", true)
        }
        Command::Discard { job_id, retry } => {
            input.job_id = job_id;
            input.retry_failed = retry;
            ("agent.discard", false)
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
    };
    let result = call(&client, operation, json!(input), args.connection.request_id).await?;
    if operation == "agent.discard" {
        out.agent_event(&json!({"type":"discard","state":result["discard"],"job":result["job"]}));
        return if result["discard"] == "failed" {
            Err(error(
                "Workspace deletion failed; inspect the machine, then use discard --retry.",
            ))
        } else {
            Ok(())
        };
    }
    out.agent_event(&json!({"type":"job","job":result["job"]}));
    if !stream {
        return Ok(());
    }
    let id = result["job"]["id"]
        .as_str()
        .ok_or_else(|| error("Server did not return a turn ID."))?
        .to_owned();
    let mut stream = client
        .subscribe_job(&id, CancellationToken::new())
        .map_err(|_| error("Cannot subscribe to this turn."))?;
    let mut retry = Value::Null;
    while let Some(event) = stream
        .next()
        .await
        .map_err(|_| error("Turn stream unavailable; inspect the turn before resuming."))?
    {
        match event.kind.as_str() {
            "chunk" => out.agent_event(&event.data["data"]),
            "reset" => out.agent_event(&json!({"type":"reset","checkpoint":event.data})),
            "snapshot" | "state" => {
                if event.data["job"]["checkpoint"]["retry"] != retry {
                    retry = event.data["job"]["checkpoint"]["retry"].clone();
                    if !retry.is_null() {
                        out.agent_event(&json!({"type":"retry_waiting","retry":retry}));
                    }
                }
                if event.data["job"]["state"] == "waiting" {
                    out.agent_event(&json!({"type":"waiting","message":"Waiting on a person. Use calternal requests list."}));
                }
            }
            "end" => {
                out.agent_event(&json!({"type":"end","job":event.data["job"],"hint":"Use calternal agent resume <job-id> for an interrupted turn."}));
                return if event.data["job"]["state"] == "done" {
                    Ok(())
                } else {
                    Err(error(
                        "Turn ended without success; inspect its state and checkpoint.",
                    ))
                };
            }
            _ => {}
        }
    }
    Err(error(
        "Turn stream ended before its final state; inspect the turn.",
    ))
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
        let value = if let Some(op) = &operation {
            call(
                &client,
                op,
                serde_json::from_str(&input).map_err(|error| {
                    CliError::new(
                        "invalid_input",
                        format!(
                            "Invalid JSON in --input at line {}, column {}.",
                            error.line(),
                            error.column()
                        ),
                    )
                    .hint("Pass a JSON object and quote it as one shell argument.")
                })?,
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
        if let Some(op) = &operation
            && calternal_core::note_operation(op).is_some()
        {
            return Ok(crate::notes::reply(out, op, value));
        }
        if matches!(
            operation.as_deref(),
            Some("plugins.enable" | "plugins.disable")
        ) {
            let plugin = &value["plugin"];
            let id = plugin["manifest"]["id"].as_str().unwrap_or_default();
            let version = plugin["manifest"]["version"].as_str().unwrap_or_default();
            let status = if plugin["enabled"] == true {
                "Enabled"
            } else {
                "Disabled"
            };
            return Ok(crate::output::Reply::new(value.clone())
                .fields([("Plugin", id), ("Version", version), ("Status", status)])
                .hint("Use `calternal plugin --help` for package and activation commands."));
        }
        if operation.as_deref() == Some(calternal_core::MACHINES_INSTALL) {
            let script = value["script"]
                .as_str()
                .ok_or_else(|| error("The server did not return an installer."))?;
            return Ok(crate::output::Reply::new(value.clone()).text(script));
        }
        if operation.as_deref() == Some(calternal_core::MACHINES_ONBOARDING) {
            let yes = |key| if value[key] == true { "Yes" } else { "No" };
            let hint = if value["ready"] == true {
                "Run calternal machine verify for a sandbox and provider check."
            } else {
                "Check that the machine service is running and has capacity, then retry."
            };
            let mut reply = crate::output::Reply::new(value.clone()).fields([
                ("Machine attached", yes("attached")),
                ("Ready for a sandbox", yes("ready")),
            ]);
            if let Some(id) = value["machine_id"].as_str() {
                reply = reply.fields([("Machine ID", id)]);
            }
            return Ok(reply.hint(hint));
        }
        if let Some(op) = operation
            .as_deref()
            .and_then(calternal_core::SearchOperation::from_id)
        {
            return Ok(crate::search::reply(op, value));
        }
        if operation.as_deref() == Some(calternal_core::MACHINES_VERIFY) {
            let yes = |b: &Value| match b.as_bool() {
                Some(true) => "Yes",
                Some(false) => "No",
                None => "Unknown",
            };
            let check = &value["provider"];
            let mut reply = crate::output::Reply::new(value.clone()).fields([
                ("Sandbox started", yes(&value["sandbox_started"])),
                ("Provider installed", yes(&check["installed"])),
                ("Provider signed in", yes(&check["signed_in"])),
            ]);
            if let Some(command) = check["sign_in_command"].as_str().filter(|s| !s.is_empty()) {
                reply = reply.fields([("Sign-in command", command)]);
            }
            let hint = if value["sandbox_started"] != true {
                "Check that the machine is online and has sandbox capacity, then retry."
            } else if check["installed"] != true {
                "Check the provider installation in the sandbox, then retry."
            } else if check["signed_in"] != true {
                "Run the sign-in command inside the machine's sandbox, then retry."
            } else {
                "The machine and provider are ready."
            };
            return Ok(reply.hint(hint));
        }
        Ok(crate::output::Reply::new(value.clone())
            .text(serde_json::to_string_pretty(&value).unwrap_or_default()))
    });
    out.finish(result)
}
