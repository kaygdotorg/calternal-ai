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
    #[command(flatten)]
    target: Target,
    /// Activate one build and exit.
    #[arg(long)]
    once: bool,
}

#[derive(clap::Args)]
struct Target {
    /// Protected appdata directory of the running development server.
    #[arg(long, env = "APPDATA", num_args = 0..=1, default_missing_value = "")]
    appdata: Option<OsString>,
    /// Development space UUID. Defaults to SPACE_ID.
    #[arg(long, num_args = 0..=1, default_missing_value = "")]
    space: Option<OsString>,
    /// Enrolled account with access to manage plugins in that space.
    #[arg(long, env = "ACCOUNT_ID", num_args = 0..=1, default_missing_value = "")]
    account: Option<OsString>,
}

#[derive(clap::Args)]
pub(crate) struct DiagnosticsArgs {
    /// ID of the currently loaded development plugin.
    id: String,
    /// Declared operation to inspect.
    operation: String,
    #[command(flatten)]
    target: Target,
}

struct Draft {
    dir: PathBuf,
    appdata: PathBuf,
    space: SpaceId,
    space_source: &'static str,
    account: AccountId,
    once: bool,
}

fn target(args: Target) -> Result<(PathBuf, SpaceId, &'static str, AccountId), CliError> {
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
    let (space, space_source) = match args.space {
        Some(value) => (Some(value), "--space"),
        None => (
            std::env::var_os("SPACE_ID"),
            "SPACE_ID environment variable",
        ),
    };
    let space = space
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
    Ok((appdata.into(), space, space_source, account))
}

fn draft(args: Args) -> Result<Draft, CliError> {
    let (appdata, space, space_source, account) = target(args.target)?;
    Ok(Draft {
        dir: args.dir,
        appdata,
        space,
        space_source,
        account,
        once: args.once,
    })
}

pub(super) fn diagnostics(args: DiagnosticsArgs) -> Result<Reply, CliError> {
    let (appdata, space, space_source, account) = target(args.target)?;
    let value = live_catalog::diagnostics(&appdata, args.id.clone(), args.operation.clone(), space, account)
        .map_err(|message| CliError::new("plugin_diagnostics", format!("{message} (space {space} from {space_source})"))
            .hint("Use the development Manager and a declared operation of the loaded draft in that space."))?;
    let revision = value["ready_revision"]
        .as_u64()
        .map_or_else(|| "none loaded".to_owned(), |r| r.to_string());
    let timing = &value["timing"];
    let ms = |key: &str| {
        timing[key]
            .as_u64()
            .map_or_else(|| "n/a".to_owned(), |v| format!("{v} ms"))
    };
    let rows = value["errors"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            vec![
                e["reason"].as_str().unwrap_or_default().to_owned(),
                e["path"].as_str().unwrap_or_default().to_owned(),
                e["elapsed_ms"].to_string(),
            ]
        })
        .collect();
    let failed = &value["last_failed_candidate"];
    let failed_text = failed["phase"].as_str().map_or_else(
        || "none".to_owned(),
        |phase| {
            format!(
                "{phase}: {}",
                failed["message"].as_str().unwrap_or_default()
            )
        },
    );
    Ok(Reply::new(value.clone())
        .fields([
            ("Plugin", args.id),
            ("Operation", args.operation),
            ("Space", space.to_string()),
            ("State", value["phase"].as_str().unwrap_or("unknown").to_owned()),
            ("Ready revision", revision),
            ("Validate", ms("validation_ms")),
            ("Handshake", ms("handshake_ms")),
            ("Publish", ms("publication_ms")),
            ("Last failed candidate", failed_text),
        ])
        .table(
            &["ERROR", "FIELD PATH", "MS"],
            rows,
            "No recent errors for this caller and operation.",
        )
        .hint("Compare each field path with the plugin's input or output schema. Edit the source to reload."))
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
        CliError::new(
            "plugin_draft",
            format!("{message} (space {} from {})", args.space, args.space_source),
        )
        .details(serde_json::json!({"space_id":args.space,"space_source":args.space_source}))
        .hint("Check that APPDATA and the space and account UUIDs come from the same development server.")
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
        "id":id, "status":"ready", "space_id":args.space, "space_source":args.space_source,
        "build_ms":build_ms, "activation_ms":activation_ms
    }))
    .fields([("Plugin", id.as_str()), ("Status", "Ready")])
    .fields([("Space", format!("{} ({})", args.space, args.space_source))])
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
