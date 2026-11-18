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

/// Local operator commands for storage usage and resource ownership.
#[derive(Subcommand)]
pub enum ResourcesCmd {
    /// Read the current logical text and binary body bytes of a space.
    StorageUsage {
        #[command(flatten)]
        installation: Installation,
        /// Space UUID.
        space: String,
    },
    /// Set a reviewed package as the hosted admission hook for a space.
    SetHook {
        #[command(flatten)]
        installation: Installation,
        /// Space UUID.
        space: String,
        /// Installed package ID. Omit to clear the operator binding.
        package: Option<String>,
        #[arg(long, requires = "package")]
        /// Complete package review SHA-256.
        checksum: Option<String>,
        #[arg(long, requires = "package")]
        /// Dedicated space where this package keeps resource state.
        state_space: Option<String>,
    },
    /// Replace byte ceilings by kind for an existing space.
    SetStorage {
        #[command(flatten)]
        installation: Installation,
        /// Space UUID.
        space: String,
        #[arg(long)]
        /// Maximum text bytes. Omit for no ceiling.
        text_bytes: Option<u64>,
        #[arg(long)]
        /// Maximum binary bytes. Omit for no ceiling.
        binary_bytes: Option<u64>,
    },
    /// Read retained confirmed runtime of hosted guests for one scope.
    RuntimeUsage {
        #[command(flatten)]
        installation: Installation,
        #[arg(long, required_unless_present = "group", conflicts_with = "group")]
        /// Account UUID.
        account: Option<String>,
        #[arg(long, required_unless_present = "account", conflicts_with = "account")]
        /// Group UUID.
        group: Option<String>,
    },
    /// Set or clear manual hosted compute limits for a group or account.
    SetCompute {
        #[command(flatten)]
        installation: Installation,
        #[arg(long, required_unless_present = "group", conflicts_with = "group")]
        /// Account UUID for the override.
        account: Option<String>,
        #[arg(long, required_unless_present = "account", conflicts_with = "account")]
        /// Group UUID for the override.
        group: Option<String>,
        #[arg(long, required_unless_present = "clear", conflicts_with = "clear")]
        /// Allow or deny hosted compute with true or false.
        hosted: Option<bool>,
        #[arg(long, conflicts_with = "clear")]
        /// Required memory profile in MiB. Omit to accept any profile.
        memory_mib: Option<u32>,
        #[arg(long, conflicts_with = "clear")]
        /// Optional hard cap on active threads per guest.
        active_threads: Option<u32>,
        #[arg(long, conflicts_with = "clear")]
        /// Optional bound on waiting leases.
        queue_bound: Option<u32>,
        #[arg(long, conflicts_with = "clear")]
        /// Hosted guest idle timeout in seconds.
        idle_seconds: Option<u32>,
        #[arg(long, requires = "reset_seconds", conflicts_with = "clear")]
        /// Confirmed sandbox seconds allowed per reset window.
        runtime_seconds: Option<u32>,
        #[arg(long, requires = "runtime_seconds", conflicts_with = "clear")]
        /// Window length in seconds from the Unix epoch.
        reset_seconds: Option<u32>,
        #[arg(long)]
        /// Clear the override for this scope.
        clear: bool,
    },
    /// Choose which group owns a shared space's resource usage.
    SetGroup {
        #[command(flatten)]
        installation: Installation,
        /// Shared space UUID.
        space: String,
        /// Group UUID. Omit it to clear the resource binding.
        group: Option<String>,
    },
}

fn resource_owner(
    account: Option<String>,
    group: Option<String>,
) -> Result<calternal_types::ResourceOwner, CliError> {
    use calternal_types::ResourceOwner;
    match (account, group) {
        (Some(id), None) => Ok(ResourceOwner::Account(
            calternal_types::AccountId::parse(&id).map_err(|_| bad_uuid("account"))?,
        )),
        (None, Some(id)) => Ok(ResourceOwner::Group(
            calternal_types::GroupId::parse(&id).map_err(|_| bad_uuid("group"))?,
        )),
        _ => Err(CliError::usage("pass either --account or --group")),
    }
}

