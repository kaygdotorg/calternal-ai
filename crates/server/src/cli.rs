//! Command definitions shared by the parser and startup dispatch.
use crate::admin_cli::MachinesCmd;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "calternal",
    about = "Run a Calternal server or manage its enrolled machines",
    after_help = "Examples:\n  calternal serve --appdata ./data/appdata --userdata-ssd ./data/ssd --userdata-hdd ./data/hdd --development\n  calternal machines list --appdata /srv/calternal/appdata\n\nSee also: calternal serve --help, calternal machine --help, calternal machines --help."
)]
/// Parsed server and operator commands; validation precedes side effects.
pub(super) struct Cli {
    /// Print one JSON document instead of text (errors too, on stdout); see docs/cli.md.
    #[arg(long, global = true)]
    pub(super) json: bool,
    /// Hide the short next-step line shown after successful terminal output.
    #[arg(
        long,
        global = true,
        help = "Hide terminal hints (also CALTERNAL_NO_HINTS=1)"
    )]
    pub(super) no_hints: bool,
    #[command(subcommand)]
    pub(super) cmd: Cmd,
}

#[derive(Subcommand)]
/// Supported commands; handlers enforce current authority and durable storage.
pub(super) enum Cmd {
    /// Create, check, test and benchmark TypeScript plugins.
    Plugin {
        #[command(subcommand)]
        command: crate::plugin_cli::PluginCommand,
    },
    /// Create, read, send to, stop, archive and delete saved conversations.
    Threads {
        #[command(flatten)]
        args: calternal_cli::threads::Args,
    },
    /// Search readable spaces and browse palette suggestions.
    Search {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::search::Command,
    },
    /// Find, rename, delete and restore notes.
    Notes {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::notes::Notes,
    },
    /// List and look up note tags in a space.
    Tags {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::notes::Tags,
    },
    /// List or permanently delete notes in Trash.
    Trash {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::notes::Trash,
    },
    /// Run, watch and control official provider sessions in machine sandboxes.
    Agent {
        #[command(flatten)]
        args: calternal_cli::agent::Args,
    },
    /// Invoke a discovered operation with structured JSON input.
    #[command(
        after_help = "With a thread credential, invoke access.request with an app and optional write flag. The credential supplies the job ID. People must supply job_id."
    )]
    Invoke {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
        operation: String,
        #[arg(long, default_value = "{}")]
        input: String,
    },
    /// Discover available operations using the current person or thread credential.
    Describe {
        #[command(flatten)]
        connection: calternal_cli::agent::Connection,
    },
    /// Read or record onboarding completion for the signed-in account.
    Onboarding {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::onboarding::Onboarding,
    },
    /// Save, read and erase private account compositions.
    Drafts {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::account_input::Drafts,
    },
    /// Queue, edit, confirm, submit or cancel private pending input.
    Pending {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::account_input::Pending,
    },
    /// Manage space grants, role definitions, profiles, and server people.
    People {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::people::People,
    },
    /// Inspect usage and assign a shared space's resource group.
    Resources {
        #[command(subcommand)]
        cmd: crate::admin_cli::ResourcesCmd,
    },
    /// List and manage server groups and their members.
    Groups {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::people::Groups,
    },
    /// Create, copy, revoke and redeem space invitations.
    Invites {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::people::Invites,
    },
    /// List, answer, approve or deny the requests waiting on you.
    Requests {
        #[command(flatten)]
        connection: calternal_cli::account_input::Connection,
        #[command(subcommand)]
        cmd: calternal_cli::requests::Command,
    },
    /// Follow a space journal; --json prints one compact JSON object per event.
    Watch {
        space: String,
        #[arg(long, env = "CALTERNAL_URL", default_value = "http://127.0.0.1:3210")]
        base_url: String,
        #[arg(long, env = "CALTERNAL_TOKEN_FILE")]
        token_file: PathBuf,
        #[arg(
            long,
            help = "Bookmark cache directory exclusive to this server and account"
        )]
        bookmarks: Option<PathBuf>,
    },
    /// Issue a fresh first-owner code through the running server.
    SetupCode {
        /// The running server's appdata directory.
        #[arg(long)]
        appdata: PathBuf,
        /// The installation UUID configured for the running server.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: String,
    },
    /// Issue an enrollment grant after verifying an account owner's identity.
    AccountRecover {
        /// Account UUID from the operator directory.
        account: String,
        /// Operator audit reason: lost_passkeys, compromised, or mail_unavailable.
        #[arg(long)]
        reason: String,
        /// Confirms that the operator verified the caller's identity out of band.
        #[arg(long, required = true)]
        identity_verified: bool,
        /// The running server's appdata directory.
        #[arg(long)]
        appdata: PathBuf,
        /// The installation UUID configured for the running server.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: String,
    },
    /// Send a test message through the running server's account mail service.
    MailTest {
        /// Recipient mailbox for the test message.
        recipient: String,
        /// The running server's appdata directory.
        #[arg(long)]
        appdata: PathBuf,
        /// The installation UUID configured for the running server.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: String,
    },
    /// Inspect receipt-pruning health for one space through the private admin socket.
    ReceiptStatus {
        /// Space UUID to inspect.
        space: String,
        /// The running server's appdata directory.
        #[arg(long)]
        appdata: PathBuf,
        /// The installation UUID configured for the running server.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: String,
    },
    /// Safely prepare, inspect, or recover a data upgrade with both roles stopped.
    Upgrade {
        #[command(flatten)]
        command: crate::upgrade_cli::UpgradeArgs,
    },
    /// Serve Calternal. Listens on loopback by default.
    #[command(
        about = "Start the development-preview server",
        after_help = "Examples:\n  calternal serve --appdata ./data/appdata --userdata-ssd ./data/ssd --userdata-hdd ./data/hdd --development\n  calternal serve --appdata /srv/calternal/appdata --userdata-ssd /srv/calternal/ssd --userdata-hdd /mnt/hdd/calternal --port 3210\n\nSee also: calternal machines --help, calternal machine --help."
    )]
    Serve {
        /// Protected databases and state (normally on SSD).
        #[arg(long)]
        appdata: PathBuf,
        /// Space trees of claim-validated text files (normally on SSD).
        #[arg(long)]
        userdata_ssd: PathBuf,
        /// Space trees of other original files (normally on HDD).
        #[arg(long)]
        userdata_hdd: PathBuf,
        /// First setup: pair three empty or missing roots. Without it every root must
        /// already belong to this installation; harmless on a paired installation.
        #[arg(long)]
        create: bool,
        /// This installation's UUID, kept in the server's configuration outside every
        /// root (ADR 0062). Each root's marker must carry it; required.
        #[arg(long, env = "CALTERNAL_INSTALLATION_ID")]
        installation_id: Option<String>,
        /// Days each space keeps request receipts; a retry after that runs as a new
        /// request.
        #[arg(long, env = "CALTERNAL_RECEIPT_RETENTION_DAYS", default_value_t = 7)]
        receipt_retention_days: u32,
        /// Days to retain an invitation after expiry, revocation or redemption.
        #[arg(long, env = "CALTERNAL_INVITE_RETENTION_DAYS",
            default_value_t = calternal_auth::invites::DEFAULT_INVITE_RETENTION_DAYS,
            value_parser = clap::value_parser!(u32).range(1..))]
        invite_retention_days: u32,
        /// Bind address. Use a proxy and network access controls for non-loopback listeners.
        #[arg(long, default_value = "127.0.0.1")]
        bind_ip: std::net::IpAddr,
        /// Listen port; 0 picks a free port (see startup.json).
        #[arg(long, default_value_t = calternal_core::DEFAULT_PORT)]
        port: u16,
        /// Built web assets served same-origin (any static directory).
        #[arg(long)]
        web_dir: Option<PathBuf>,
        /// Enable disposable development accounts and bearer token files (off by default).
        #[arg(long)]
        development: bool,
        /// Exact public WebAuthn origin; defaults to `http://localhost:<bound-port>`.
        #[arg(long, env = "CALTERNAL_ORIGIN")]
        origin: Option<String>,
        /// Comma-separated trusted proxy CIDRs; forwarded headers are ignored by default.
        #[arg(long, env = "CALTERNAL_TRUSTED_PROXIES", default_value = "")]
        trusted_proxies: String,
        /// Root of trusted plugins. Defaults to ./plugins with the shipped Threads manifest available.
        #[arg(long)]
        plugin_root: Option<PathBuf>,
        /// Explicit direct development worker runtime; requires --development.
        /// Shipped servers route workers to enrolled machines.
        #[arg(long)]
        node: Option<PathBuf>,
        /// Root of the test-only plugins (`tests/plugins`); refused unless the server
        /// was built with the `test-plugins` feature.
        #[arg(long)]
        test_plugin_root: Option<PathBuf>,
        /// How long a machine's leases survive without a sync, in milliseconds.
        #[arg(long, default_value_t = 90_000)]
        machine_lease_ttl_ms: u64,
        /// Longest hold of a machine's long-poll, in milliseconds.
        #[arg(long, default_value_t = 20_000)]
        machine_poll_hold_ms: u64,
        /// CPU ceiling for each agent sandbox, in millicores. Worker limits are separate.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
        machine_sandbox_cpu_millis: Option<u32>,
    },
    /// Machine role: enroll this computer and run threads for a server.
    #[command(
        about = "Enroll this computer and run assigned threads",
        after_help = "Examples:\n  calternal machine enroll --help\n  calternal machine status\n\nSee also: calternal machines code, calternal machine enroll."
    )]
    Machine {
        #[command(subcommand)]
        cmd: calternal_machine::cli::Command,
    },
    /// Operator administration of execution machines in a data directory.
    #[command(
        about = "Manage machines enrolled with this server",
        after_help = "Examples:\n  calternal machines list --appdata /srv/calternal/appdata\n\nSee also: calternal machines code, calternal machine --help."
    )]
    Machines {
        #[command(subcommand)]
        cmd: MachinesCmd,
    },
    /// Print SHA-256 of trusted worker bundles for pinning in trusted-plugins.json.
    #[command(
        about = "Show worker bundle hashes and their trust status",
        after_help = "Examples:\n  calternal plugin-digest\n  calternal plugin-digest --plugin-root ./plugins\n\nSee also: calternal serve --help."
    )]
    PluginDigest {
        /// Directory containing the trusted plugin bundles.
        #[arg(long, default_value = "plugins")]
        plugin_root: PathBuf,
    },
}
