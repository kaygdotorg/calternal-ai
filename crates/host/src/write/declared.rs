//! Plan an item assignment against the current version held by the write batch.
use super::Working;
use crate::declarations::{self, Fields, Kind};
use crate::space::{Row, Space};
use crate::{internal, records};
use calternal_format::{self as format, Change, EvalContext, OperationForm, OperationInput};
use calternal_types::{Error, Result};

pub(super) fn plan(
    kind: &Kind<'_>,
    row: &Row,
    name: &str,
    inputs: &Fields,
) -> Result<format::Fields> {
    let Some(operation) = kind.operations.get(name) else {
        return Err(Error::invalid_input(
            "The item does not declare that operation.",
        ));
    };
    let OperationForm::Set(assignments) = &operation.form else {
        return Err(Error::invalid_input(
            "This call requires an operation that sets item fields.",
        ));
    };
    if assignments.iter().any(|set| set.state) {
        return Err(Error::invalid_input(
            "Use the viewer state call for personal state.",
        ));
    }
    // A literal or computed assignment owns its value.
    if inputs.keys().any(|key| {
        !assignments
            .iter()
            .any(|set| set.value.is_none() && set.target == *key)
    }) {
        return Err(Error::invalid_input(
            "The operation does not accept those inputs.",
        ));
    }
    let fields = declarations::as_fields(&row.fields)?;
    let metadata = format::Fields::new();
    let (today, now) = declarations::clock()?;
    let context = EvalContext {
        today: Some(&today),
        now: Some(&now),
        offset_seconds: Some(0),
        ..EvalContext::default()
    };
    let submitted = declarations::as_fields(inputs)?;
    let input = OperationInput {
        item: Some(format::Row {
            fields: &fields,
            metadata: &metadata,
        }),
        fields: &submitted,
        record: None,
    };
    let changes = format::apply_operation(kind.compiled, name, &input, &context)
        .map_err(declarations::invalid)?;
    let mut changes = changes.into_iter();
    match (changes.next(), changes.next()) {
        (Some(Change::Update(patch)), None) => Ok(patch),
        _ => Err(internal("item assignment did not return one patch")),
    }
}

pub(super) fn perform(
    space: &Space,
    kind: &Kind<'_>,
    item: &mut Working,
    name: &str,
    input: &Fields,
    base: Option<u64>,
) -> Result<()> {
    let patch = plan(kind, &item.row, name, input)?;
    check_base(kind, item, base, |key| patch.contains_key(key))?;
    let original =
        std::str::from_utf8(&item.bytes[..item.row.head.prefix as usize]).map_err(internal)?;
    for (key, value) in &patch {
        let field = kind.fields[key]
            .field_type
            .as_ref()
            .expect("assignment is stored");
        if *value != format::Value::Empty && !fits_host(field, value) {
            return Err(Error::invalid_input("Field value has the wrong type."));
        }
    }
    let prefix = declarations::render_item(original, &patch, None)?;
    // The planner checked the typed overlay and its save conditions. Parse
    // the rendered prefix only to enforce the file grammar and its bounds.
    format::parse_item(&prefix).map_err(declarations::invalid)?;
    for (key, value) in patch {
        if value == format::Value::Empty {
            item.row.fields.remove(&key);
        } else {
            item.row.fields.insert(key, declarations::as_json(&value));
        }
    }
    replace_prefix(space, item, prefix.into_bytes());
    Ok(())
}

// Typed coercion already checked the schema. These checks cover the narrower
// representation that this host can round-trip through its item index.
fn fits_host(field: &format::FieldType, value: &format::Value) -> bool {
    use format::{FieldType as T, Value as V};
    match (field, value) {
        (T::Person | T::Time | T::Anchor, _) => {
            declarations::accepts(field, &declarations::as_json(value))
        }
        (T::Number(units), _) => units.is_empty(),
        (T::List(inner), V::List(values)) => values
            .iter()
            .all(|value| *value != V::Empty && fits_host(inner, value)),
        (
            T::Bool
            | T::Text
            | T::LongText
            | T::Link(_)
            | T::Date
            | T::When
            | T::Url
            | T::Repeat
            | T::Choice(_),
            _,
        ) => true,
        _ => false,
    }
}

pub(super) fn update(
    space: &Space,
    kind: &Kind,
    item: &mut Working,
    fields: Fields,
    body: Option<&str>,
    base: Option<u64>,
) -> Result<()> {
    check_base(kind, item, base, |key| fields.contains_key(key))?;
    let original =
        std::str::from_utf8(&item.bytes[..item.row.head.prefix as usize]).map_err(internal)?;
    declarations::apply_fields(&kind.fields, &mut item.row.fields, fields.clone(), false)?;
    let changed = body.map(records::escape_body);
    let prefix = declarations::write_item(original, &fields, changed.as_deref(), kind)?;
    replace_prefix(space, item, prefix.into_bytes());
    Ok(())
}

fn check_base(
    kind: &Kind<'_>,
    item: &Working,
    base: Option<u64>,
    includes: impl Fn(&str) -> bool,
) -> Result<()> {
    if base.is_none() && kind.together.iter().flatten().any(|field| includes(field)) {
        return Err(Error::conflict(
            "Reload the item and supply its version to change grouped fields.",
        ));
    }
    if base.is_some_and(|base| base != item.row.head.version) {
        return Err(Error::conflict("The item version changed."));
    }
    Ok(())
}

fn replace_prefix(space: &Space, item: &mut Working, mut prefix: Vec<u8>) {
    let head = &mut item.row.head;
    let before = head.prefix as usize;
    head.hash = space.hash(&prefix);
    head.prefix = prefix.len() as u64;
    for record in &mut item.records {
        record.offset = record.offset - before as u64 + head.prefix;
    }
    prefix.extend_from_slice(&item.bytes[before..]);
    item.bytes = prefix;
}
