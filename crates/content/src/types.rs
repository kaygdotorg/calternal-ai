//! Generic item contracts, separate from plugin-specific document wire formats.
use calternal_types::{AccountId, Error, SpaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Logs a private storage failure and returns the caller-safe internal error.
/// Cost: one log line and a short message allocation.
pub(crate) fn internal(detail: impl std::fmt::Display) -> Error {
    tracing::error!(error=%detail,"content storage failure");
    Error::internal("content storage error")
}
/// The not-found answer, identical for absent and foreign items.
pub(crate) fn missing() -> Error {
    Error::not_found("item not found")
}
/// The answer when the current file no longer matches the stored head.
pub(crate) fn blocked() -> Error {
    Error::conflict("item file was changed outside Calternal; writes are blocked")
        .with_details(serde_json::json!({"reason":"external_change"}))
}
/// Result of a content operation; all error messages are safe for callers.
pub type Result<T> = calternal_types::Result<T>;

/// One globally identified item version; bytes are opaque to the host.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    /// Fresh UUIDv7, independent from its space and account identifiers.
    pub id: Uuid,
    /// Owning space selected before opening any content database.
    pub space: SpaceId,
    /// Stable plural address kind, independent from a plugin identifier.
    pub kind: String,
    /// Plugin-defined metadata retained with each immutable version.
    pub metadata: Value,
    /// Version bytes; current bytes also reside in the selected space tree.
    pub bytes: Vec<u8>,
    /// Opaque version token, unique across items and spaces.
    pub revision: String,
    /// UTC commit timestamp with millisecond precision.
    pub updated_at: String,
}
/// Bounded list row; avoids loading version bytes for list operations.
#[derive(Debug, Clone)]
pub struct Summary {
    /// Global item identifier.
    pub id: Uuid,
    /// Owning space.
    pub space: SpaceId,
    /// Stable address kind.
    pub kind: String,
    /// Current plugin metadata.
    pub metadata: Value,
    /// Current opaque version token.
    pub revision: String,
    /// Current commit timestamp.
    pub updated_at: String,
}
/// Authenticated request attribution, checked by the host before library entry.
#[derive(Clone, Copy)]
pub struct Request<'a> {
    /// Acting account, distinct from the selected space.
    pub account: AccountId,
    /// Stable account-qualified request identity.
    pub id: &'a str,
    /// Operation being applied.
    pub operation: &'a str,
    /// Canonical input digest for mismatched-retry detection.
    pub fingerprint: &'a str,
}
/// Replayable response retained by the owning space's journal store.
#[derive(Debug, Clone)]
pub struct Receipt {
    /// Operation that committed this response.
    pub operation: String,
    /// Canonical operation/input digest.
    pub fingerprint: String,
    /// Original operation payload.
    pub result: Value,
    /// Optional affected item for trusted host bookkeeping; grants no access.
    /// Replay uses the durable SSD result, independently of userdata placement.
    pub item: Option<Uuid>,
}
/// Durable boundaries exposed to trusted crash tests; no environment hooks in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultPoint {
    /// Content and publication intent committed, before any file placement.
    AfterCommit,
    /// Immediately before fsio publishes current bytes.
    BeforePlacement,
    /// Bytes and parent entry durable, before placement bookkeeping.
    AfterPlacement,
}
/// Logical collection ownership; account ownership never confers space access.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "scope", content = "id", rename_all = "snake_case")]
pub enum CollectionOwner {
    /// Collection belongs to a space and shares its authority boundary.
    Space(SpaceId),
    /// Collection belongs to a protected account profile.
    Account(AccountId),
}
/// Stored explicit-membership collection metadata, independent from its renderer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Collection {
    /// Ownership boundary; account lists never expose their members without access checks.
    pub owner: CollectionOwner,
    /// User-visible name, unrelated to identity.
    pub title: String,
    /// Address kind whose items this standard collection lists.
    pub item_kind: String,
    /// Manifest key, stable across repeated plugin enablement.
    pub standard_key: Option<String>,
    /// Explicit globally identified members; reads must recheck each item's access.
    pub members: Vec<Uuid>,
}

/// Per-kind userdata placement declared by the plugin's kind registration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "visibility", rename_all = "snake_case")]
pub enum Placement {
    /// A human-readable top-level folder in the space's userdata tree.
    Visible {
        /// Single safe NFC folder name, such as `Notes`.
        folder: String,
    },
    /// A registered subtree below a visible top-level folder. This permits
    /// inherited layouts such as `AI/Chats` without special-casing an app.
    VisibleTree {
        /// Single safe NFC top-level folder name.
        folder: String,
        /// Safe NFC path components below the top-level folder, at most eight.
        subdirectories: Vec<String>,
    },
    /// Internal app files beneath the space's hidden metadata directory.
    Hidden {
        /// Stable plugin namespace; files use `.calternal/<plugin-id>/<kind>/`,
        /// never an arbitrary plugin-supplied path.
        plugin_id: String,
    },
}
/// File encoding chosen by the kind registration, independent from item identity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum FileFormat {
    /// UTF-8 Markdown with a host-owned `calternal-id` frontmatter key.
    Markdown,
    /// Opaque bytes with a registered safe filename extension; never parsed by the host.
    Opaque {
        /// Lowercase ASCII extension without the dot, at most 16 characters.
        extension: String,
    },
}
