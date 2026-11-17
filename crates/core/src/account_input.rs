//! Private account input contracts; account identity always comes from the session.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Draft key: one private composition per account, space and target item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DraftKey {
    /// Owning space UUID, used only to recheck access and remove revoked input.
    pub space_id: String,
    /// Full UUID of the thread or item being composed against.
    pub item_id: String,
}
/// Last-write-wins draft replacement; no client merge or base revision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DraftSaveInput {
    /// Owning space UUID.
    pub space_id: String,
    /// Full target item UUID.
    pub item_id: String,
    /// Unsent UTF-8 text, bounded by the document limit and account ceiling.
    pub text: String,
}
/// Account-private list input. No caller-supplied account is accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountInputList {}
/// Stable pending identity: the write's space and request ID, as request IDs
/// are scoped to (account, space). Another account's identical ID selects its own state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PendingInput {
    /// Space UUID the write targets.
    pub space_id: String,
    /// Request ID of the underlying write, retained through every reconnect.
    pub id: String,
}
/// Queue or replace a pending write before its first submission attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PendingQueueInput {
    /// Owning space UUID; must agree with the write's resolved target.
    pub space_id: String,
    /// Full target item UUID, used to clear its draft after a successful send.
    pub item_id: String,
    /// Original offline creation time in Unix milliseconds; future times are refused.
    #[ts(type = "number")]
    pub created_at_ms: i64,
    /// Validated operation envelope with a stable request ID; nested input operations forbidden.
    pub request: crate::OperationRequest,
}
/// Current account-owned record; revisions come from the server, never a device clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct AccountInputRecord {
    /// Draft item UUID or pending request ID.
    pub id: String,
    /// Space checked before delivery or submission.
    pub space_id: String,
    /// Thread or item being composed against.
    pub item_id: String,
    /// Opaque server revision for this replacement.
    pub revision: String,
    /// Original creation time, preserved on edits.
    #[ts(type = "number")]
    pub created_at_ms: i64,
    /// Draft text, absent for pending submissions.
    pub text: Option<String>,
    /// Stable operation, absent for drafts.
    pub request: Option<crate::OperationRequest>,
    /// queued, confirmation_required, or submitting (immutable after attempt).
    pub state: String,
}
/// Operation result containing one private input, or none after removal/send.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AccountInputResult {
    /// Current record; confirmation_required is also a request addressed to this person.
    pub record: Option<AccountInputRecord>,
    /// Underlying write's ordinary receipt, returned after submission.
    #[ts(type = "unknown")]
    pub result: Option<serde_json::Value>,
}
/// Complete bounded account input snapshot, fetched again after a stream reset.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
pub struct AccountInputListResult {
    /// Current authorized records, ordered by identity.
    pub records: Vec<AccountInputRecord>,
}
/// Account feed uses the space feed's explicit acceptance/reset rules.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AccountFrame {
    /// Invalidate and reload account input; carries no private content after revocation.
    Changed {
        /// Account-bound unpredictable bookmark, persisted only after reload completes.
        bookmark: String,
    },
    /// Backlog exhausted at this account-bound bookmark.
    Ready {
        /// Current accepted position.
        bookmark: String,
    },
    /// Foreign, expired or rolled-back bookmark: reload before reopening.
    Reset,
}

