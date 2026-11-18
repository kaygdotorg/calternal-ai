//! Local TypeScript author commands. Author scripts run as the invoking user.
use calternal_cli::output::{CliError, Reply};
use calternal_plugin_host::{CompiledSchemas, MAX_MANIFEST_BYTES};
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
    /// Create a TypeScript plugin with a kind, operation, portable view and fixture test.
    New {
        /// Lowercase package ID and new directory name.
        name: String,
        /// Calternal source checkout with a built TypeScript SDK.
        #[arg(long, env = "CALTERNAL_SDK_ROOT", default_value = ".")]
        sdk_root: PathBuf,
    },
    /// Build and reload a plugin while its development server stays running.
    #[cfg(feature = "dev-plugin-drafts")]
    Dev {
        #[command(flatten)]
        args: dev::Args,
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
        PluginCommand::New { name, sdk_root } => scaffold(&name, &sdk_root),
        #[cfg(feature = "dev-plugin-drafts")]
        PluginCommand::Dev { .. } => unreachable!("handled before local commands"),
        PluginCommand::Check { dir } => check(&dir),
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
            .hint("Compare the field with the plugin API reference.")
    })?;
    CompiledSchemas::compile(&manifest).map_err(|problem| {
        CliError::new("invalid_input", format!("manifest.json: {problem}"))
            .hint("Check IDs, schemas, operation capabilities and risks against the plugin API reference.")
    })?;
    let mut docs = String::new();
    fs::File::open(dir.join("README.md"))
        .map_err(|_| CliError::new("invalid_input", "Add a README.md documenting the plugin."))?
        .take(256 * 1024 + 1)
        .read_to_string(&mut docs)
        .map_err(local)?;
    if docs.len() > 256 * 1024
        || docs.lines().all(|line| !line.starts_with("# "))
        || manifest.operations.iter().any(|op| !docs.contains(&op.id))
    {
        return Err(CliError::new(
            "invalid_input",
            "README.md must have a title and document each operation ID within 256 KiB.",
        ));
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
    let output = Command::new("npm")
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

fn scaffold(name: &str, root: &Path) -> Result<Reply, CliError> {
    if name.len() > 64
        || !name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(CliError::new(
            "invalid_input",
            "Use 1–64 lowercase ASCII letters, digits or hyphens and start with a letter.",
        ));
    }
    let root = root.canonicalize().map_err(local)?;
    if !root.join("packages/plugin-sdk/dist/index.js").is_file()
        || !root.join("packages/contracts/package.json").is_file()
    {
        return Err(CliError::new("invalid_input", "The Calternal SDK build was not found.")
            .hint("Build the SDK in the Calternal checkout with `npm run build:plugins` and pass the checkout with --sdk-root."));
    }
    let dir = Path::new(name);
    fs::create_dir(dir).map_err(local)?;
    let operation = format!("{name}.hello");
    let manifest: Value = serde_json::from_str(
        &include_str!("../../../packages/plugin-sdk/templates/manifest.json")
            .replace("__PLUGIN__", name),
    )
    .expect("checked starter manifest template");
    let dependencies = json!({
        "name":name,"version":"0.1.0","private":true,
        "scripts":{"build":"node build.mjs","calternal:dev":"node build.mjs --draft","test":"npm run --silent build && node --test worker.test.mjs",
            "bench":"npm run --silent build && node bench.mjs"},
        "dependencies":{
            "@calternal/contracts":format!("file:{}",root.join("packages/contracts").display()),
            "@calternal/plugin-sdk":format!("file:{}",root.join("packages/plugin-sdk").display())},
        "devDependencies":{"typescript":"5.9.3","esbuild":"0.25.12","@types/node":"22.15.30"}
    });
    let tsconfig = json!({"compilerOptions":{"target":"ES2022","module":"ESNext",
        "moduleResolution":"Bundler","strict":true,"skipLibCheck":true},"include":["worker.ts"]});
    for (file, content) in [
        ("manifest.json", manifest),
        ("package.json", dependencies),
        ("tsconfig.json", tsconfig),
    ] {
        fs::write(dir.join(file), format!("{content:#}\n")).map_err(local)?;
    }
    for (file, body) in [
        ("worker.ts", include_str!("../plugin-template/worker.ts")),
        (
            "worker.test.mjs",
            include_str!("../plugin-template/worker.test.mjs"),
        ),
        ("build.mjs", include_str!("../plugin-template/build.mjs")),
        ("bench.mjs", include_str!("../plugin-template/bench.mjs")),
    ] {
        fs::write(dir.join(file), body.replace("__PLUGIN__", name)).map_err(local)?;
    }
    fs::write(
        dir.join(".gitignore"),
        "/dist/\n/node_modules/\n/manifest.generated.ts\n",
    )
    .map_err(local)?;
    fs::write(dir.join("README.md"), format!("# {name}\n\n`{operation}` returns a generated status view.\nAn optional `reads` flag exercises three read calls against the fixture.\n\nRun `npm install` and `calternal plugin test` in this directory.\n")).map_err(local)?;
    Ok(
        Reply::new(json!({"id":name,"directory":name,"operation":operation,
                "entry":"worker.ts","test":"worker.test.mjs",
                "next_commands":[format!("cd {name}"),"npm install","calternal plugin check .","calternal plugin test ."]}))
            .fields([("New plugin", name.to_owned()), ("Operation", operation)])
            .fields([("Worker", "worker.ts"), ("Test", "worker.test.mjs")])
            .line(format!("Next: cd {name} && npm install && calternal plugin check . && calternal plugin test ."))
            .hint("Source the development env in this shell before running the commands if Node.js or npm is missing."),
    )
}
