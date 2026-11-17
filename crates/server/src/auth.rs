//! Account directory, optional development credentials, and stored space grants.
//!
//! Two development tokens authenticate seeded accounts with independent UUIDv7
//! identities. Their spaces and current memberships live in system/auth.db.
//!
//! Each gets a separate random 256-bit token stored raw (no newline) in
//! `<data>/dev-tokens/<principal>.token`, mode 0600, directory 0700. Token
//! values are never printed or logged.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use calternal_core::Principal;
use subtle::ConstantTimeEq;

use calternal_auth::spaces::{SpaceChange, Spaces};
use calternal_types::{AccountId, Capability, CapabilitySet, Error, SpaceId};
use calternal_core::JsonObject;
use serde_json::{Value, json};

/// Verified account identity without cached authority. Copying it is free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    /// The signed-in account, used for grants and mutation receipts.
    pub id: AccountId,
}

/// Protected identity and membership source, replaceable by a stored directory.
///
/// Implementations must read current grants on each `capabilities` call and distinguish
/// neither unknown spaces nor absent membership: both return `None`. Fail closed
/// on lookup failure. Storage implementations own lookup and synchronization costs; the host
/// invokes authorization on blocking tasks to keep SQLite off async workers. Authentication
/// must validate current sessions, never trust a caller-supplied account ID.
pub trait Directory: Send + Sync + 'static {
    /// Optional protected account HTTP service; absent for synthetic test directories.
    fn http_auth(&self) -> Option<&crate::account_http::HttpAuth> {
        None
    }
    /// Resolve a current bearer token to identity only, without caching grants.
    fn authenticate(&self, presented: &[u8]) -> Option<Identity>;
    /// Resolve current capability grants for one account and space; no membership
    /// or failed lookup returns `None`, while an empty set grants no operations.
    fn capabilities(&self, account: AccountId, space: SpaceId) -> Option<CapabilitySet>;
    /// Build the discovery snapshot; these grants are informational, not authority.
    /// Allocation is confined to discovery, outside the operation check path.
    fn describe(&self, principal: &Identity) -> Principal;
    /// Check the directory receipt namespace before a content mutation. The host
    /// holds its content store lock while checking and applying either domain.
    fn has_space_receipt(&self, _principal: &Identity, _request_id: &str) -> Result<bool, Error> {
        Ok(false)
    }
    /// Execute a stored-directory operation on a blocking host thread.
    /// Implementations without a stored directory fail closed.
    fn space_operation(
        &self,
        _principal: &Identity,
        _rid: &str,
        _op: &str,
        _input: &JsonObject,
    ) -> Result<Value, Error> {
        Err(Error::unavailable("stored space directory unavailable"))
    }
}

