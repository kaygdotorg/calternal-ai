//! Local TypeScript author commands. Author scripts run as the invoking user.
use calternal_cli::output::{CliError, Reply};
use calternal_plugin_host::MAX_MANIFEST_BYTES;
use calternal_server::plugins::{
    live_catalog::{self, Change},
    package_store::{self, Package},
};
use clap::Subcommand;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

#[cfg(feature = "dev-plugin-drafts")]
mod dev;
mod question;
mod scaffold;
mod submission;

#[derive(Subcommand)]
pub(super) enum PluginCommand {
    /// Show the package's operation and broker permissions and approval checksum.
    Review {
        /// Built plugin directory or .calplugin archive.
        source: PathBuf,
        /// Compare the declarations against this server's installed package.
        #[arg(long)]
        appdata: Option<PathBuf>,
    },
    /// Make a portable package archive from a built plugin directory.
    Pack {
        /// Built plugin directory.
        source: PathBuf,
        /// New archive path, usually ending in .calplugin.
        archive: PathBuf,
    },
    /// Create an offline gallery submission from a checked plugin directory.
    Package {
        /// Built plugin directory containing gallery.json and screenshots.
        source: PathBuf,
        /// New gallery submission archive path.
        archive: PathBuf,
    },
    /// Check a gallery submission without running its worker or contacting any service.
    VerifyPackage {
        /// Gallery submission archive to validate.
        archive: PathBuf,
    },
    /// Install the exact reviewed package on the running server.
    Install {
        #[command(flatten)]
        args: InstallArgs,
    },
    /// Replace an installed package with the exact reviewed package.
    Update {
        #[command(flatten)]
        args: InstallArgs,
    },
    /// Uninstall a package from the running server.
    Remove {
        /// Installed plugin ID.
        id: String,
        /// Protected server appdata directory on this host.
        #[arg(long)]
        appdata: PathBuf,
    },
    /// Enable an installed package in one space.
    Enable {
        #[command(flatten)]
        args: ActivationArgs,
    },
    /// Disable an installed package in one space.
    Disable {
        #[command(flatten)]
        args: ActivationArgs,
    },
    /// Create a TypeScript plugin with a view or a durable Inbox question.
    New {
        /// Lowercase package ID and new directory name.
        name: String,
        /// Calternal source checkout with a built TypeScript SDK.
        #[arg(long, env = "CALTERNAL_SDK_ROOT", default_value = ".")]
        sdk_root: PathBuf,
        /// Choose a generated view or a job that waits for an Inbox answer.
        #[arg(long, value_enum, default_value = "view")]
        template: scaffold::Template,
    },
    /// Build and reload a plugin while its development server stays running.
    #[cfg(feature = "dev-plugin-drafts")]
    Dev {
        #[command(flatten)]
        args: dev::Args,
    },
    /// Inspect a development revision and this caller's recent operation errors.
    #[cfg(feature = "dev-plugin-drafts")]
    Diagnostics {
        #[command(flatten)]
        args: dev::DiagnosticsArgs,
    },
    /// Validate a plugin manifest and its package documentation.
    Check {
        /// Plugin package directory.
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Type-check, build and run local plugin tests against a fixture host.
    Test {
        /// Plugin package directory with dependencies installed.
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Validate the fixture worker's question from standard input.
    #[command(hide = true)]
    CheckQuestion,
    /// Measure the local fixture's cold, warm and three-host-call paths.
    Bench {
        /// Plugin package directory with dependencies installed.
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

#[derive(clap::Args)]
pub(super) struct InstallArgs {
    /// Built plugin directory or .calplugin archive.
    source: PathBuf,
    /// Protected server appdata directory on this host.
    #[arg(long)]
    appdata: PathBuf,
    /// Full checksum from calternal plugin review.
    #[arg(long)]
    approve: String,
    /// Space in which the candidate worker performs its data-free readiness check.
    #[arg(long, requires = "account")]
    space: Option<calternal_types::SpaceId>,
    /// Account with plugin-management access and an available enrolled machine.
    #[arg(long, requires = "space")]
    account: Option<calternal_types::AccountId>,
}

#[derive(clap::Args)]
pub(super) struct ActivationArgs {
    /// Installed plugin ID.
    id: String,
    /// Space UUID.
    #[arg(long)]
    space: calternal_types::SpaceId,
    #[command(flatten)]
    connection: calternal_cli::agent::Connection,
}

pub(super) fn main(command: PluginCommand, out: &calternal_cli::output::Output) -> i32 {
    match command {
        PluginCommand::Enable { args } => activate(args, "plugins.enable", out),
        PluginCommand::Disable { args } => activate(args, "plugins.disable", out),
        #[cfg(feature = "dev-plugin-drafts")]
        PluginCommand::Dev { args } => dev::run(args, out),
        local_command => out.finish(run(local_command)),
    }
}

fn activate(args: ActivationArgs, operation: &str, out: &calternal_cli::output::Output) -> i32 {
    calternal_cli::agent::invoke(
        args.connection,
        Some(operation.to_owned()),
        json!({"plugin_id": args.id, "space_id": args.space}).to_string(),
        out,
    )
}

fn run(command: PluginCommand) -> Result<Reply, CliError> {
    match command {
        PluginCommand::Review { source, appdata } => review(&source, appdata.as_deref()),
        PluginCommand::Pack { source, archive } => {
            use std::io::Write;
            let package = Package::read(&source).map_err(package_error)?;
            let bytes = package.archive().map_err(package_error)?;
            let mut file = fs::File::create_new(&archive).map_err(local)?;
            file.write_all(&bytes).map_err(local)?;
            Ok(Reply::new(json!({"archive":archive}))
                .fields([("Package archive", archive.display().to_string())])
                .hint("Run `calternal plugin review` on the archive to get its approval checksum."))
        }
        PluginCommand::Package { source, archive } => submission::create(&source, &archive),
        PluginCommand::VerifyPackage { archive } => submission::verify(&archive),
        PluginCommand::Install { args } => install(args, false),
        PluginCommand::Update { args } => install(args, true),
        PluginCommand::Remove { id, appdata } => {
            live_catalog::request(&appdata, Change::Remove { id: id.clone() })
                .map_err(package_error)?;
            Ok(applied(&id, "Removed"))
        }
        PluginCommand::Enable { .. } | PluginCommand::Disable { .. } => {
            unreachable!("handled before local commands")
        }
        PluginCommand::New {
            name,
            sdk_root,
            template,
        } => scaffold::create(&name, &sdk_root, template),
        #[cfg(feature = "dev-plugin-drafts")]
        PluginCommand::Dev { .. } => unreachable!("handled before local commands"),
        #[cfg(feature = "dev-plugin-drafts")]
        PluginCommand::Diagnostics { args } => dev::diagnostics(args),
        PluginCommand::Check { dir } => check(&dir),
        PluginCommand::CheckQuestion => question::validate(std::io::stdin().lock()),
        PluginCommand::Test { dir } => {
            check(&dir)?;
            npm(&dir, &["test", "--silent"])?;
            Ok(Reply::new(json!({"directory":dir,"passed":true}))
                .fields([("Directory", dir.display().to_string())])
                .line("Plugin fixture tests passed.")
                .hint("Run `calternal plugin bench` to measure operation latency."))
        }
        PluginCommand::Bench { dir } => {
            check(&dir)?;
            let report = npm(&dir, &["run", "--silent", "bench"])?;
            let report: Value = serde_json::from_slice(&report).map_err(|_| {
                CliError::new(
                    "invalid_input",
                    "The bench script did not print its JSON report.",
                )
            })?;
            let rows = ["cold", "warm", "three_host_calls"]
                .into_iter()
                .map(|name| {
                    let row = &report[name];
                    let missing =
                        || CliError::new("invalid_input", "The benchmark report is incomplete.");
                    let n = row["samples"].as_u64().ok_or_else(missing)?;
                    let median = row["p50_ms"].as_f64().ok_or_else(missing)?;
                    let p95 = row["p95_ms"].as_f64().ok_or_else(missing)?;
                    Ok(vec![
                        name.replace('_', " "),
                        n.to_string(),
                        format!("{median:.3}"),
                        format!("{p95:.3}"),
                    ])
                })
                .collect::<Result<_, CliError>>()?;
            Ok(Reply::new(report).table(&["PATH", "SAMPLES", "P50 (MS)", "P95 (MS)"], rows, "")
                .hint("These timings use the fixture host. Measure deployed latency on a performance machine."))
        }
    }
}

fn package_error(message: String) -> CliError {
    CliError::new("plugin_package", message).hint(
        "Check the source and appdata path. Run `calternal plugin review` on the exact package.",
    )
}

fn applied(id: &str, message: &str) -> Reply {
    Reply::new(json!({"id":id,"activation":"live","status":message}))
        .fields([("Plugin", id), ("Status", message)])
        .hint("Use `calternal plugin enable` to approve an installed package in a space.")
}

fn install(args: InstallArgs, update: bool) -> Result<Reply, CliError> {
    let package = Package::read(&args.source).map_err(package_error)?;
    let id = package.manifest.id.clone();
    live_catalog::request(
        &args.appdata,
        Change::Install {
            package,
            approval: args.approve,
            update,
            space: args.space,
            account: args.account,
        },
    )
    .map_err(package_error)?;
    Ok(applied(&id, if update { "Updated" } else { "Installed" }))
}

fn review(source: &Path, appdata: Option<&Path>) -> Result<Reply, CliError> {
    let package = Package::read(source).map_err(package_error)?;
    let checksum = package.checksum().map_err(package_error)?;
    let manifest = &package.manifest;
    let previous = appdata
        .map(|root| package_store::installed_manifest(root, &manifest.id))
        .transpose()
        .map_err(package_error)?
        .flatten();
    let reenable = previous
        .as_ref()
        .is_some_and(|old| package_store::authority_changed(old, manifest));
    let changed = previous.as_ref().map(|old| {
        let operations = manifest
            .operations
            .iter()
            .filter(|operation| {
                !old.operations.iter().any(|before| {
                    before.id == operation.id
                        && before.required_capability == operation.required_capability
                        && before.risk == operation.risk
                        && before.mutates == operation.mutates
                })
            })
            .map(|op| op.id.as_str())
            .collect::<Vec<_>>();
        let calls = manifest
            .host_calls
            .iter()
            .filter(|call| !old.host_calls.contains(call))
            .map(|call| json!(call).as_str().unwrap_or("").to_owned())
            .collect::<Vec<_>>();
        let secrets = manifest
            .secrets
            .iter()
            .filter(|secret| !old.secrets.contains(secret))
            .map(String::as_str)
            .collect::<Vec<_>>();
        (operations, calls, secrets)
    });
    let operations = manifest.operations.iter().map(|operation| {
        json!({"id":operation.id,"risk":operation.risk,"capability":operation.required_capability})
    }).collect::<Vec<_>>();
    let calls = manifest
        .host_calls
        .iter()
        .map(|call| json!(call).as_str().unwrap_or("").to_owned())
        .collect::<Vec<_>>();
    let rows = operations
        .iter()
        .map(|operation| {
            ["id", "risk", "capability"]
                .into_iter()
                .map(|field| operation[field].as_str().unwrap_or("").to_owned())
                .collect()
        })
        .collect();
    let mut reply = Reply::new(
        json!({"id":manifest.id,"version":manifest.version,"checksum":checksum,
        "operations":operations,"host_calls":calls,"secrets":manifest.secrets,
        "complete_manifest":manifest,
        "changes":changed.as_ref().map(|(operations,calls,secrets)| json!({
            "operations":operations, "added_host_calls":calls, "added_secrets":secrets,
            "requires_space_enable": reenable
        }))}),
    )
    .fields([
        ("Plugin", manifest.id.as_str()),
        ("Version", manifest.version.as_str()),
        ("SHA-256", checksum.as_str()),
    ])
    .table(
        &["OPERATION", "RISK", "CAPABILITY"],
        rows,
        "No operation permissions requested.",
    )
    .fields([
        ("Host calls", calls.join(", ")),
        ("Secret names", manifest.secrets.join(", ")),
    ]);
    if let Some(fetch) = manifest
        .network
        .as_ref()
        .and_then(|network| network.content_fetch.as_ref())
    {
        let sources = fetch
            .sources
            .iter()
            .map(|source| source.kind.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        reply = reply.fields([(
            "Network content fetch",
            format!("Load images and fonts referenced by {sources} via anonymous HTTPS. Servers see asset URLs and fetch times."),
        )]);
    }
    if let Some((operations, calls, secrets)) = changed {
        reply = reply.fields([
            ("New or changed operations", operations.join(", ")),
            ("Added host calls", calls.join(", ")),
            ("Added secret names", secrets.join(", ")),
        ]);
    }
    if reenable {
        reply = reply.line("Spaces must enable the package again after this update.");
    }
    Ok(reply.hint("For all declarations rerun with `--json`. Pass the reviewed SHA-256 to `install` or `update --approve`."))
}

fn local(error: std::io::Error) -> CliError {
    CliError::new("plugin_io", error.to_string())
        .hint("Check the plugin path and your filesystem permissions.")
}

fn check(dir: &Path) -> Result<Reply, CliError> {
    let file = dir.join("manifest.json");
    let mut bytes = Vec::new();
    fs::File::open(&file)
        .map_err(local)?
        .take((MAX_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(local)?;
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(CliError::new(
            "invalid_input",
            "manifest.json exceeds 256 KiB.",
        ));
    }
    let manifest: calternal_core::ManifestV1 = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::new("invalid_input", format!("manifest.json: {error}"))
            .hint("Use plugins.use to start a job, space.read for a read, and content.write for content edits. See the plugin API reference for all capabilities.")
    })?;
    calternal_plugin_host::validate_manifest_detailed(&manifest).map_err(|problem| {
        CliError::new("invalid_input", format!("manifest.json: {problem}"))
            .hint("Check IDs, schemas, operation capabilities and risks against the plugin API reference.")
    })?;
    question::check_file(dir)?;
    let mut docs = String::new();
    fs::File::open(dir.join("README.md"))
        .map_err(|_| CliError::new("invalid_input", "Add a README.md documenting the plugin."))?
        .take(256 * 1024 + 1)
        .read_to_string(&mut docs)
        .map_err(local)?;
    if docs.len() > 256 * 1024 || docs.lines().all(|line| !line.starts_with("# ")) {
        return Err(CliError::new(
            "invalid_input",
            "README.md needs a Markdown title and must fit within 256 KiB.",
        )
        .hint("Start with a line such as # My plugin."));
    }
    let undocumented: Vec<_> = manifest
        .operations
        .iter()
        .filter(|op| !docs.contains(&op.id))
        .map(|op| op.id.as_str())
        .collect();
    if !undocumented.is_empty() {
        return Err(CliError::new(
            "invalid_input",
            format!(
                "README.md is missing these operation IDs: {}.",
                undocumented.join(", ")
            ),
        )
        .hint("Document what each listed operation does and include its full ID."));
    }
    Ok(
        Reply::new(json!({"id":manifest.id,"version":manifest.version,
        "host_calls":manifest.host_calls,"operations":manifest.operations.iter().map(|op| json!({
            "id":op.id, "risk":op.risk, "capability": op.required_capability
        })).collect::<Vec<_>>()}))
        .fields([("Plugin", manifest.id), ("Version", manifest.version)])
        .table(
            &["OPERATION", "RISK", "CAPABILITY"],
            manifest
                .operations
                .into_iter()
                .map(|op| {
                    vec![
                        op.id,
                        json!(op.risk).as_str().unwrap_or("").to_owned(),
                        calternal_server::plugins::capability_name(op.required_capability),
                    ]
                })
                .collect(),
            "No operations declared.",
        )
        .fields([(
            "Host calls",
            manifest
                .host_calls
                .iter()
                .map(|call| json!(call).as_str().unwrap_or("").to_owned())
                .collect::<Vec<_>>()
                .join(", "),
        )])
        .hint("Run `calternal plugin test` to exercise the worker with fixture data."),
    )
}

fn npm(dir: &Path, args: &[&str]) -> Result<Vec<u8>, CliError> {
    let tool_hint = "Source the development env in this shell, or put Node.js and npm on PATH.";
    let mut command = Command::new("npm");
    if args.first() == Some(&"test") {
        command.env(
            "CALTERNAL_PLUGIN_TEST_BIN",
            std::env::current_exe().map_err(local)?,
        );
    }
    let output = command
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                CliError::new("plugin_tool_missing", "npm was not found on PATH.").hint(tool_hint)
            } else {
                local(error)
            }
        })?;
    if !output.status.success() {
        // Node's test reporter places fixture failures on stdout; compilers and
        // subprocesses can also use stderr. Keep both within the local CLI budget.
        let stdout = String::from_utf8_lossy(&output.stdout[..output.stdout.len().min(4096)]);
        let stderr = String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(4096)]);
        let diagnostic = format!("{stdout}{stderr}");
        let hint = if diagnostic.contains("node: not found")
            || diagnostic.contains("env: ‘node’: No such file")
        {
            tool_hint
        } else if diagnostic.contains("ERR_MODULE_NOT_FOUND")
            && diagnostic.contains("Cannot find package")
        {
            "Run `npm install` in the plugin directory. Check that the missing package is declared in package.json."
        } else if diagnostic.contains("fixture worker") || args.first() == Some(&"test") {
            "Check the failing test and the worker diagnostic above. Rebuild with `npm run build` if the worker bundle is missing."
        } else {
            "Fix the first diagnostic above and rerun the plugin command."
        };
        return Err(CliError::new(
            "plugin_script_failed",
            format!(
                "npm {} ({})\n{}",
                args.join(" "),
                output.status,
                diagnostic.trim_end()
            ),
        )
        .hint(hint));
    }
    Ok(output.stdout)
}
