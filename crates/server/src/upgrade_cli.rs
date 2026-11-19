//! Local deployment commands over the guarded server installation.
use std::path::PathBuf;

use calternal_cli::output::{CliError, Reply};
use calternal_roots::Roots;
use calternal_server::upgrades::{self, Action};
use clap::{Args, Subcommand};

use crate::admin_cli::Installation;

/// Operator arguments shared by the deployment steps.
#[derive(Args)]
pub(super) struct UpgradeArgs {
    #[command(flatten)]
    installation: Installation,
    /// State directory of a co-located machine role, if the deployment has one.
    #[arg(long)]
    machine_dir: Option<PathBuf>,
    #[command(subcommand)]
    step: Step,
}

#[derive(Subcommand)]
enum Step {
    /// Check the exact release pair and snapshot footprint without stopping service.
    Preflight(ReleaseArgs),
    /// Capture pre-upgrade state, then migrate and verify the selected release.
    Prepare(ReleaseArgs),
    /// Publish the cutover decision before admitting public writes.
    Commit {
        /// Exact candidate image digest from the prepared intent.
        #[arg(long)]
        candidate: String,
    },
    /// Inspect the durable decision.
    Status,
    /// Restore a pending upgrade before either release serves requests.
    Recover,
    /// Verify that this binary understands the current protected state.
    Verify,
}

#[derive(Args)]
struct ReleaseArgs {
    /// Digest of the current image.
    #[arg(long)]
    previous: String,
    /// Digest of the candidate image.
    #[arg(long)]
    candidate: String,
    /// Use a full restore point with the owner's explicit go.
    #[arg(long, conflicts_with = "proof")]
    incompatible: bool,
    /// Compatibility evidence for the exact two images.
    #[arg(long)]
    proof: Option<PathBuf>,
}

pub(super) fn run(command: UpgradeArgs) -> Result<Reply, CliError> {
    let preflight = matches!(command.step, Step::Preflight(_));
    let error = |error: std::io::Error| {
        let hint = if preflight {
            "Resolve the named refusal before stopping services or retrying the deploy."
        } else {
            "Use calternal upgrade status to inspect the deployment before starting either role."
        };
        CliError::new("upgrade_failed", error.to_string()).hint(hint)
    };
    let id = uuid::Uuid::parse_str(&command.installation.installation_id)
        .map_err(|_| CliError::new("invalid_input", "Use this server's installation UUID."))?;
    let roots = Roots {
        appdata: command.installation.appdata,
        userdata_ssd: command.installation.userdata_ssd,
        userdata_hdd: command.installation.userdata_hdd,
    };
    let (action, label) = match command.step {
        Step::Preflight(ReleaseArgs {
            previous,
            candidate,
            incompatible,
            proof,
        }) => (
            Action::Preflight {
                previous,
                candidate,
                incompatible,
                proof,
            },
            "Upgrade preflight passed",
        ),
        Step::Prepare(ReleaseArgs {
            previous,
            candidate,
            incompatible,
            proof,
        }) => (
            Action::Prepare {
                previous,
                candidate,
                incompatible,
                proof,
            },
            "Prepared upgrade",
        ),
        Step::Commit { candidate } => (Action::Commit { candidate }, "Committed upgrade"),
        Step::Status => (Action::Status, "Upgrade status"),
        Step::Recover => (Action::Recover, "Recovered upgrade"),
        Step::Verify => (Action::Verify, "Data versions verified"),
    };
    let result = upgrades::run(roots, command.machine_dir, id, action).map_err(error)?;
    let fields = result
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| {
            (!value.is_null()).then(|| {
                let text = value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string());
                (key.replace('_', " "), text)
            })
        })
        .collect::<Vec<_>>();
    Ok(Reply::new(result).line(label).fields(fields))
}
