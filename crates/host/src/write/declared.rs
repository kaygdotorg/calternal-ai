//! Plan an item assignment against the current version held by the write batch.
use super::Working;
use crate::declarations::{self, Fields, Kind};
use crate::space::{Row, Space};
use crate::{internal, records};
use calternal_format::{self as format, Change, EvalContext, OperationForm, OperationInput};
use calternal_types::{Error, Result};

pub(super) fn plan(kind: &Kind<'_>, row: &Row, name: &str, inputs: &Fields) -> Result<Fields> {
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
    let [Change::Update(patch)] = changes.as_slice() else {
        return Err(crate::internal("item assignment did not return one patch"));
    };
    Ok(patch
        .iter()
        .map(|(key, value)| (key.clone(), declarations::as_json(value)))
        .collect())
}

pub(super) fn update(
    space: &Space,
    kind: &Kind,
    item: &mut Working,
    fields: Fields,
    body: Option<&str>,
    base: Option<u64>,
) -> Result<()> {
    let head = &mut item.row.head;
    let grouped = kind
        .together
        .iter()
        .flatten()
        .any(|field| fields.contains_key(field));
    if grouped && base.is_none() {
        return Err(Error::conflict(
            "Reload the item and supply its version to change grouped fields.",
        ));
    }
    if base.is_some_and(|base| base != head.version) {
        return Err(Error::conflict("The item version changed."));
    }
    let before = head.prefix as usize;
    let original = std::str::from_utf8(&item.bytes[..before]).map_err(internal)?;
    declarations::apply_fields(&kind.fields, &mut item.row.fields, fields.clone(), false)?;
    let changed = body.map(records::escape_body);
    let mut prefix =
        declarations::write_item(original, &fields, changed.as_deref(), kind)?.into_bytes();
    head.hash = space.hash(&prefix);
    head.prefix = prefix.len() as u64;
    for record in &mut item.records {
        record.offset = record.offset - before as u64 + head.prefix;
    }
    prefix.extend_from_slice(&item.bytes[before..]);
    item.bytes = prefix;
    Ok(())
}
