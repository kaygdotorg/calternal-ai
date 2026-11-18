//! Account directory, optional development credentials, and stored space grants.
//!
//! Two development tokens authenticate seeded accounts with independent UUIDv7
//! identities. Their spaces and current memberships live in system/auth.db.
//!
//! Each gets a separate random 256-bit token stored raw (no newline) in
//! `<data>/dev-tokens/<principal>.token`, mode 0600, directory 0700. Token
//! values are never printed or logged.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use calternal_core::Principal;
use subtle::ConstantTimeEq;

use calternal_auth::spaces::{SpaceChange, Spaces};
use calternal_core::JsonObject;
use calternal_types::{AccountId, CapabilitySet, Error, SpaceId};
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
/// awaits native authority workers or adapts synchronous providers on blocking tasks. Authentication
/// must validate current sessions, never trust a caller-supplied account ID.
pub trait Directory: Send + Sync + 'static {
    /// Mutation hints for idle live connections. Implementations must include
    /// membership and session sources; delivery still checks current authority.
    /// Cost: bounded watch receiver clones, no I/O.
    fn changes(&self) -> Vec<tokio::sync::watch::Receiver<u64>> {
        Vec::new()
    }
    /// Optional protected account HTTP service; absent for synthetic test directories.
    fn http_auth(&self) -> Option<&crate::account_http::HttpAuth> {
        None
    }
    /// Resolve a current bearer token to identity only, without caching grants.
    fn authenticate(&self, presented: &[u8]) -> Option<Identity>;
    /// Resolve current capability grants for one account and space; no membership
    /// or failed lookup returns `None`, while an empty set grants no operations.
    fn capabilities(&self, account: AccountId, space: SpaceId) -> Option<CapabilitySet>;
    /// Optional direct asynchronous current-grant query. Some returns a future
    /// owning only the selected account/space and backend handle; it must fail
    /// closed and retain the synchronous API's publication ordering. None selects
    /// the synchronous adapter on a blocking thread. Constructing it performs no
    /// I/O; no result may be cached as authority for a later request.
    fn capabilities_async(
        &self,
        _account: AccountId,
        _space: SpaceId,
    ) -> Option<futures_util::future::BoxFuture<'static, Option<CapabilitySet>>> {
        None
    }

    /// Build the discovery snapshot; these grants are informational, not authority.
    /// Allocation is confined to discovery, outside the operation check path.
    fn describe(&self, principal: &Identity) -> Principal;
    /// Protected stored authority, when this directory owns a SQLite directory.
    /// An in-memory fixture may omit it. Borrow only, no I/O.
    fn protected_spaces(&self) -> Option<&Spaces> {
        None
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

/// The protected security log attached to `auth.db` (see `calternal-recovery`).
pub type Recovery = std::sync::Arc<dyn calternal_auth::spaces::RecoverySink>;

/// Stored account/space directory with optional development tokens and HTTP sessions.
/// Memberships are read on each request and never cached in identities.
pub struct StoredDirectory {
    entries: Vec<(Vec<u8>, Identity)>,
    /// Private token file paths, never token values.
    pub files: Vec<(&'static str, PathBuf)>,
    /// Protected persisted space service for development host integration.
    pub spaces: Spaces,
    http: Option<crate::account_http::HttpAuth>,
}
impl StoredDirectory {
    /// Open system/auth.db and seed disposable fixtures once, then open tokens.
    /// In-process fixtures only: no account service and no recovery log.
    pub fn load_or_create(data_dir: &Path) -> std::io::Result<Self> {
        let path = data_dir.join("system/auth.db");
        let spaces = on_startup_thread(move || Spaces::open(&path))?;
        Self::with_spaces(data_dir, spaces, true, None)
    }
    /// Opens the protected directory and account service over one `auth.db` handle.
    /// Only explicit development mode creates fixture accounts or accepts
    /// development bearer tokens.
    ///
    /// `recovery` is the protected security log: it is attached before any fixture
    /// is seeded or request served, so every security change is published to it
    /// before acknowledgement. Without it nothing is logged.
    pub fn open(
        data: &Path,
        config: calternal_auth::AuthConfig,
        development: bool,
        recovery: Option<Recovery>,
    ) -> std::io::Result<Self> {
        let auth =
            calternal_auth::Auth::open(data, config.clone()).map_err(std::io::Error::other)?;
        let mut table = Self::with_spaces(data, auth.spaces(), development, recovery)?;
        table.http = Some(crate::account_http::HttpAuth { auth, config });
        Ok(table)
    }
    /// Attaches `recovery`, then (when `development`) seeds the fixtures and opens
    /// their bearer token files.
    fn with_spaces(
        data_dir: &Path,
        spaces: Spaces,
        development: bool,
        recovery: Option<Recovery>,
    ) -> std::io::Result<Self> {
        let (spaces, ids) = on_startup_thread(move || {
            if let Some(sink) = recovery {
                spaces.configure_recovery(sink)?;
            }
            let ids = development.then(|| spaces.seed_development()).transpose()?;
            Ok((spaces, ids))
        })?;
        let mut entries = Vec::new();
        let mut files = Vec::new();
        if let Some(ids) = ids {
            let dir = data_dir.join("dev-tokens");
            calternal_fsio::private::directory(&dir)?;
            for (name, id) in [("owner", ids.owner), ("guest", ids.guest)] {
                let path = dir.join(format!("{name}.token"));
                entries.push((load_or_create_token(&path)?, Identity { id }));
                files.push((name, path));
            }
        }
        Ok(Self {
            entries,
            files,
            spaces,
            http: None,
        })
    }
}
/// Runs blocking directory startup on its own thread: these constructors are also
/// called from async fixtures, and the auth worker's blocking admission must never
/// run inside a Tokio runtime context.
fn on_startup_thread<T: Send + 'static>(
    f: impl FnOnce() -> calternal_auth::Result<T> + Send + 'static,
) -> std::io::Result<T> {
    std::thread::spawn(f)
        .join()
        .map_err(|_| std::io::Error::other("directory startup failed"))?
        .map_err(std::io::Error::other)
}
pub(crate) fn auth_error(e: calternal_auth::Error) -> Error {
    match e.code() {
        calternal_auth::ErrorCode::InvalidInput => Error::invalid_input("invalid space operation"),
        calternal_auth::ErrorCode::NotFound => Error::not_found("directory entry not found"),
        calternal_auth::ErrorCode::Forbidden => Error::forbidden("no access to this space"),
        calternal_auth::ErrorCode::Conflict => Error::request_reused(),
        _ => Error::internal("space database operation failed"),
    }
}
impl Directory for StoredDirectory {
    fn changes(&self) -> Vec<tokio::sync::watch::Receiver<u64>> {
        // Accounts and spaces share one auth.db handle, hence one change watch.
        vec![self.spaces.changes()]
    }

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
        self.spaces.capabilities(account, space).ok()?
    }
    fn capabilities_async(
        &self,
        account: AccountId,
        space: SpaceId,
    ) -> Option<futures_util::future::BoxFuture<'static, Option<CapabilitySet>>> {
        let spaces = self.spaces.clone();
        Some(Box::pin(async move {
            spaces
                .capabilities_async(account, space)
                .await
                .ok()
                .flatten()
        }))
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
    fn protected_spaces(&self) -> Option<&Spaces> {
        Some(&self.spaces)
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
            let page = self
                .spaces
                .members(
                    actor,
                    space.ok_or_else(|| Error::forbidden("no access to this space"))?,
                    input
                        .get("cursor")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
                .map_err(auth_error)?;
            return serde_json::to_value(page).map_err(|_| Error::internal("encode members"));
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
            "spaces.leave" => SpaceChange::Leave,
            "spaces.transfer" => SpaceChange::Transfer { target: target()? },
            "spaces.delete" => SpaceChange::Delete,
            _ => {
                let operation = calternal_core::PeopleOperation::from_id(op)
                    .ok_or_else(|| Error::not_found("unknown operation"))?;
                crate::api::people::grant_change(operation, input)?
            }
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
