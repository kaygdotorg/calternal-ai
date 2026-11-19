//! Shared plumbing of the Calternal command-line programs.
//!
//! [`output`] is the one place that decides how any `cl` or `calternal` command
//! prints: aligned tables and labelled blocks for people, one JSON document with
//! `--json`, plain-language errors, and exit codes. Command layers build a
//! [`output::Reply`] or a [`output::CliError`] and hand it to an [`output::Output`];
//! they never print results themselves. The reference for users is `docs/cli.md`.
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

pub mod output;
#[cfg(feature = "testing")]
pub mod testing;

pub mod agent;

pub mod watch;

pub mod account_input;
pub mod onboarding;

/// Non-interactive thread operations using manifest admission and common output.
pub mod threads;

/// HTTP people, group and invitation commands.
pub mod people;

/// Structured Notes, tags and Trash command adapters.
pub mod notes;

/// Account requests and trusted person decisions.
pub mod requests;

pub mod search;

/// Decode up to 2 MiB of JSON from a local file, or from stdin for `-`.
/// Errors identify the input problem without displaying its contents.
pub fn read_json_file(path: &std::path::Path) -> Result<serde_json::Value, output::CliError> {
    use std::io::Read;

    let reader: Box<dyn Read> = if path.as_os_str() == "-" {
        Box::new(std::io::stdin().lock())
    } else {
        Box::new(
            std::fs::File::open(path)
                .map_err(|_| output::CliError::usage("Cannot open the JSON input file."))?,
        )
    };
    let mut bytes = Vec::new();
    reader
        .take(calternal_core::MAX_BODY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| output::CliError::usage("Cannot read the JSON input."))?;
    if bytes.len() > calternal_core::MAX_BODY_BYTES {
        return Err(output::CliError::usage("JSON input exceeds 2 MiB."));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        output::CliError::usage(format!(
            "Invalid JSON at line {}, column {}.",
            error.line(),
            error.column()
        ))
        .hint("Pass one JSON object.")
    })
}

/// Validate an opaque page cursor, reading one bounded value from stdin for `-`.
pub(crate) fn parse_cursor(argument: &str) -> Result<String, String> {
    use std::io::Read;

    if argument != "-" {
        calternal_core::validate_page_cursor(argument).map_err(|error| error.message)?;
        return Ok(argument.to_owned());
    }
    let mut token = String::new();
    std::io::stdin()
        .take(calternal_core::MAX_PAGE_CURSOR_BYTES as u64 + 2)
        .read_to_string(&mut token)
        .map_err(|_| "cannot read a cursor from stdin")?;
    let token = token.trim_end_matches(['\r', '\n']);
    calternal_core::validate_page_cursor(token).map_err(|error| error.message)?;
    Ok(token.to_owned())
}
