//! Three-role storage startup guard, ported from Calternal's storage startup.
//!
//! The caller pins an installation UUID in configuration outside appdata and
//! explicitly opts into first setup. All roles and identities are checked before
//! any lock sentinel or store is created. Existing installations require a
//! nonempty regular `system/auth.db`; this is not SQLite integrity validation or
//! FO1 reconstruction. This library never creates a database.
//!
//! On Linux, Android and Apple, every path component is opened with `NOFOLLOW`
//! and directories are retained by descriptor. Markers, locks and setup writes
//! resolve through those descriptors, so path symlink substitutions cannot redirect
//! writes. Lexically overlapping paths and equal device/inode roots are refused.
//! Bind-mount ancestry under different path spellings is not detected. These checks do not
//! prove separate devices, mount IDs, SSD/HDD hardware, mount continuity, or
//! provenance of copied markers/auth files. No device separation is enforced:
//! userdata-ssd and appdata are allowed on the same filesystem. An unavailable
//! mount is detected through missing roots/markers/identity, not a mount-table
//! check. Hostile concurrent mutation of real directories or marker bytes is
//! outside the cooperative root-lock contract; this is not a tenant sandbox.
//!
//! Incomplete first setup is preserved and refused on subsequent opens; there
//! is no automatic repair, adoption of legacy single-root storage, or automatic re-pair. Deliberate offline re-pair requires explicit confirmation.

#![deny(missing_docs, rustdoc::broken_intra_doc_links)]

use std::path::PathBuf;

/// The installation's three separate role directories.
#[derive(Clone, Debug)]
pub struct Roots {
    /// Claim-validated text space tree (userdata-ssd), normally on SSD.
    pub userdata_ssd: PathBuf,
    /// Other original files space tree (userdata-hdd), normally on HDD.
    pub userdata_hdd: PathBuf,
    /// Protected databases and state (appdata), normally on SSD; never a space directory.
    pub appdata: PathBuf,
}

#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
mod guard;
#[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
pub use guard::{RootLocks, open, prepare, prepare_with_wait, re_pair};
