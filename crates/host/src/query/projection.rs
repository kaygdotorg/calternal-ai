//! Serialize authorized query rows, pagination and aggregate evidence.
use super::{Budget, QueryInput};
use crate::declarations::{self, Kind};
use crate::space::Row;
use calternal_format as format;
use calternal_types::{Error, Result};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn page(
    kind: &Kind<'_>,
    rows: &[&Row],
    inputs: &[QueryInput],
    result: format::QueryResult,
    cursor: Option<Uuid>,
    limit: usize,
    clock: &(String, String),
    budget: &mut Budget,
) -> Result<Value> {
    let start = cursor
        .map(|cursor| {
            result
                .rows
                .iter()
                .position(|selected| rows[selected.index].head.id == cursor)
                .map(|position| position + 1)
                .ok_or_else(|| Error::conflict("The query cursor is no longer present."))
        })
        .transpose()?
        .unwrap_or(0);
    let page = calternal_types::Page::from_lookahead(
        result
            .rows
            .into_iter()
            .skip(start)
            .take(limit + 1)
            .collect(),
        limit,
        |selected| rows[selected.index].head.id,
    );
    let totals = list(result.totals, budget)?;
    let totals = if totals.as_array().is_some_and(Vec::is_empty) {
        Value::Object(Default::default())
    } else {
        totals
    };
    let groups = result
        .groups
        .into_iter()
        .map(|group| {
            budget.format("key", &group.key)?;
            let key = declarations::as_json(&group.key);
            let totals = list(group.totals, budget)?;
            Ok(Value::Object(serde_json::Map::from_iter([
                ("key".into(), key),
                ("matched".into(), json!(group.matched)),
                ("totals".into(), totals),
            ])))
        })
        .collect::<Result<Vec<_>>>()?;
    let rows = page
        .rows
        .into_iter()
        .map(|selected| {
            let mut row = row(
                kind,
                rows[selected.index],
                &inputs[selected.index],
                &selected.computed,
                clock,
                budget,
            )?;
            if !selected.groups.is_empty() {
                row["groups"] = list(selected.groups, budget)?;
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Value::Object(serde_json::Map::from_iter([
        ("rows".into(), Value::Array(rows)),
        ("totals".into(), totals),
        ("groups".into(), Value::Array(groups)),
        ("namedTotals".into(), map(&result.named_totals, budget)?),
        ("cursor".into(), json!(page.next)),
    ])))
}

fn list(values: Vec<format::Value>, budget: &mut Budget) -> Result<Value> {
    for value in &values {
        budget.format("", value)?;
    }
    Ok(Value::Array(
        values.iter().map(declarations::as_json).collect(),
    ))
}

fn map(values: &format::Fields, budget: &mut Budget) -> Result<Value> {
    for (key, value) in values {
        budget.format(key, value)?;
    }
    Ok(Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), declarations::as_json(value)))
            .collect(),
    ))
}

pub(super) fn row(
    kind: &Kind<'_>,
    row: &Row,
    input: &QueryInput,
    computed: &format::Fields,
    clock: &(String, String),
    budget: &mut Budget,
) -> Result<Value> {
    let empty = format::Fields::new();
    let records = input
        .records
        .iter()
        .map(|metadata| format::Row {
            fields: &empty,
            metadata,
        })
        .collect::<Vec<_>>();
    let context = format::EvalContext {
        today: Some(&clock.0),
        now: Some(&clock.1),
        offset_seconds: Some(0),
        state: input.state.as_ref(),
        records: &records,
        record_positions: input.state.as_ref().map(|_| &input.record_positions),
        ..format::EvalContext::default()
    };
    let when = budget.work(|steps| {
        format::operation_conditions_bounded(
            kind.compiled,
            format::Row {
                fields: &input.fields,
                metadata: &input.metadata,
            },
            computed,
            &context,
            steps,
        )
    })?;
    let computed = map(computed, budget)?;
    let mut state = serde_json::Map::new();
    if let Some(values) = &input.state {
        for key in kind.state.keys() {
            let value = values.get(key).unwrap_or(&format::Value::Empty);
            budget.format(key, value)?;
            state.insert(key.clone(), declarations::as_json(value));
        }
    }
    let mut result = base(row, budget)?;
    result["computed"] = computed;
    result["state"] = Value::Object(state);
    result["when"] = json!(when);
    Ok(result)
}

pub(super) fn base(row: &Row, budget: &mut Budget) -> Result<Value> {
    for (key, value) in &row.fields {
        budget.json(key, value)?;
    }
    Ok(
        json!({"id":row.head.id,"kind":row.head.kind,"fields":row.fields,
        "version":row.head.version,"edited":row.head.edited}),
    )
}
