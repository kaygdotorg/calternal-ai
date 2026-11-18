//! The operator commands of the `calternal` binary: `machines …` (execution
//! machines of an installation) and `plugin-digest`. Each builds a
//! [`Reply`]; printing, `--json`, errors and exit codes belong to
//! [`calternal_cli::output`] (reference: `docs/cli.md`).

use std::path::{Path, PathBuf};

use calternal_cli::output::{CliError, Output, Reply};
use calternal_machine::cli::owner_label;
use calternal_machine::protocol::Owner;
use clap::{Args, Subcommand};
use serde_json::json;

use calternal_server::machines::admin::{self, Request, Response};
use calternal_server::plugins;

/// The installation a `machines` command acts on: the same three roots and pinned
/// installation ID as `calternal serve`. Nothing is created from them.
#[derive(Args, Clone)]
pub struct Installation {
    /// The server's appdata root.
    #[arg(long)]
    pub appdata: PathBuf,
    /// The server's userdata-ssd root (holds the recovery log).
    #[arg(long)]
    pub userdata_ssd: PathBuf,
    /// The server's userdata-hdd root.
    #[arg(long)]
    pub userdata_hdd: PathBuf,
    /// This installation's UUID, as configured for `calternal serve`.
    #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
    pub installation_id: String,
}

/// `calternal machines …`: operator administration of execution machines.
#[derive(Subcommand)]
#[command(
    after_help = "Examples:\n  calternal machines list --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n  calternal machines code --pool --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n\nThe installation ID comes from CALTERNAL_INSTALLATION_ID or --installation-id.\nSee also: calternal machines --help, calternal machine --help."
)]
pub enum MachinesCmd {
    /// Create a one-time code to enroll a person-owned or pool machine.
    #[command(
        about = "Create a one-time machine enrollment code",
        after_help = "Examples:\n  calternal machines code --pool --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n  calternal machines code --account ACCOUNT_UUID --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n\nSee also: calternal machine enroll, calternal machines list."
    )]
    Code {
        #[command(flatten)]
        installation: Installation,
        /// Code for a pool machine the operator runs.
        #[arg(long, conflicts_with = "account")]
        pool: bool,
        /// Code for a machine owned by this account UUID.
        #[arg(long)]
        account: Option<String>,
        /// Validity in minutes.
        #[arg(long, default_value_t = 60)]
        minutes: u64,
    },
    /// List enrolled machines, including revoked machines.
    #[command(
        about = "List machines enrolled with this server",
        after_help = "Examples:\n  calternal machines list --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n\nSee also: calternal machines code, calternal machines revoke."
    )]
    List {
        #[command(flatten)]
        installation: Installation,
    },
    /// Revoke a machine; a running server applies it at once.
    #[command(
        about = "Revoke a machine's access to this server",
        after_help = "Examples:\n  calternal machines revoke MACHINE_UUID --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n\nSee also: calternal machines list, calternal machines code."
    )]
    Revoke {
        #[command(flatten)]
        installation: Installation,
        /// Machine UUID.
        machine: String,
    },
    /// Allow an account to use pool machines, or remove that access.
    #[command(
        about = "Grant or remove an account's access to pool machines",
        after_help = "Examples:\n  calternal machines pool-grant ACCOUNT_UUID --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n  calternal machines pool-grant ACCOUNT_UUID --remove --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal\n\nSee also: calternal machines list, calternal machines code."
    )]
    PoolGrant {
        #[command(flatten)]
        installation: Installation,
        /// Account UUID.
        account: String,
        #[arg(long)]
        remove: bool,
    },
}

fn installation_error(message: impl Into<String>) -> CliError {
    CliError::new("failed", message).hint(
        "Pass the same three roots and installation ID as `calternal serve`; see `calternal machines --help`.",
    )
}

fn bad_uuid(what: &str) -> CliError {
    CliError::new("invalid_input", format!("not a valid {what} UUID")).hint(format!(
        "Pass the {what}'s UUID, for example 123e4567-e89b-42d3-a456-426614174000."
    ))
}

