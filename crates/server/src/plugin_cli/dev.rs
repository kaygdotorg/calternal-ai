//! Local development watch loop. The server validates and activates each build.
use super::{Change, CliError, Package, Reply, check, live_catalog, local, npm};
use calternal_cli::output::Output;
use calternal_types::{AccountId, SpaceId};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime},
};

#[derive(clap::Args)]
pub(crate) struct Args {
    /// Source directory created by calternal plugin new.
    dir: PathBuf,
    /// Protected appdata directory of the running development server.
    #[arg(long)]
    appdata: PathBuf,
    /// Development space UUID.
    #[arg(long)]
    space: SpaceId,
    /// Enrolled account with access to manage plugins in that space.
    #[arg(long)]
    account: AccountId,
    /// Activate one build and exit.
    #[arg(long)]
    once: bool,
}

fn activate(args: &Args) -> Result<Reply, CliError> {
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
