//! Native Notes operation declarations and bounded picker payloads.
use crate::*;
use calternal_types::{
    Capability,
    Capability::{ContentWrite, SpaceRead},
    Error, SpaceId,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Space-scoped lookup of notes or tags by literal prefix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotePrefixInput {
    /// UUID of the space whose index to query.
    pub space_id: String,
    /// Literal prefix, up to 256 Unicode scalars. Empty starts at the first name.
    #[schemars(length(max = 256))]
    pub prefix: String,
    /// Opaque cursor from the preceding page with the same space and filters.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "optional_present"
    )]
    #[ts(optional)]
    #[schemars(with = "String", length(min = 1, max = 524288))]
    pub cursor: Option<String>,
}

/// Metadata for one picker match, including its relative Markdown destination.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct NoteMatch {
    /// Current note identity, title and revision.
    #[serde(flatten)]
    pub note: NoteSummary,
    /// Encoded relative target for a Markdown link from another note.
    pub link_target: String,
}

/// Bounded title-matching page in folded title and UUID order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NotesLookupResult {
    /// At most 100 live matches with their exact on-disk destinations.
    pub notes: Vec<NoteMatch>,
    /// Opaque continuation for the same prefix, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}

/// A named tag and its current note count in one space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TagCount {
    /// Tag name without a leading hash.
    pub name: String,
    /// Number of live notes with this tag or one nested underneath it.
    pub count: u32,
}

/// Bounded tag lookup result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TagsResult {
    /// Up to 100 tag names and counts, in literal name order.
    pub tags: Vec<TagCount>,
    /// Opaque continuation for the same space and prefix, omitted at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}

/// Rename one note at the revision shown in the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteRenameInput {
    /// Global note UUID; its current space is resolved by the host.
    pub id: String,
    /// Opaque revision selected by the caller.
    #[schemars(length(min = 1, max = 256))]
    pub base_revision: String,
    /// New title, used for the first H1 and readable filename.
    #[schemars(length(min = 1, max = 256))]
    pub title: String,
}

/// A single note UUID targeted by a lifecycle operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteTarget {
    /// Global note UUID, resolved to its owning space by the host.
    pub id: String,
}

/// Result of a recoverable Trash move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteChange {
    /// Updated metadata including the revision and the time of the move.
    pub note: NoteSummary,
}

/// Stable state of an item address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "snake_case")]
pub enum ItemState {
    /// The item is in its ordinary collection.
    Active,
    /// The item can be restored from the space's Trash.
    InTrash,
    /// The identity is retained after permanent deletion.
    Deleted,
}

/// Permanent deletion acknowledgement. Contains no note contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PurgeResult {
    /// Stable UUID retained for address resolution.
    pub id: String,
    /// Deleted state of the address.
    pub state: ItemState,
}

/// One trashed note and its remaining whole-day retention, rounded up.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct TrashedNote {
    /// Stable note identity and deletion timestamp.
    #[serde(flatten)]
    pub note: NoteSummary,
    /// Days until eligibility for automatic purge. Zero means eligible now.
    pub days_remaining: u32,
}

/// One page of notes eligible for restore or purge, newest deletion first.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TrashListResult {
    /// At most 100 notes with their remaining retention.
    pub notes: Vec<TrashedNote>,
    /// Opaque next-page cursor, omitted when there are no more notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}

/// Selects the Trash of one space for an irreversible-action preview.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TrashScope {
    /// UUID of the owning space.
    pub space_id: String,
}

/// Confirmation for exactly the current set of trashed revisions.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct TrashEmptyPreview {
    /// Number of selected notes.
    pub count: u32,
    /// Logical UTF-8 bytes in all retained versions of the selected notes.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Opaque set fingerprint required when starting the job.
    pub confirmation: String,
}

/// Confirm the irreversible deletion of a previewed Trash selection.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TrashEmptyInput {
    /// UUID of the owning space.
    pub space_id: String,
    /// Confirmation from a fresh preview for the same space.
    pub confirmation: String,
}

/// Select one previously admitted job in its owning space.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NoteJobInput {
    /// UUID of the owning space.
    pub space_id: String,
    /// Job UUID returned on admission.
    pub job_id: String,
}

/// Durable progress of one irreversible Empty Trash job.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct TrashJob {
    /// Stable job UUID.
    pub job_id: String,
    /// Number of notes selected at confirmation.
    pub count: u32,
    /// Logical version bytes selected at confirmation.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Number of selected revisions permanently removed by this job.
    pub purged: u32,
    /// Number of selected revisions restored, replaced, blocked or already removed.
    pub skipped: u32,
    /// `pending` while work remains, `done` after all selected revisions are examined.
    pub state: String,
}