/// Run one request on the installation: through the running server's admin
/// socket when it answers, otherwise by opening the stopped installation through
/// its root guard and recovery log. Nothing is created before validation.
/// Cost: one socket round trip, or one guarded open and at most one protected commit.
fn run(installation: Installation, request: Request) -> Result<Response, CliError> {
    let id = uuid::Uuid::parse_str(&installation.installation_id).map_err(|_| {
        CliError::new("invalid_input", "the installation ID is not a UUID")
            .hint("Use the UUID configured as CALTERNAL_INSTALLATION_ID for `calternal serve`.")
    })?;
    if let Some(response) = admin::ask_server(&installation.appdata, id, request.clone())
        .map_err(|e| installation_error(e.message))?
    {
        return Ok(response);
    }
    let roots = calternal_roots::Roots {
        appdata: installation.appdata,
        userdata_ssd: installation.userdata_ssd,
        userdata_hdd: installation.userdata_hdd,
    };
    let (_store, svc) =
        admin::open_offline(&roots, id).map_err(|e| installation_error(e.message))?;
    admin::execute(&svc, request).map_err(|e| installation_error(e.message))
}

fn unexpected() -> CliError {
    CliError::new("internal", "the server gave an unexpected answer")
}

/// Request a fresh code from the live admin socket. One round trip. The
/// command never opens the installation's database or prints an old code.
pub fn setup_code(appdata: &Path, installation_id: &str) -> Result<Reply, CliError> {
    let id = uuid::Uuid::parse_str(installation_id).map_err(|_| bad_uuid("installation"))?;
    let next = "Run the command as the server's user with its appdata path and installation ID.";
    match admin::ask_server(appdata, id, Request::SetupCode)
        .map_err(|e| CliError::new("failed", e.message).hint(next))?
    {
        Some(Response::SetupCode {
            code,
            expires_ms,
            url,
        }) => {
            let expiry = Output::iso_ms(expires_ms);
            Ok(
                Reply::new(json!({"code":code,"expires_at":expiry,"url":url}))
                    .line("First-owner setup code")
                    .fields([("code", code), ("expires at", expiry), ("open", url)])
                    .hint("Open the address and paste the code to register the first owner."),
            )
        }
        Some(Response::SetupComplete { url }) => Err(CliError::new(
            "setup_complete",
            "First-owner setup is already complete.",
        )
        .hint(format!("Open {url} and sign in with your passkey."))),
        None => Err(CliError::new(
            "server_unavailable",
            "The server's local admin socket is unavailable.",
        )
        .hint(
            "Start the server, then run this command as the server's user with the same --appdata.",
        )),
        _ => Err(unexpected()),
    }
}

/// Ask the local server for an enrollment grant after operator identity verification.
pub fn account_recover(
    appdata: &Path,
    installation_id: &str,
    account: &str,
    reason: &str,
    verified: bool,
) -> Result<Reply, CliError> {
    use std::io::IsTerminal;
    if !verified || !std::io::stdout().is_terminal() {
        return Err(CliError::new(
            "invalid_input",
            "Verify the person's identity and run the command in a private terminal.",
        ));
    }
    let id = uuid::Uuid::parse_str(installation_id).map_err(|_| bad_uuid("installation"))?;
    let account = calternal_types::AccountId::parse(account).map_err(|_| bad_uuid("account"))?;
    let request = Request::AccountRecover {
        account,
        reason: reason.to_owned(),
    };
    match admin::ask_server(appdata, id, request).map_err(|e| CliError::new("failed", e.message))? {
        Some(Response::AccountRecovery {
            enrollment_grant,
            url,
        }) => Ok(Reply::new(json!({
            "enrollment_grant":enrollment_grant,"url":url,"valid_for_seconds":900
        }))
        .line("Account recovery grant")
        .fields([("code", enrollment_grant), ("open", url)])
        .hint("Use this code within 15 minutes to enroll a replacement passkey.")),
        None => Err(CliError::new(
            "server_unavailable",
            "Start the server and run as the server's user.",
        )),
        _ => Err(unexpected()),
    }
}

