//! Thread revision patches and recoverable Markdown placement.
#[cfg(target_os = "linux")]
mod group;
mod io;
#[cfg(target_os = "linux")]
pub(crate) mod proof;
pub(crate) mod revision;
#[cfg(target_os = "linux")]
mod writer;
#[cfg(target_os = "linux")]
pub(crate) use group::Group;