/// Progress of an authorized Notes rename or its inverse.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct NoteRenameJob {
    /// Job UUID returned by the rename.
    pub job_id: String,
    /// Stable identity of the renamed note.
    pub note_id: String,
    /// Original number of candidate linking notes.
    pub total: u32,
    /// Number of source files still to inspect in the current direction.
    pub remaining: u32,
    /// Number of conflicts skipped in the current direction.
    pub skipped: u32,
    /// `pending`, `done`, `undoing`, `undone`, or `blocked`.
    pub state: String,
}

/// Authority and wire metadata shared by Notes admission and discovery.
pub struct NoteOperation {
    /// Stable operation name.
    pub id: &'static str,
    /// Current capability required at the selected item or space.
    pub capability: Capability,
    /// Whether the caller supplies a note UUID, resolved to the owning space.
    pub item: bool,
    description: &'static str,
    /// Declared effect class used by discovery and the approval reviewer.
    pub risk: OperationRisk,
    schemas: fn(&NoteOperation) -> OperationDescription,
    validate: fn(&JsonObject) -> Result<(), Error>,
}

impl NoteOperation {
    /// Form the discovery entry. Allocates the generated schemas at catalog init only.
    pub(crate) fn describe(&self) -> OperationDescription {
        (self.schemas)(self)
    }

    /// Decode and validate typed operation input without doing permission checks.
    pub(crate) fn validate(&self, input: &JsonObject) -> Result<(), Error> {
        (self.validate)(input)
    }
}

fn schemas<I: JsonSchema, O: JsonSchema>(op: &NoteOperation) -> OperationDescription {
    let object = |value| serde_json::from_value(value).expect("schema is object");
    OperationDescription {
        required_capability: Some(op.capability.name().into()),
        target: None,
        id: op.id.into(),
        description: op.description.into(),
        risk: op.risk,
        mutates: op.risk != OperationRisk::Read,
        input_schema: object(serde_json::to_value(schemars::schema_for!(I)).unwrap()),
        output_schema: object(serde_json::to_value(schemars::schema_for!(O)).unwrap()),
    }
}

fn scope(space: &str) -> Result<(), Error> {
    SpaceId::parse(space)
        .map(|_| ())
        .map_err(|_| Error::not_found("space not found"))
}

fn list(input: &JsonObject) -> Result<(), Error> {
    let i: NotesListInput = crate::validation::decode(input)?;
    if i.tag
        .as_ref()
        .is_some_and(|s| s.is_empty() || s.chars().any(char::is_control))
    {
        return Err(Error::invalid_input("invalid tag name"));
    }
    i.cursor.as_deref().map(validate_page_cursor).transpose()?;
    validate_id(&i.space_id)
}

fn prefix(input: &JsonObject) -> Result<(), Error> {
    let i: NotePrefixInput = crate::validation::decode(input)?;
    if i.prefix.chars().count() > 256 || i.prefix.chars().any(char::is_control) {
        return Err(Error::invalid_input("invalid prefix"));
    }
    i.cursor.as_deref().map(validate_page_cursor).transpose()?;
    scope(&i.space_id)
}

fn space_input(input: &JsonObject) -> Result<(), Error> {
    let i: SpaceDirectoryPageInput = crate::validation::decode(input)?;
    i.cursor.as_deref().map(validate_page_cursor).transpose()?;
    scope(&i.space_id)
}

fn trash_scope(input: &JsonObject) -> Result<(), Error> {
    scope(&crate::validation::decode::<TrashScope>(input)?.space_id)
}

fn empty_trash(input: &JsonObject) -> Result<(), Error> {
    let i: TrashEmptyInput = crate::validation::decode(input)?;
    scope(&i.space_id)?;
    if i.confirmation.len() != 64 || !i.confirmation.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::invalid_input(
            "invalid Trash confirmation; preview it again",
        ));
    }
    Ok(())
}

fn note_job(input: &JsonObject) -> Result<(), Error> {
    let i: NoteJobInput = crate::validation::decode(input)?;
    scope(&i.space_id)?;
    validate_resource_id(&i.job_id)
}

// Dispatch injects space_id only after validating these item input schemas.
fn target(input: &JsonObject) -> Result<(), Error> {
    let i: NoteTarget = crate::validation::decode(input)?;
    validate_resource_id(&i.id)
}