#[derive(Clone, Copy)]
enum Shape {
    Save,
    List,
    Draft,
    Queue,
    Pending,
}
const OPERATIONS: &[(&str, bool, Shape)] = &[
    ("drafts.save", true, Shape::Save),
    ("drafts.list", false, Shape::List),
    ("drafts.get", false, Shape::Draft),
    ("drafts.delete", true, Shape::Draft),
    ("pending.queue", true, Shape::Queue),
    ("pending.edit", true, Shape::Queue),
    ("pending.list", false, Shape::List),
    ("pending.cancel", true, Shape::Pending),
    ("pending.confirm", true, Shape::Pending),
    ("pending.submit", true, Shape::Pending),
];
/// Private input operation names, reused to derive host policy. Iteration is
/// bounded by the static catalog; no schema generation, allocation or I/O.
pub fn account_input_operations() -> impl Iterator<Item = &'static str> {
    OPERATIONS.iter().map(|(id, _, _)| *id)
}
/// Account input schemas built once and cloned for discovery. The static operation
/// declaration also owns recognition and host policy; no per-event schema work.
pub fn account_input_catalog() -> Vec<crate::OperationDescription> {
    fn schema<T: JsonSchema>() -> crate::JsonObject {
        serde_json::from_value(serde_json::to_value(schemars::schema_for!(T)).unwrap()).unwrap()
    }
    static CATALOG: std::sync::OnceLock<Vec<crate::OperationDescription>> =
        std::sync::OnceLock::new();
    CATALOG
        .get_or_init(|| {
            OPERATIONS
                .iter()
                .map(|(id, mutates, shape)| crate::OperationDescription {
                    id: (*id).into(),
                    description: id.replace('.', " "),
                    mutates: *mutates,
                    input_schema: match shape {
                        Shape::Save => schema::<DraftSaveInput>(),
                        Shape::List => schema::<AccountInputList>(),
                        Shape::Draft => schema::<DraftKey>(),
                        Shape::Queue => schema::<PendingQueueInput>(),
                        Shape::Pending => schema::<PendingInput>(),
                    },
                    output_schema: if matches!(shape, Shape::List) {
                        schema::<AccountInputListResult>()
                    } else {
                        schema::<AccountInputResult>()
                    },
                })
                .collect()
        })
        .clone()
}
/// Recognize private input operations with a bounded static scan, no allocation.
pub fn is_account_input_operation(op: &str) -> bool {
    account_input_operations().any(|id| id == op)
}
/// One private-input operation's input, decoded and validated.
#[derive(Debug, Clone, PartialEq)]
pub enum AccountInput {
    /// `drafts.list` or `pending.list`.
    List,
    /// `drafts.get` or `drafts.delete`.
    Draft(DraftKey),
    /// `drafts.save`.
    Save(DraftSaveInput),
    /// `pending.queue` or `pending.edit`.
    Queue(PendingQueueInput),
    /// `pending.submit`, `pending.confirm` or `pending.cancel`.
    Pending(PendingInput),
}

/// Parses and validates private input once, consuming it: one JSON projection into
/// the typed input. Nested operations must be writes; their current authority and
/// item route are checked again at submission.
///
/// # Errors
/// Invalid input for a shape or value violation.
/// Cost: one bounded decode of `input`; no storage access.
pub fn parse_account_input(
    op: &str,
    input: crate::JsonObject,
) -> Result<AccountInput, calternal_types::Error> {
    fn decode<T: serde::de::DeserializeOwned>(
        input: crate::JsonObject,
    ) -> Result<T, calternal_types::Error> {
        serde_json::from_value(serde_json::Value::Object(input.into_iter().collect())).map_err(
            |_| calternal_types::Error::invalid_input("input does not match operation schema"),
        )
    }
    Ok(match op {
        "drafts.list" | "pending.list" => {
            let _: AccountInputList = decode(input)?;
            AccountInput::List
        }
        "drafts.get" | "drafts.delete" => {
            let i: DraftKey = decode(input)?;
            crate::validate_resource_id(&i.space_id)?;
            crate::validate_resource_id(&i.item_id)?;
            AccountInput::Draft(i)
        }
        "drafts.save" => {
            let i: DraftSaveInput = decode(input)?;
            crate::validate_resource_id(&i.space_id)?;
            crate::validate_resource_id(&i.item_id)?;
            crate::validate_markdown(&i.text)?;
            AccountInput::Save(i)
        }
        "pending.queue" | "pending.edit" => {
            let i: PendingQueueInput = decode(input)?;
            crate::validate_resource_id(&i.space_id)?;
            crate::validate_resource_id(&i.item_id)?;
            if is_account_input_operation(&i.request.operation)
                || i.request.operation.starts_with("spaces.")
                || i.request.operation.starts_with("secrets.")
            {
                return Err(calternal_types::Error::invalid_input(
                    "only content writes may be queued",
                ));
            }
            crate::validate_envelope(&i.request)?;
            if crate::is_known_operation(&i.request.operation) {
                crate::validate_operation_input(&i.request.operation, &i.request.input)?;
            }
            if i.created_at_ms < 0 {
                return Err(calternal_types::Error::invalid_input(
                    "creation time must be nonnegative",
                ));
            }
            AccountInput::Queue(i)
        }
        _ => {
            let i: PendingInput = decode(input)?;
            if i.id.is_empty() || i.id.chars().count() > 128 {
                return Err(calternal_types::Error::invalid_input(
                    "request ID must have 1..128 characters",
                ));
            }
            AccountInput::Pending(i)
        }
    })
}

/// [`parse_account_input`] for callers that keep the input (client-side checks).
/// Cost: one clone of `input` plus the parse.
pub fn validate_account_input(
    op: &str,
    input: &crate::JsonObject,
) -> Result<(), calternal_types::Error> {
    parse_account_input(op, input.clone()).map(drop)
}
