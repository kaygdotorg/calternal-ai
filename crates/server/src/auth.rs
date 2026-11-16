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

use calternal_core::{Principal, SpaceGrant, SpaceRole};
use subtle::ConstantTimeEq;

pub const SPACES: [&str; 2] = ["personal", "shared"];

#[derive(Debug, Clone)]
pub struct DevPrincipal {
    pub id: &'static str,
    pub grants: Vec<(&'static str, SpaceRole)>,
}

impl DevPrincipal {
    pub fn role(&self, space_id: &str) -> Option<SpaceRole> {
        self.grants
            .iter()
            .find(|(s, _)| *s == space_id)
            .map(|(_, r)| r.clone())
    }
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

pub struct TokenTable {
    entries: Vec<(Vec<u8>, DevPrincipal)>,
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
            entries.push((token, p));
        }
        Ok(Self { entries, files })
    }

    /// Constant-time compare against every token (no early exit on match).
    pub fn authenticate(&self, presented: &[u8]) -> Option<DevPrincipal> {
        let mut found = None;
        for (token, p) in &self.entries {
            if bool::from(token.as_slice().ct_eq(presented)) {
                found = Some(p.clone());
            }
        }
        found
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
