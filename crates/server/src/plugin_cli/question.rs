//! Authoring entry points for the shared Inbox form validator.
use super::{CliError, Reply, local};
use std::{fs::File, io::Read, path::Path};

const MAX_FORM_JSON: u64 = 512 * 1024;

pub(super) fn check_file(dir: &Path) -> Result<(), CliError> {
    match File::open(dir.join("question.form.json")) {
        Ok(file) => {
            validate(file)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(local(error)),
    }
}

pub(super) fn validate(input: impl Read) -> Result<Reply, CliError> {
    let mut bytes = Vec::new();
    input
        .take(MAX_FORM_JSON + 1)
        .read_to_end(&mut bytes)
        .map_err(local)?;
    if bytes.len() as u64 > MAX_FORM_JSON {
        return Err(CliError::new(
            "invalid_input",
            "Question form JSON exceeds 512 KiB.",
        ));
    }
    let form: calternal_core::GenUiView = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::new(
            "invalid_input",
            format!("Invalid question form JSON: {error}"),
        )
    })?;
    let label = calternal_server::question_form::label(&form).map_err(|error| {
        let mut reply = CliError::new("invalid_input", error.message)
            .hint("Use one action-free Form. See the public plugin guide's waits topic.");
        if let Some(details) = error.details {
            if let Some(rows) = details["diagnostics"].as_array() {
                let messages = rows
                    .iter()
                    .map(|row| {
                        format!(
                            "{}:{}:{} {} (got {})",
                            row["path"].as_str().unwrap_or("/"),
                            row["line"],
                            row["column"],
                            row["expected"].as_str().unwrap_or("invalid expression"),
                            row["got"].as_str().unwrap_or("unknown")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                reply.message.push('\n');
                reply.message.push_str(&messages);
            }
            reply = reply.details(details);
        }
        reply
    })?;
    Ok(
        Reply::new(serde_json::json!({"label":label}))
            .line(format!("Valid question form: {label}")),
    )
}
