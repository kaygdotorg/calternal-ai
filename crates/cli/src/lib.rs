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

/// Account requests and trusted person decisions.
pub mod requests;