/// Send one operator test message through the live server's SMTP queue.
/// Costs one local socket request and one bounded delivery attempt.
pub fn mail_test(
    appdata: &Path,
    installation_id: &str,
    recipient: &str,
) -> Result<Reply, CliError> {
    let id = uuid::Uuid::parse_str(installation_id).map_err(|_| bad_uuid("installation"))?;
    let request = Request::MailTest {
        recipient: recipient.to_owned(),
    };
    match admin::ask_server(appdata, id, request).map_err(|e| CliError::new("failed", e.message))? {
        Some(Response::MailTest {
            accepted: Some(true),
        }) => {
            Ok(Reply::new(json!({"accepted":true})).line("SMTP server accepted the test message."))
        }
        Some(Response::MailTest {
            accepted: Some(false),
        }) => Err(CliError::new(
            "mail_delivery_failed",
            "SMTP acceptance could not be confirmed.",
        )
        .hint("Check the recipient and the server's SMTP settings, then try again.")),
        Some(Response::MailTest { accepted: None }) => Err(CliError::new(
            "mail_not_configured",
            "Account mail is not configured on the running server.",
        )
        .hint("Configure SMTP on the server and restart it.")),
        None => Err(CliError::new(
            "server_unavailable",
            "Start the server and run as the server's user.",
        )),
        _ => Err(unexpected()),
    }
}

/// Inspect one space's receipt-pruning health through the live admin socket.
/// Costs one socket round trip and the server's read-only inspection.
pub fn receipt_status(
    appdata: &Path,
    installation_id: &str,
    space: &str,
) -> Result<Reply, CliError> {
    use calternal_content::ReceiptPruningIssue;
    let installation =
        uuid::Uuid::parse_str(installation_id).map_err(|_| bad_uuid("installation"))?;
    let space = calternal_types::SpaceId::parse(space).map_err(|_| bad_uuid("space"))?;
    let hint = "Run as the server's user with its appdata directory and installation ID.";
    let response = admin::ask_server(appdata, installation, Request::ReceiptStatus { space })
        .map_err(|e| CliError::new("failed", e.message).hint(hint))?;
    let issue = match response {
        Some(Response::ReceiptStatus { issue }) => issue,
        Some(_) => return Err(unexpected()),
        None => {
            return Err(CliError::new(
                "server_unavailable",
                "The local admin socket is unavailable.",
            )
            .hint(hint));
        }
    };
    let status = match &issue {
        None => "No receipt-pruning problem observed.".to_owned(),
        Some(ReceiptPruningIssue::ExpiredPin { age_ms }) => {
            let hours = age_ms / 3_600_000;
            format!(
                "Outbox pins an expired receipt ({} days, {} hours old).",
                hours / 24,
                hours % 24
            )
        }
        Some(ReceiptPruningIssue::OutboxUnreadable) => {
            "Outbox unreadable. Receipt age unknown.".to_owned()
        }
        Some(ReceiptPruningIssue::JournalUnreadable) => {
            "Journal unreadable. Receipt age unknown.".to_owned()
        }
    };
    Ok(Reply::new(json!({"space_id": space, "issue": issue})).line(status))
}