/// Stored account/space directory with optional development tokens and HTTP sessions.
/// Memberships are read on each request and never cached in identities.
pub struct TokenTable {
    entries: Vec<(Vec<u8>, Identity)>,
    /// Private token file paths, never token values.
    pub files: Vec<(&'static str, PathBuf)>,
    /// Protected persisted space service for development host integration.
    pub spaces: Spaces,
    http: Option<crate::account_http::HttpAuth>,
}
impl TokenTable {
    /// Open system/auth.db and seed disposable fixtures once, then open tokens.
    /// Only development startup calls this; it supplies no production sign-in.
    pub fn load_or_create(data_dir: &Path) -> std::io::Result<Self> {
        // This synchronous constructor is also used by async integration fixtures.
        // Join a dedicated startup thread so bounded-worker blocking admission is
        // never attempted from inside a Tokio runtime context.
        let path = data_dir.join("system/auth.db");
        let (spaces, ids) = std::thread::spawn(move || -> calternal_auth::Result<_> {
            let spaces = Spaces::open(&path)?;
            let ids = spaces.seed_development()?;
            Ok((spaces, ids))
        })
        .join()
        .map_err(|_| std::io::Error::other("directory startup failed"))?
        .map_err(std::io::Error::other)?;
        let dir = data_dir.join("dev-tokens");
        calternal_fsio::private::directory(&dir)?;
        let mut entries = Vec::new();
        let mut files = Vec::new();
        for (name, id) in [("owner", ids.owner), ("guest", ids.guest)] {
            let path = dir.join(format!("{name}.token"));
            entries.push((
                load_or_create_token(&path)?,
                Identity { id },
            ));
            files.push((name, path));
        }
        Ok(Self {
            entries,
            files,
            spaces,
            http: None,
        })
    }
    /// Opens the protected directory and account service. Only explicit development
    /// mode creates fixture accounts or accepts development bearer tokens.
    pub fn open(
        data: &Path,
        config: calternal_auth::AuthConfig,
        development: bool,
    ) -> std::io::Result<Self> {
        let mut table = if development {
            Self::load_or_create(data)?
        } else {
            let path = data.join("system/auth.db");
            let spaces = std::thread::spawn(move || Spaces::open(&path))
                .join()
                .map_err(|_| std::io::Error::other("directory startup failed"))?
                .map_err(std::io::Error::other)?;
            Self {
                entries: Vec::new(),
                files: Vec::new(),
                spaces,
                http: None,
            }
        };
        let auth =
            calternal_auth::Auth::open(data, config.clone()).map_err(std::io::Error::other)?;
        table.http = Some(crate::account_http::HttpAuth { auth, config });
        Ok(table)
    }
}
fn auth_error(e: calternal_auth::Error) -> Error {
    match e.code() {
        calternal_auth::ErrorCode::InvalidInput => {
            Error::invalid_input("invalid space operation")
        }
        calternal_auth::ErrorCode::Forbidden | calternal_auth::ErrorCode::NotFound => {
            Error::forbidden("no access to this space")
        }
        calternal_auth::ErrorCode::Conflict => {
            Error::conflict("request_id was already used with different input")
        }
        _ => Error::internal("space database operation failed"),
    }
}
impl Directory for TokenTable {
    fn http_auth(&self) -> Option<&crate::account_http::HttpAuth> {
        self.http.as_ref()
    }
    fn authenticate(&self, presented: &[u8]) -> Option<Identity> {
        let mut found = None;
        for (token, identity) in &self.entries {
            if bool::from(token.as_slice().ct_eq(presented)) {
                found = Some(identity.clone());
            }
        }
        found
    }
    fn capabilities(&self, account: AccountId, space: SpaceId) -> Option<CapabilitySet> {
        let names = self.spaces.capabilities(account, space).ok()??;
        let caps = names
            .iter()
            .filter_map(|name| Capability::from_name(name))
            .collect::<Vec<_>>();
        Some(CapabilitySet::new(&caps))
    }
    fn describe(&self, principal: &Identity) -> Principal {
        let spaces = self.spaces.list(principal.id).unwrap_or_default();
        Principal {
            id: principal.id.to_string(),
            spaces: spaces
                .into_iter()
                .filter_map(|s| serde_json::from_value(json!({"id":s.id,"role":s.role})).ok())
                .collect(),
        }
    }
    fn has_space_receipt(&self, principal: &Identity, request_id: &str) -> Result<bool, Error> {
        self.spaces
            .has_receipt(principal.id, request_id)
            .map_err(auth_error)
    }
    fn space_operation(
        &self,
        principal: &Identity,
        rid: &str,
        op: &str,
        input: &JsonObject,
    ) -> Result<Value, Error> {
        calternal_core::validate_operation_input(op, input)?;
        let actor = principal.id;
        if op == "spaces.list" {
            return Ok(json!({"spaces":self.spaces.list(actor).map_err(auth_error)?}));
        }
        let space = input
            .get("space_id")
            .and_then(Value::as_str)
            .and_then(|s| SpaceId::parse(s).ok());
        if op == "spaces.members" {
            return Ok(
                json!({"members":self.spaces.members(actor,space.ok_or_else(||Error::forbidden("no access to this space"))?).map_err(auth_error)?}),
            );
        }
        let target = || {
            input
                .get("account_id")
                .and_then(Value::as_str)
                .and_then(|a| AccountId::parse(a).ok())
                .ok_or_else(|| Error::invalid_input("account UUID required"))
        };
        let change = match op {
            "spaces.create" => SpaceChange::Create {
                name: input
                    .get("name")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::invalid_input("name required"))?
                    .into(),
            },
            "spaces.set_role" => SpaceChange::SetRole {
                target: target()?,
                role: input
                    .get("role")
                    .and_then(Value::as_str)
                    .ok_or_else(|| Error::invalid_input("role required"))?
                    .into(),
            },
            "spaces.remove_member" => SpaceChange::Remove { target: target()? },
            "spaces.leave" => SpaceChange::Leave,
            "spaces.transfer" => SpaceChange::Transfer { target: target()? },
            "spaces.delete" => SpaceChange::Delete,
            _ => return Err(Error::not_found("unknown operation")),
        };
        let fp =
            crate::store::sha256_hex(calternal_core::canonical_request_input(input).as_bytes());
        self.spaces
            .change_with_receipt(actor, space, change, Some((rid, op, &fp)))
            .map_err(auth_error)
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
