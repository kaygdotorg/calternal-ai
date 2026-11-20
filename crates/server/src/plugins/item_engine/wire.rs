//! Typed item operands and scoped identity formatting.
use super::*;

pub(super) fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, Error> {
    T::deserialize(value).map_err(|_| Error::invalid_input("invalid item operand"))
}

pub(super) fn body(encoded: &str, limit: usize) -> Result<Vec<u8>, Error> {
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| Error::invalid_input("invalid item bytes"))?;
    if bytes.len() > limit {
        return Err(Error::invalid_input("item bytes exceed the limit"));
    }
    Ok(bytes)
}

pub(super) fn locator(space: SpaceId, item: Uuid) -> Value {
    json!({"space":space,"id":item})
}

pub(super) struct Write<'a> {
    pub(super) method: &'a str,
    pub(super) operand: &'a Value,
    pub(super) id: RequestId,
    pub(super) target: Option<Uuid>,
    pub(super) reference: &'a ItemRef,
    pub(super) expected: Option<&'a RevisionRef>,
    pub(super) placement: CheckedItemPlacement<'a>,
    pub(super) metadata: Value,
    pub(super) body: CheckedItemBody<'a>,
    pub(super) marker: Option<&'a str>,
    pub(super) stage_begin: Option<&'a str>,
    /// The host-composed record, when the body change is attested.
    pub(super) record: Option<&'a Composed>,
    /// The body change is raw plugin bytes (14.36: never preserves
    /// creator-only discovery and leaves the body unproven).
    pub(super) opaque: bool,
}

#[derive(Deserialize)]
pub(super) struct Create {
    pub(super) request_id: RequestId,
    pub(super) kind: String,
    #[serde(default)]
    pub(super) folders: Vec<String>,
    pub(super) name: String,
    pub(super) metadata: Value,
    pub(super) body: Option<String>,
    pub(super) attempt_discovery: Option<String>,
    pub(super) attested: Option<Attested>,
}

#[derive(Deserialize)]
pub(super) struct Head {
    pub(super) item: ItemRef,
}

#[derive(Deserialize)]
pub(super) struct Range {
    pub(super) item: ItemRef,
    pub(super) revision: RevisionRef,
    pub(super) offset: u64,
    pub(super) length: usize,
}

#[derive(Deserialize)]
pub(super) struct Save {
    pub(super) request_id: RequestId,
    pub(super) item: ItemRef,
    pub(super) expected: RevisionRef,
    pub(super) patch: Option<Value>,
    pub(super) name: Option<String>,
    pub(super) body: Option<String>,
    pub(super) staged: Option<String>,
    pub(super) attempt_discovery: Option<String>,
    pub(super) attested: Option<Attested>,
    pub(super) replace_span: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct Append {
    pub(super) request_id: RequestId,
    pub(super) item: ItemRef,
    pub(super) expected_revision: RevisionRef,
    pub(super) bytes: Option<String>,
    pub(super) attempt_discovery: Option<String>,
    pub(super) attested: Option<Attested>,
}