fn rename(input: &JsonObject) -> Result<(), Error> {
    let i: NoteRenameInput = crate::validation::decode(input)?;
    validate_resource_id(&i.id)?;
    validate_title(&i.title)?;
    if i.base_revision.is_empty() || i.base_revision.len() > 256 {
        return Err(Error::invalid_input("invalid base_revision"));
    }
    Ok(())
}

// Permanent removal administers the space and uses the existing Manager/Owner capability.
const TRASH_ADMINISTRATION: Capability = Capability::SpaceSettings;

macro_rules! operation {
    ($id:literal, $description:literal, $capability:expr, $risk:ident, $item:literal, $in:ty, $out:ty, $validate:ident) => {
        NoteOperation {
            id: $id,
            description: $description,
            capability: $capability,
            risk: OperationRisk::$risk,
            item: $item,
            schemas: schemas::<$in, $out>,
            validate: $validate,
        }
    };
}

/// Native Notes discovery and lifecycle operations. Bounded immutable slice, with no runtime construction.
pub static NOTE_OPERATIONS: &[NoteOperation] = &[
    operation!(
        "notes.list",
        "List a page of up to 100 live notes, optionally under a tag",
        SpaceRead,
        Read,
        false,
        NotesListInput,
        NotesListResult,
        list
    ),
    operation!(
        "notes.delete",
        "Move a note to its space's Trash",
        ContentWrite,
        Reversible,
        true,
        NoteTarget,
        NoteChange,
        target
    ),
    operation!(
        "notes.restore",
        "Restore a note from its space's Trash",
        ContentWrite,
        Reversible,
        true,
        NoteTarget,
        NoteChange,
        target
    ),
    operation!(
        "trash.list",
        "List a page of up to 100 notes in the space's Trash",
        SpaceRead,
        Read,
        false,
        SpaceDirectoryPageInput,
        TrashListResult,
        space_input
    ),
    operation!(
        "trash.purge",
        "Permanently delete a note from Trash",
        TRASH_ADMINISTRATION,
        Irreversible,
        true,
        NoteTarget,
        PurgeResult,
        target
    ),
    operation!(
        "trash.empty.preview",
        "Count notes and retained version bytes before emptying Trash",
        TRASH_ADMINISTRATION,
        Read,
        false,
        TrashScope,
        TrashEmptyPreview,
        trash_scope
    ),
    operation!(
        "trash.empty",
        "Start a resumable job to permanently purge the confirmed Trash selection",
        TRASH_ADMINISTRATION,
        Irreversible,
        false,
        TrashEmptyInput,
        TrashJob,
        empty_trash
    ),
    operation!(
        "trash.empty.status",
        "Read progress of an Empty Trash job",
        TRASH_ADMINISTRATION,
        Read,
        false,
        NoteJobInput,
        TrashJob,
        note_job
    ),
    operation!(
        "notes.rename",
        "Rename a note at its current revision",
        ContentWrite,
        Reversible,
        true,
        NoteRenameInput,
        NoteResult,
        rename
    ),
    operation!(
        "notes.rename.status",
        "Read progress of a Notes rename or its inverse",
        SpaceRead,
        Read,
        false,
        NoteJobInput,
        NoteRenameJob,
        note_job
    ),
    operation!(
        "notes.rename.undo",
        "Restore the original name and linked revisions from a completed rename",
        ContentWrite,
        Reversible,
        false,
        NoteJobInput,
        NoteRenameJob,
        note_job
    ),
    operation!(
        "notes.lookup",
        "Find a page of up to 100 notes by title prefix",
        SpaceRead,
        Read,
        false,
        NotePrefixInput,
        NotesLookupResult,
        prefix
    ),
    operation!(
        "tags.list",
        "List a page of up to 100 tags and their current note counts",
        SpaceRead,
        Read,
        false,
        SpaceDirectoryPageInput,
        TagsResult,
        space_input
    ),
    operation!(
        "tags.lookup",
        "Find a page of up to 100 tags by name prefix",
        SpaceRead,
        Read,
        false,
        NotePrefixInput,
        TagsResult,
        prefix
    ),
];

/// Find a declared operation in the fixed Notes operation slice. No allocation or I/O.
pub fn note_operation(id: &str) -> Option<&'static NoteOperation> {
    NOTE_OPERATIONS.iter().find(|op| op.id == id)
}
