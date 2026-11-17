//! Notes operation inputs and results, and the generic address resolver's.
use crate::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Metadata returned by `notes.list`; excludes Markdown to bound listing payloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteSummary {
    /// Stable UUID note identity; access is checked against the accompanying space.
    pub id: String,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Current snapshot returned by `notes.get`, `notes.create` and `notes.update`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct Note {
    /// Stable UUID note identity; access is checked against the accompanying space.
    pub id: String,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Immutable historical snapshot in a `notes.history` page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteVersion {
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
    /// Opaque nonempty server revision; compare for equality, never infer order or parse it.
    pub revision: String,
    /// Server-authored UTC RFC 3339 timestamp for this snapshot.
    pub updated_at: String,
}
/// Input for `notes.list`; the server independently checks access to the selected space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListInput {
    /// Exact tag name (without a leading hash), including notes with descendant tags.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_present"
    )]
    #[ts(optional)]
    #[schemars(with = "String")]
    pub tag: Option<String>,
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
}
/// Input for the common `/<kind>/<id>` address resolver.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolveInput {
    /// Open item-kind identifier, independent of its rendering plugin; future kinds need no enum change.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub kind: String,
    /// Globally unique item UUID; possession grants no authority.
    pub id: String,
}
/// Current owning space and effective capabilities for a visible address.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ResolveResult {
    /// Resolved item kind.
    pub kind: String,
    /// Stable item UUID.
    pub id: String,
    /// Owning space UUID, independent of account identity.
    pub space_id: String,
    /// Lifecycle state of a Notes address, omitted for kinds without this projection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[schemars(with = "ItemState")]
    pub state: Option<ItemState>,
    /// Current effective capability names; informational, rechecked on use.
    pub access: Vec<String>,
}
/// Input for `notes.get` and `notes-insights.summarize`; a resource ID alone grants no access.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesGetInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
}
/// Input for `notes.create`; creates a new resource in an authorized space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesCreateInput {
    /// Owning/access space identifier: 1..80 ASCII letters, digits or `_.:-`; never proof of authority.
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Display title, nonblank, at most 256 Unicode scalars and without control characters.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
}
/// Input for `notes.update`; the base revision must match before the write can commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesUpdateInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
    /// Expected opaque current revision, 1..256 UTF-8 bytes; mismatch produces a conflict.
    #[schemars(length(min = 1, max = 256))]
    pub base_revision: String,
    /// Optional replacement title: nonblank, at most 256 Unicode scalars, no controls; omission preserves the title and null is rejected.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    /// UTF-8 Markdown content, at most 1 MiB; byte size differs from character count.
    pub markdown: String,
}
/// Input for `notes.history`; bounded pagination uses opaque server-issued cursors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryInput {
    /// Optional compatibility space hint; omission derives the owning space from `id`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    #[ts(as = "Option<String>", optional)]
    #[schemars(length(min = 1, max = 80), regex(pattern = r"^[a-zA-Z0-9_.:-]+$"))]
    pub space_id: String,
    /// Stable globally unique note UUID; the server resolves its owning space.
    pub id: String,
    /// Optional page size, 1..100 snapshots; omission selects the server default, null is rejected.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "u32")]
    #[schemars(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    /// Optional opaque pagination token, 1..256 UTF-8 bytes; omit for the first page, never send null.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    #[serde(deserialize_with = "optional_present")]
    #[schemars(with = "String")]
    #[schemars(length(min = 1, max = 256))]
    pub cursor: Option<String>,
}
/// Result for `notes.list`, containing at most 100 current note summaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesListResult {
    /// At most 100 current summaries, ordered by descending update timestamp then resource ID.
    pub notes: Vec<NoteSummary>,
}
/// Result for `notes.get`, `notes.create` and `notes.update`, containing one current snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteResult {
    /// Current note snapshot visible to the authenticated caller.
    pub note: Note,
}
/// Result for `notes.history`; snapshots are paginated without exposing storage paths.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesHistoryResult {
    /// Historical snapshots in descending server sequence order, bounded by the requested page size.
    pub versions: Vec<NoteVersion>,
    /// Opaque token for the next history page; null marks the end.
    #[serde(deserialize_with = "required_nullable_string")]
    #[schemars(schema_with = "nullable_string_schema")]
    pub next_cursor: Option<String>,
}
