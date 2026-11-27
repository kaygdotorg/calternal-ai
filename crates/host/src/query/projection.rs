//! Serialize authorized query rows, pagination and aggregate evidence.
use super::QueryInput;
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
    let totals = result
        .totals
        .iter()
        .map(declarations::as_json)
        .collect::<Vec<_>>();
    let groups = result
        .groups
        .iter()
        .map(|group| {
            json!({
                "key":declarations::as_json(&group.key),
                "matched":group.matched,
                "totals":group.totals.iter().map(declarations::as_json).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    let rows = page
        .rows
        .into_iter()
        .map(|selected| {
            let mut row = row(
                kind,
                rows[selected.index],
                &inputs[selected.index],
                &selected.computed,
                &clock.0,
                &clock.1,
            )?;
            if !selected.groups.is_empty() {
                row["groups"] = declarations::as_json(&format::Value::List(selected.groups));
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "rows":rows,
        "totals":if totals.is_empty() {json!({})} else {json!(totals)},
        "groups":groups,
        "namedTotals":declarations::as_json(&format::Value::Map(result.named_totals)),
        "cursor":page.next
    }))
}

pub(super) fn row(
    kind: &Kind<'_>,
    row: &Row,
    input: &QueryInput,
    computed: &format::Fields,
    today: &str,
    now: &str,
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
        today: Some(today),
        now: Some(now),
        offset_seconds: Some(0),
        state: input.state.as_ref(),
        records: &records,
        record_positions: input.state.as_ref().map(|_| &input.record_positions),
        ..format::EvalContext::default()
    };
    let when = format::operation_conditions(
        kind.compiled,
        format::Row {
            fields: &input.fields,
            metadata: &input.metadata,
        },
        computed,
        &context,
    )
    .map_err(declarations::invalid)?;
    let computed = declarations::as_json(&format::Value::Map(computed.clone()));
    let state = input.state.as_ref().map(|state| {
        kind.state
            .keys()
            .map(|key| {
                (
                    key.clone(),
                    state
                        .get(key)
                        .map(declarations::as_json)
                        .unwrap_or_default(),
                )
            })
            .collect::<serde_json::Map<_, _>>()
    });
    Ok(
        json!({"id":row.head.id,"kind":row.head.kind,"fields":row.fields,"computed":computed,
        "state":state.unwrap_or_default(),"when":when,"version":row.head.version,"edited":row.head.edited}),
    )
}
