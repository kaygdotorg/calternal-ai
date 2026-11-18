//! Local development watch loop. The server validates and activates each build.
use super::{Change, CliError, Package, Reply, check, live_catalog, local, npm};
use calternal_cli::output::Output;
use calternal_types::{AccountId, SpaceId};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Source directory created by calternal plugin new.
    dir: PathBuf,
    /// Protected appdata directory of the running development server.
    #[arg(long, env = "APPDATA", num_args = 0..=1, default_missing_value = "")]
    appdata: Option<OsString>,
    /// Development space UUID.
    #[arg(long, env = "SPACE_ID", num_args = 0..=1, default_missing_value = "")]
    space: Option<OsString>,
    /// Enrolled account with access to manage plugins in that space.
    #[arg(long, env = "ACCOUNT_ID", num_args = 0..=1, default_missing_value = "")]
    account: Option<OsString>,
    /// Activate one build and exit.
    #[arg(long)]
    once: bool,
}

struct Draft {
    dir: PathBuf,
    appdata: PathBuf,
    space: SpaceId,
    account: AccountId,
    once: bool,
}

fn draft(args: Args) -> Result<Draft, CliError> {
    let hint = "Source your development server's env file in this shell, or pass --appdata, --space and --account explicitly.";
    let appdata = args
        .appdata
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CliError::usage(
                "Pass --appdata PATH or set APPDATA to the development server's appdata directory.",
            )
            .hint(hint)
        })?;
    let space = args
        .space
        .as_deref()
        .and_then(|value| value.to_str())
        .and_then(|value| SpaceId::parse(value).ok())
        .ok_or_else(|| {
            CliError::usage("Pass --space UUID or set SPACE_ID to the development space UUID.")
                .hint(hint)
        })?;
    let account = args
        .account
        .as_deref()
        .and_then(|value| value.to_str())
        .and_then(|value| AccountId::parse(value).ok())
        .ok_or_else(|| {
            CliError::usage(
                "Pass --account UUID or set ACCOUNT_ID to the development account UUID.",
            )
            .hint(hint)
        })?;
    Ok(Draft {
        dir: args.dir,
        appdata: appdata.into(),
        space,
        account,
        once: args.once,
    })
}

fn activate(args: &Draft) -> Result<Reply, CliError> {
    let start = Instant::now();
    check(&args.dir)?;
    let script = fs::File::open(args.dir.join("package.json")).map_err(local)?;
    let manifest: serde_json::Value =
        serde_json::from_reader(std::io::Read::take(script, 256 * 1024))
            .map_err(|error| CliError::new("plugin_draft", format!("package.json: {error}")))?;
    let build = if manifest["scripts"]["calternal:dev"].is_string() {
        "calternal:dev"
    } else {
        "build"
    };
    npm(&args.dir, &["run", build, "--silent"])?;
    let draft_error = |message| {
        CliError::new("plugin_draft", message).hint(
            "Check the package and the running development server's appdata, space and account.",
        )
    };
    let built = Instant::now();
    let package = Package::read(&args.dir).map_err(draft_error)?;
    let id = package.manifest.id.clone();
    live_catalog::request(
        &args.appdata,
        Change::Draft {
            package,
            space: args.space,
            account: args.account,
        },
    )
    .map_err(draft_error)?;
    let build_ms = built.duration_since(start).as_secs_f64() * 1000.0;
    let activation_ms = built.elapsed().as_secs_f64() * 1000.0;
    Ok(Reply::new(serde_json::json!({
        "id":id, "status":"ready", "space_id":args.space,
        "build_ms":build_ms, "activation_ms":activation_ms
    }))
    .fields([("Plugin", id.as_str()), ("Status", "Ready")])
    .fields([
        ("Build", format!("{build_ms:.0} ms")),
        ("Activate", format!("{activation_ms:.0} ms")),
    ])
    .hint("Invoke the plugin's operation in this space. Edit the source to reload."))
}

pub(super) fn run(args: Args, out: &Output) -> i32 {
    let args = match draft(args) {
        Ok(args) => args,
        Err(error) => return out.finish(Err(error)),
    };
    if args.once {
        return out.finish(activate(&args));
    }
    let mut observed = None;
    loop {
        let mut files = BTreeMap::new();
        let mut budget = 4096;
        if let Err(error) = scan(&args.dir, 0, &mut budget, &mut files) {
            return out.finish(Err(error));
        }
        if observed.as_ref() != Some(&files) {
            observed = Some(files);
            let status = match activate(&args) {
                Ok(reply) => out.event(&reply),
                Err(error) => out.event_error(&error),
            };
            if status != 0 {
                return status;
            }
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn scan(
    path: &Path,
    depth: usize,
    budget: &mut usize,
    files: &mut BTreeMap<PathBuf, (SystemTime, u64)>,
) -> Result<(), CliError> {
    if depth > 16 {
        return Err(CliError::new(
            "plugin_draft",
            "Plugin source exceeds 16 directory levels.",
        ));
    }
    for entry in fs::read_dir(path).map_err(local)? {
        let entry = entry.map_err(local)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.')
            || matches!(
                name.as_ref(),
                "node_modules" | "dist" | "manifest.generated.ts"
            )
        {
            continue;
        }
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| CliError::new("plugin_draft", "Plugin source exceeds 4096 entries."))?;
        let file_type = entry.file_type().map_err(local)?;
        if file_type.is_dir() {
            scan(&entry.path(), depth + 1, budget, files)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata().map_err(local)?;
            files.insert(
                entry.path(),
                (metadata.modified().map_err(local)?, metadata.len()),
            );
        }
    }
    Ok(())
}
