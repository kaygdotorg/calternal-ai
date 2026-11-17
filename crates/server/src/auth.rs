//! Development-preview principals and bearer tokens. Not production auth.
//!
//! Two fixed dev principals (never real accounts):
//! - `owner`: manager in `personal` and `shared`
//! - `guest`: reader in `shared` only
//!
//! Each gets a separate random 256-bit token stored raw (no newline) in
//! `<data>/dev-tokens/<principal>.token`, mode 0600, directory 0700. Token
//! values are never printed or logged.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use calternal_core::{Principal, SpaceGrant, SpaceRole};
use subtle::ConstantTimeEq;

use crate::authz::capabilities::{CapabilitySet, development_role};

/// Fixed development spaces; membership, rather than this list, grants access.
pub const SPACES: [&str; 2] = ["personal", "shared"];

/// Verified identity without cached authority. Cloning only increments an Arc.
#[derive(Debug, Clone)]
pub struct Identity {
    /// Stable account identifier used for grants and mutation receipts.
    pub id: Arc<str>,
}

/// Protected identity and membership source, replaceable by a stored directory.
///
/// Implementations must read current grants on each `capabilities` call and distinguish
/// neither unknown spaces nor absent membership: both return `None`. Fail closed
/// on lookup failure. Dispatch adds no locks or allocations to capability lookup;
/// storage implementations own their lookup/synchronization cost. Authentication
/// must validate current sessions, never trust a caller-supplied account ID.
pub trait Directory: Send + Sync + 'static {
    /// Resolve a current bearer token to identity only, without caching grants.
    fn authenticate(&self, presented: &[u8]) -> Option<Identity>;
    /// Resolve current capability grants for one account and space; no membership
    /// or failed lookup returns `None`, while an empty set grants no operations.
    fn capabilities(&self, principal: &str, space: &str) -> Option<CapabilitySet>;
    /// Build the discovery snapshot; these grants are informational, not authority.
    /// Allocation is confined to discovery, outside the operation check path.
    fn describe(&self, principal: &Identity) -> Principal;
}

/// Development fixture account; grants are fixtures, never request authority.
#[derive(Debug, Clone)]
pub struct DevPrincipal {
    /// Stable fixture account identifier.
    pub id: &'static str,
    /// Static fixture memberships used by the development directory.
    pub grants: Vec<(&'static str, SpaceRole)>,
}

impl DevPrincipal {
    /// Find a fixture membership with a bounded, allocation-free linear scan.
    pub fn role(&self, space_id: &str) -> Option<SpaceRole> {
        self.grants
            .iter()
            .find(|(s, _)| *s == space_id)
            .map(|(_, r)| r.clone())
    }
    /// Allocate a discovery snapshot of the fixture account and its grants.
    pub fn to_wire(&self) -> Principal {
        Principal {
            id: self.id.to_string(),
            spaces: self
                .grants
                .iter()
                .map(|(s, r)| SpaceGrant {
                    id: s.to_string(),
                    role: r.clone(),
                })
                .collect(),
        }
    }
}

/// Construct the two disposable development accounts and their fixture grants.
pub fn dev_principals() -> Vec<DevPrincipal> {
    vec![
        DevPrincipal {
            id: "owner",
            grants: vec![
                ("personal", SpaceRole::Manager),
                ("shared", SpaceRole::Manager),
            ],
        },
        DevPrincipal {
            id: "guest",
            grants: vec![("shared", SpaceRole::Reader)],
        },
    ]
}

impl From<DevPrincipal> for Identity {
    fn from(principal: DevPrincipal) -> Self {
        Self {
            id: Arc::from(principal.id),
        }
    }
}

/// Development directory with constant-time token comparison and fixed grants.
/// Capability reads map wire fixture roles to built-in sets and scan two accounts without allocation or synchronization.
pub struct TokenTable {
    entries: Vec<(Vec<u8>, Identity, DevPrincipal)>,
    /// Private token file locations for startup discovery; never token values.
    pub files: Vec<(&'static str, PathBuf)>,
}

impl TokenTable {
    /// Load or create token files for all dev principals.
    pub fn load_or_create(data_dir: &Path) -> std::io::Result<Self> {
        let dir = data_dir.join("dev-tokens");
        fs::create_dir_all(&dir)?;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        let mut entries = Vec::new();
        let mut files = Vec::new();
        for p in dev_principals() {
            let path = dir.join(format!("{}.token", p.id));
            let token = load_or_create_token(&path)?;
            files.push((p.id, path));
            entries.push((token, p.clone().into(), p));
        }
        Ok(Self { entries, files })
    }
}

impl Directory for TokenTable {
    fn authenticate(&self, presented: &[u8]) -> Option<Identity> {
        let mut found = None;
        for (token, identity, _) in &self.entries {
            if bool::from(token.as_slice().ct_eq(presented)) {
                found = Some(identity.clone());
            }
        }
        found
    }

    fn capabilities(&self, principal: &str, space: &str) -> Option<CapabilitySet> {
        self.entries
            .iter()
            .find(|(_, _, p)| p.id == principal)
            .and_then(|(_, _, p)| p.role(space))
            .map(|role| development_role(&role))
    }

    fn describe(&self, principal: &Identity) -> Principal {
        self.entries
            .iter()
            .find(|(_, _, p)| p.id == principal.id.as_ref())
            .map(|(_, _, p)| p.to_wire())
            .unwrap_or_else(|| Principal {
                id: principal.id.to_string(),
                spaces: vec![],
            })
    }
}

fn load_or_create_token(path: &Path) -> std::io::Result<Vec<u8>> {
    match fs::File::open(path) {
        Ok(mut f) => {
            let mode = f.metadata()?.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                return Err(std::io::Error::other(format!(
                    "token file {} must be owner-only (mode {:o})",
                    path.display(),
                    mode
                )));
            }
            let mut s = String::new();
            f.read_to_string(&mut s)?;
            let t = s.trim().as_bytes().to_vec();
            if t.len() < 32 || !t.iter().all(|c| c.is_ascii_hexdigit()) {
                return Err(std::io::Error::other(format!(
                    "token file {} is malformed",
                    path.display()
                )));
            }
            Ok(t)
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let mut raw = [0u8; 32];
            getrandom::fill(&mut raw).map_err(|e| std::io::Error::other(e.to_string()))?;
            let token = hex::encode(raw);
            let mut f = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?;
            f.write_all(token.as_bytes())?;
            f.sync_all()?;
            Ok(token.into_bytes())
        }
        Err(e) => Err(e),
    }
}