/// Run one `machines` subcommand. Cost: see [`run`].
pub fn machines(cmd: MachinesCmd, out: &Output) -> Result<Reply, CliError> {
    let account = |s: &str| calternal_types::AccountId::parse(s).map_err(|_| bad_uuid("account"));
    match cmd {
        MachinesCmd::Code {
            installation,
            pool,
            account: acct,
            minutes,
        } => {
            let owner = match (pool, acct) {
                (true, _) => Owner::Pool,
                (false, Some(a)) => Owner::Account { id: account(&a)? },
                (false, None) => {
                    return Err(CliError::usage("pass --pool or --account <uuid>")
                        .hint("--pool makes a machine the operator runs; --account makes one a person owns."));
                }
            };
            let Response::Code { code, expires_ms } =
                run(installation, Request::Code { owner, minutes })?
            else {
                return Err(unexpected());
            };
            Ok(Reply::new(json!({
                "code": code,
                "owner": owner_label(owner),
                "expires_at": Output::iso_ms(expires_ms),
            }))
            .fields([
                ("code", code),
                ("owner", owner_label(owner)),
                ("expires", out.local_ms(expires_ms)),
            ])
            .blank()
            .line("This code works once. Share it with the machine's owner now.")
            .hint("Next: run `calternal machine enroll` on that computer."))
        }
        MachinesCmd::List { installation } => {
            let Response::List { machines, erasures } = run(installation, Request::List)? else {
                return Err(unexpected());
            };
            let state = |m: &admin::MachineSummary| {
                if m.revoked {
                    "revoked"
                } else if m.draining {
                    "draining"
                } else {
                    "enrolled"
                }
            };
            let rows = machines
                .iter()
                .map(|m| {
                    vec![
                        m.id.to_string(),
                        owner_label(m.owner),
                        m.name.clone(),
                        format!("{}/{}", m.os, m.arch),
                        state(m).to_owned(),
                    ]
                })
                .collect();
            let listed: Vec<_> = machines
                .iter()
                .map(|m| {
                    json!({"id": m.id, "owner": owner_label(m.owner), "name": m.name,
                           "os": m.os, "arch": m.arch, "state": state(m)})
                })
                .collect();
            let empty = machines.is_empty();
            let mut reply = Reply::new(json!({ "machines": listed, "pending_erasures": erasures }))
                .table(
                    &["ID", "OWNER", "NAME", "PLATFORM", "STATE"],
                    rows,
                    "No machines are enrolled yet. Run `calternal machines code` to add one.",
                );
            if !erasures.is_empty() {
                reply = reply.blank().line("Pending machine erasures").table(
                    &["MACHINE", "PENDING", "ACTION"],
                    erasures
                        .iter()
                        .map(|entry| {
                            vec![
                                entry.machine_id.to_string(),
                                entry.pending.to_string(),
                                if entry.operator_wipe {
                                    "Operator wipe"
                                } else {
                                    "Awaiting machine acknowledgement"
                                }
                                .to_owned(),
                            ]
                        })
                        .collect(),
                    "",
                );
            }
            Ok(if empty {
                reply.hint("Next: run `calternal machines code --help`.")
            } else {
                reply
            })
        }
        MachinesCmd::Revoke {
            installation,
            machine,
        } => {
            let id = uuid::Uuid::parse_str(&machine).map_err(|_| bad_uuid("machine"))?;
            let Response::Revoke { revoked } = run(installation, Request::Revoke { machine: id })?
            else {
                return Err(unexpected());
            };
            if revoked {
                Ok(Reply::new(json!({"machine": id, "revoked": true}))
                    .line(format!("Revoked machine {id}."))
                    .hint("Next: run `calternal machines list` to review machine access."))
            } else {
                Err(CliError::new("not_found", format!("no machine {id}"))
                    .hint("`calternal machines list` shows the machines and their ids.")
                    .human_message(format!("No machine with ID {id} was found.")))
            }
        }
        MachinesCmd::PoolGrant {
            installation,
            account: acct,
            remove,
        } => {
            let id = account(&acct)?;
            let granted = !remove;
            run(
                installation,
                Request::PoolGrant {
                    account: id,
                    granted,
                },
            )?;
            Ok(
                Reply::new(json!({"account": acct, "pool_access": granted})).line(format!(
                    "Account {acct} {} pool machines.",
                    if granted {
                        "may now use"
                    } else {
                        "may no longer use"
                    }
                )),
            )
        }
    }
}

/// `calternal plugin-digest`: the SHA-256 of each trusted worker bundle and
/// whether `trusted-plugins.json` pins it. Cost: reads and hashes each bundle once.
pub fn plugin_digest(root: &Path) -> Reply {
    let mut rows = Vec::new();
    let mut workers = Vec::new();
    for e in plugins::registry_entries() {
        let Some(w) = &e.worker else { continue };
        let (digest, state, pinned) = match plugins::file_sha256(&root.join(w)) {
            Ok(d) => {
                let pinned = e.worker_sha256.as_deref() == Some(d.as_str());
                (
                    Some(d),
                    if pinned { "pinned" } else { "NOT PINNED" }.to_owned(),
                    pinned,
                )
            }
            Err(why) => (None, format!("unreadable: {why}"), false),
        };
        rows.push(vec![
            e.id.to_string(),
            w.to_string(),
            digest.clone().unwrap_or_else(|| "-".into()),
            state.clone(),
        ]);
        workers.push(
            json!({"id": e.id, "worker": w, "sha256": digest, "pinned": pinned,
                            "state": state}),
        );
    }
    Reply::new(json!({ "workers": workers })).table(
        &["PLUGIN", "WORKER", "SHA-256", "STATE"],
        rows,
        "No trusted plugins with worker bundles were found. Check `--plugin-root`.",
    )
}
