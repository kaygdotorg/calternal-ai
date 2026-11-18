//! Private account input storage, following the secrets per-owner database pattern.
//! Mutations open one account connection on a blocking worker; live feeds retain
//! their own connection under the existing listener lease. No mutation connection,
//! mutation mutex or background thread is shared across accounts. A short map
//! lookup selects an active account's wake sender, released before sending and
//! never held over SQL, I/O or awaits. One IMMEDIATE transaction
//! owns each state change and its invalidation event; FULL commits provide durability.
mod db;
mod dispatch;
mod http;
mod reader;
mod wake;
use calternal_types::Error;
pub(crate) use dispatch::dispatch;
pub(crate) use http::events;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// Operator safety ceilings, independent of plan entitlements. Existing input is
/// preserved if ceilings are lowered; only growth is refused with a clear error.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Maximum saved drafts per account (default 256).
    pub drafts: usize,
    /// Maximum pending submissions per account (default 256).
    pub pending: usize,
    /// Total UTF-8 draft text and serialized pending-envelope bytes per account (default 16 MiB).
    pub bytes: usize,
    /// Maximum space journals on one multiplexed connection (default 16).
    pub live_spaces: usize,
    /// Live connections per account or space (default 128).
    pub listeners: usize,
    /// Retained invalidation bookmarks per account (default 1024).
    pub events: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            drafts: 256,
            pending: 256,
            bytes: 16 * 1024 * 1024,
            events: 1024,
            live_spaces: 16,
            listeners: 128,
        }
    }
}
impl Limits {
    /// Read positive operator ceilings once at startup. Fails closed on malformed
    /// values; no environment reads or parsing on the request path.
    pub fn from_env() -> Result<Self, Error> {
        fn value(name: &str, default: usize) -> Result<usize, Error> {
            match std::env::var(name) {
                Ok(v) => v
                    .parse()
                    .ok()
                    .filter(|n| *n > 0 && *n <= i32::MAX as usize)
                    .ok_or_else(|| {
                        Error::invalid_input(format!(
                            "{name} must be a positive integer <= 2147483647"
                        ))
                    }),
                Err(std::env::VarError::NotPresent) => Ok(default),
                Err(_) => Err(Error::invalid_input("invalid input ceiling")),
            }
        }
        let d = Self::default();
        Ok(Self {
            drafts: value("CALTERNAL_DRAFT_COUNT", d.drafts)?,
            pending: value("CALTERNAL_PENDING_COUNT", d.pending)?,
            bytes: value("CALTERNAL_INPUT_BYTES", d.bytes)?,
            events: value("CALTERNAL_ACCOUNT_EVENTS", d.events)?,
            live_spaces: value("CALTERNAL_LIVE_SPACES", d.live_spaces)?,
            listeners: value("CALTERNAL_LIVE_LISTENERS", d.listeners)?,
        })
    }
}
/// Cheap account storage handle; root must already be validated and locked.
/// Clock injection makes seven-day confirmation independent of wall-clock tests.
#[derive(Clone)]
pub struct Inputs {
    root: PathBuf,
    wakes: wake::Wakes,
    content: Option<Arc<calternal_content::Content>>,
    limits: Limits,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}
impl Inputs {
    /// Configure the owner-local database service without opening files. Calls
    /// later fail closed on invalid account UUIDs or SQLite/filesystem failures.
    pub fn new(root: &Path, limits: Limits) -> Self {
        Self::with_clock(
            root,
            limits,
            Arc::new(|| {
                time::OffsetDateTime::now_utc()
                    .unix_timestamp_nanos()
                    .div_euclid(1_000_000) as i64
            }),
        )
    }
    /// Attach the existing generic receipt service at startup. No I/O or new
    /// lock/connection is created. Enables replay after pending input was sent
    /// and removed; callers without this service cannot recover terminal replies.
    pub fn with_content(mut self, content: Arc<calternal_content::Content>) -> Self {
        self.content = Some(content);
        self
    }
    fn step(
        &self,
        account: calternal_types::AccountId,
    ) -> Result<Option<calternal_types::OwnerStep>, Error> {
        self.content
            .as_ref()
            .map(|content| content.lifecycle().step(account.as_uuid()))
            .transpose()
    }

    /// Immutable startup ceilings, borrowed without I/O or allocation.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }
    /// Construct with a host-owned millisecond clock; no I/O, one Arc clone per use.
    pub fn with_clock(
        root: &Path,
        limits: Limits,
        clock: Arc<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            root: root.into(),
            wakes: wake::Wakes::default(),
            content: None,
            limits,
            clock,
        }
    }
}
fn internal(_: impl std::fmt::Display) -> Error {
    Error::internal("account input storage failed")
}

/// Determine accounts whose input must be erased by a successful membership
/// removal. Runs on a blocking worker before the mutation; bounded member pages
/// for a whole-space deletion, otherwise no extra I/O. Does not grant access.
pub(crate) fn revoked_targets(
    d: &impl crate::auth::Directory,
    p: &crate::auth::Identity,
    op: &str,
    input: &calternal_core::JsonObject,
) -> Result<Vec<calternal_types::AccountId>, Error> {
    use calternal_types::AccountId;
    use serde_json::Value;
    match op {
        "spaces.remove_member" | "spaces.set_role" => Ok(input
            .get("account_id")
            .and_then(Value::as_str)
            .and_then(|a| AccountId::parse(a).ok())
            .into_iter()
            .collect()),
        "spaces.leave" => Ok(vec![p.id]),
        "spaces.delete" => {
            let mut input = input.clone();
            let mut accounts = Vec::new();
            loop {
                let page =
                    d.space_operation(p, "input-cleanup-members", "spaces.members", &input)?;
                accounts.extend(
                    page["members"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|m| {
                            m["account_id"]
                                .as_str()
                                .and_then(|id| AccountId::parse(id).ok())
                        }),
                );
                let Some(cursor) = page["next_cursor"]
                    .as_str()
                    .filter(|cursor| cursor.starts_with("account:"))
                else {
                    break;
                };
                input.insert("cursor".into(), Value::String(cursor.to_owned()));
            }
            Ok(accounts)
        }
        _ => Ok(Vec::new()),
    }
}
