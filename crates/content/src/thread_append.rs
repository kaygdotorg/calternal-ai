//! Thread revision patches and recoverable Markdown placement.
mod io;
#[cfg(target_os = "linux")]
pub(crate) mod proof;
pub(crate) mod revision;
#[cfg(target_os = "linux")]
mod writer;
