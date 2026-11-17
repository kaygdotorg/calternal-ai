//! The operator commands of the `calternal` binary: `machines …` (execution
//! machines in a data directory) and `plugin-digest`. Each builds a
//! [`Reply`]; printing, `--json`, errors and exit codes belong to
//! [`calternal_cli::output`] (reference: `docs/cli.md`).

use std::path::{Path, PathBuf};

use calternal_cli::output::{CliError, Output, Reply};
use calternal_machine::cli::owner_label;
use calternal_machine::protocol::Owner;
use clap::Subcommand;
use serde_json::json;

use calternal_server::machines::admin;
use calternal_server::plugins;

/// `calternal machines …`: operator administration of execution machines.
#[derive(Subcommand)]
pub enum MachinesCmd {
    /// Print a one-time enrollment code (for an account, or the shared pool).
    Code {
        #[arg(long)]
        data_dir: PathBuf,
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
    /// List enrolled machines.
    List {
        #[arg(long)]
        data_dir: PathBuf,
    },
    /// Revoke a machine; a running server applies it within a second.
    Revoke {
        #[arg(long)]
        data_dir: PathBuf,
        /// Machine UUID.
        machine: String,
    },
    /// Allow (or with --remove, stop allowing) an account to use pool machines.
    PoolGrant {
        #[arg(long)]
        data_dir: PathBuf,
        /// Account UUID.
        account: String,
        #[arg(long)]
        remove: bool,
    },
}

fn data_dir_error(message: impl Into<String>) -> CliError {
    CliError::new("failed", message)
        .hint("Check that --data-dir is the server's data directory and that you can write to it.")
}

fn bad_uuid(what: &str) -> CliError {
    CliError::new("invalid_input", format!("not a valid {what} UUID")).hint(format!(
        "Pass the {what}'s UUID, for example 123e4567-e89b-42d3-a456-426614174000."
    ))
}

/// Run one `machines` subcommand. Cost: opens `<data>/system/machines.db` once.
pub fn machines(cmd: MachinesCmd, out: &Output) -> Result<Reply, CliError> {
    let account = |s: &str| calternal_auth::AccountId::parse(s).ok_or_else(|| bad_uuid("account"));
    match cmd {
        MachinesCmd::Code {
            data_dir,
            pool,
            account: acct,
            minutes,
        } => {
            let owner = match (pool, acct) {
                (true, _) => Owner::Pool,
                (false, Some(a)) => Owner::Account { id: account(&a)?.0 },
                (false, None) => {
                    return Err(CliError::usage("pass --pool or --account <uuid>")
                        .hint("--pool makes a machine the operator runs; --account makes one a person owns."));
                }
            };
            let ttl = std::time::Duration::from_secs(minutes * 60);
            let (code, expires) =
                admin::issue_code(&data_dir, owner, ttl).map_err(|e| data_dir_error(e.message))?;
            Ok(Reply::new(json!({
                "code": code,
                "owner": owner_label(owner),
                "expires_at": Output::iso_ms(expires),
            }))
            .fields([
                ("code", code),
                ("owner", owner_label(owner)),
                ("expires", out.local_ms(expires)),
            ])
            .blank()
            .line("The code works once. Give it to the machine's owner now; it is not shown again.")
            .line("On that machine: calternal machine enroll --server URL --code CODE"))
        }
        MachinesCmd::List { data_dir } => {
            let machines = admin::list(&data_dir).map_err(|e| data_dir_error(e.message))?;
            let state = |m: &calternal_server::machines::db::MachineRow| {
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
            Ok(Reply::new(json!({ "machines": listed })).table(
                &["ID", "OWNER", "NAME", "PLATFORM", "STATE"],
                rows,
                "No machines are enrolled. Create a code with `calternal machines code`.",
            ))
        }
        MachinesCmd::Revoke { data_dir, machine } => {
            let id = uuid::Uuid::parse_str(&machine).map_err(|_| bad_uuid("machine"))?;
            if admin::revoke(&data_dir, id).map_err(|e| data_dir_error(e.message))? {
                Ok(
                    Reply::new(json!({"machine": id, "revoked": true})).line(format!(
                        "Revoked machine {id}. A running server applies it within a second."
                    )),
                )
            } else {
                Err(CliError::new("not_found", format!("no machine {id}")).hint(
                    "`calternal machines list --data-dir DIR` shows the machines and their ids.",
                ))
            }
        }
        MachinesCmd::PoolGrant {
            data_dir,
            account: acct,
            remove,
        } => {
            let id = account(&acct)?;
            admin::pool_grant(&data_dir, id, !remove).map_err(|e| data_dir_error(e.message))?;
            let granted = !remove;
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
        "No trusted plugin has a worker bundle.",
    )
}