/// Run an owner-only resource command through the local operator socket or
/// the guarded stopped installation. Costs one authority read or commit.
pub fn resources(cmd: ResourcesCmd) -> Result<Reply, CliError> {
    let parse_space = |s: &str| calternal_types::SpaceId::parse(s).map_err(|_| bad_uuid("space"));
    match cmd {
        ResourcesCmd::StorageUsage {
            installation,
            space,
        } => {
            let space = parse_space(&space)?;
            let Response::StorageUsage { usage, owner } =
                run(installation, Request::StorageUsage { space })?
            else {
                return Err(unexpected());
            };
            let owner_label = match owner {
                Some(calternal_auth::machines::ResourceOwner::Account(id)) => {
                    format!("account {id}")
                }
                Some(calternal_auth::machines::ResourceOwner::Group(id)) => format!("group {id}"),
                None => "unassigned".into(),
            };
            Ok(Reply::new(json!({
                "space_id": space, "owner": owner, "text_bytes": usage.text_bytes, "binary_bytes": usage.binary_bytes
            })).fields([
                ("space", space.to_string()), ("owner", owner_label),
                ("text bytes", usage.text_bytes.to_string()), ("binary bytes", usage.binary_bytes.to_string()),
            ]))
        }
        ResourcesCmd::SetHook {
            installation,
            space,
            package,
            checksum,
            state_space,
        } => {
            let space = parse_space(&space)?;
            let state_space = state_space.as_deref().map(parse_space).transpose()?;
            let binding = match (package, checksum) {
                (Some(package), Some(checksum)) => {
                    Some(calternal_server::machines::ResourceHookBinding {
                        package,
                        checksum,
                        state_space,
                    })
                }
                (None, None) => None,
                _ => {
                    return Err(CliError::usage(
                        "pass --checksum from the complete package review",
                    ));
                }
            };
            let Response::ResourceHook = run(
                installation,
                Request::ResourceHook {
                    space,
                    binding: binding.clone(),
                },
            )?
            else {
                return Err(unexpected());
            };
            Ok(Reply::new(json!({ "space_id": space, "binding": binding }))
                .line("Saved the resource admission hook binding."))
        }
        ResourcesCmd::SetStorage {
            installation,
            space,
            text_bytes,
            binary_bytes,
        } => {
            let space = parse_space(&space)?;
            let limits = calternal_content::StorageLimits {
                text_bytes,
                binary_bytes,
            };
            let Response::StorageLimits =
                run(installation, Request::StorageLimits { space, limits })?
            else {
                return Err(unexpected());
            };
            Ok(Reply::new(json!({ "space_id": space, "limits": limits }))
                .line("Saved storage byte ceilings."))
        }
        ResourcesCmd::RuntimeUsage {
            installation,
            account,
            group,
        } => {
            let owner = resource_owner(account, group)?;
            let Response::RuntimeUsage { usage } =
                run(installation, Request::RuntimeUsage { owner })?
            else {
                return Err(unexpected());
            };
            Ok(
                Reply::new(json!({ "owner": owner, "usage": usage })).fields([
                    ("confirmed seconds", usage.running_seconds.to_string()),
                    ("unresolved generations", usage.unresolved.to_string()),
                    ("since", Output::iso_ms(usage.since_ms)),
                ]),
            )
        }
        ResourcesCmd::SetCompute {
            installation,
            account,
            group,
            hosted,
            memory_mib,
            active_threads,
            queue_bound,
            idle_seconds,
            runtime_seconds,
            reset_seconds,
            clear,
        } => {
            let owner = resource_owner(account, group)?;
            let limits = if clear {
                None
            } else {
                Some(calternal_server::machines::ComputeLimits {
                    hosted: hosted
                        .ok_or_else(|| CliError::usage("pass --hosted true or --hosted false"))?,
                    memory_mib,
                    active_threads,
                    queue_bound,
                    idle_seconds,
                    runtime_seconds,
                    reset_seconds,
                })
            };
            let Response::ComputeLimits =
                run(installation, Request::ComputeLimits { owner, limits })?
            else {
                return Err(unexpected());
            };
            Ok(
                Reply::new(json!({ "owner": owner, "limits": limits })).line(if clear {
                    "Cleared manual compute limits."
                } else {
                    "Saved manual compute limits."
                }),
            )
        }
        ResourcesCmd::SetGroup {
            installation,
            space,
            group,
        } => {
            let space = parse_space(&space)?;
            let group = group
                .map(|id| calternal_types::GroupId::parse(&id).map_err(|_| bad_uuid("group")))
                .transpose()?;
            let Response::ResourceGroup =
                run(installation, Request::ResourceGroup { space, group })?
            else {
                return Err(unexpected());
            };
            Ok(
                Reply::new(json!({ "space_id": space, "group_id": group })).line(match group {
                    Some(group) => format!("Group {group} owns resources for space {space}."),
                    None => format!("Cleared the resource group for space {space}."),
                }),
            )
        }
    }
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
    let (store, svc) =
        admin::open_offline(&roots, id).map_err(|e| installation_error(e.message))?;
    admin::execute(&svc, store.content(), request).map_err(|e| installation_error(e.message))
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
