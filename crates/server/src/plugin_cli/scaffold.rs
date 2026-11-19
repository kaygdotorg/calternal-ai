//! Reliable local starter packages and their README files.
use super::{CliError, Reply, fs, local};
use serde_json::json;
use std::path::Path;

/// The bundled authoring examples.
#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum Template {
    /// A generated view and a fixture read.
    View,
    /// A durable job with an Inbox question and a deadline.
    Question,
}

/// Write the selected starter to a new directory using the built local SDK.
pub(super) fn create(name: &str, root: &Path, template: Template) -> Result<Reply, CliError> {
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
    let (manifest, worker, test, descriptions) = match template {
        Template::View => (
            include_str!("../../../../packages/plugin-sdk/templates/manifest.json"),
            include_str!("../../plugin-template/worker.ts"),
            include_str!("../../plugin-template/worker.test.mjs"),
            &[
                "Returns a generated status view. The optional `reads` flag exercises fixture reads.",
            ][..],
        ),
        Template::Question => (
            include_str!("../../../../packages/plugin-sdk/templates/question.manifest.json"),
            include_str!("../../../../packages/plugin-sdk/templates/question.worker.txt"),
            include_str!("../../plugin-template/question.test.mjs"),
            &[
                "Starts a durable job and returns its ID.",
                "Asks one Inbox question and finishes with its text answer or after 20 seconds.",
                "Declares the text field that the host accepts in the question's answer.",
            ][..],
        ),
    };
    let manifest_text = manifest.replace("__PLUGIN__", name);
    let manifest: calternal_core::ManifestV1 =
        serde_json::from_str(&manifest_text).expect("checked starter manifest template");
    let operation = manifest.operations[0].id.clone();
    let mut dependencies = json!({
        "name":name,"version":"0.1.0","private":true,
        "scripts":{"build":"node build.mjs","calternal:dev":"node build.mjs --draft","test":"npm run --silent build && node --test worker.test.mjs"},
        "dependencies":{
            "@calternal/contracts":format!("file:{}",root.join("packages/contracts").display()),
            "@calternal/plugin-sdk":format!("file:{}",root.join("packages/plugin-sdk").display())},
        "devDependencies":{"typescript":"5.9.3","esbuild":"0.25.12","@types/node":"22.15.30"}
    });
    match template {
        Template::View => {
            dependencies["scripts"]["bench"] = json!("node bench.mjs");
            fs::write(
                dir.join("bench.mjs"),
                include_str!("../../plugin-template/bench.mjs").replace("__PLUGIN__", name),
            )
            .map_err(local)?;
        }
        Template::Question => fs::write(
            dir.join("question.form.json"),
            include_str!("../../../../packages/plugin-sdk/templates/question.form.json"),
        )
        .map_err(local)?,
    }
    let tsconfig = json!({"compilerOptions":{"target":"ES2022","module":"ESNext",
        "moduleResolution":"Bundler","resolveJsonModule":true,"strict":true,"skipLibCheck":true},"include":["worker.ts"]});
    for (file, content) in [
        (
            "manifest.json",
            serde_json::from_str(&manifest_text).expect("manifest is JSON"),
        ),
        ("package.json", dependencies),
        ("tsconfig.json", tsconfig),
    ] {
        fs::write(dir.join(file), format!("{content:#}\n")).map_err(local)?;
    }
    for (file, body) in [
        ("worker.ts", worker),
        ("worker.test.mjs", test),
        ("build.mjs", include_str!("../../plugin-template/build.mjs")),
    ] {
        fs::write(dir.join(file), body.replace("__PLUGIN__", name)).map_err(local)?;
    }
    fs::write(
        dir.join(".gitignore"),
        "/dist/\n/node_modules/\n/manifest.generated.ts\n",
    )
    .map_err(local)?;
    let mut readme = format!("# {name}\n\n");
    for (op, description) in manifest.operations.iter().zip(descriptions) {
        readme.push_str(&format!("- `{}` — {description}\n", op.id));
    }
    readme.push_str(
        "\nRun `npm install`, `calternal plugin check .` and `calternal plugin test .`.\n",
    );
    fs::write(dir.join("README.md"), readme).map_err(local)?;
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
