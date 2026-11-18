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

/// Non-interactive thread operations using manifest admission and common output.
pub mod threads;

/// HTTP people, group and invitation commands.
pub mod people;

/// Structured Notes, tags and Trash command adapters.
pub mod notes;

/// Account requests and trusted person decisions.
pub mod requests;

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
