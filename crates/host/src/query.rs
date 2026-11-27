//! Select authorized rows and supply viewer context to the pure format engine.
mod ad_hoc;
mod budget;
mod projection;
mod search;

#[cfg(test)]
mod tests;

use crate::declarations::{self, Collection, FieldType, Kind};
use crate::internal;
use crate::records::Record;
use crate::space::{Row, Space};
use ad_hoc::ad_hoc;
use budget::Budget;
use calternal_format as format;
use calternal_types::{Actor, Error, ErrorCode, Result, Via};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::borrow::Cow;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Query {
    from: Option<String>,
    collection: Option<String>,
    #[serde(rename = "where")]
    filter: Option<String>,
    #[serde(default)]
    sort: Vec<String>,
    search: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Read {
    item: Uuid,
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum State {
    Get {
        item: Uuid,
        key: String,
    },
    Set {
        item: Uuid,
        key: String,
        value: Value,
    },
}

pub(crate) fn query(space: &Space, actor: &Actor, input: Query) -> Result<Value> {
    let (from, collection) = match (&input.from, &input.collection) {
        (Some(from), None) => (
            from.to_owned(),
            Cow::Owned(ad_hoc(input.filter.as_deref(), &input.sort)?),
        ),
        (None, Some(id)) if input.filter.is_none() && input.sort.is_empty() => {
            let collection = space.collection(id)?;
            if collection.share_source == format::ShareSource::Shared {
                return Err(Error::forbidden(
                    "Shared collections are outside this host slice.",
                ));
            }
            let [format::Source::Kind(from)] = collection.sources.as_slice() else {
                return Err(Error::invalid_input("A collection must select one kind."));
            };
            let owner = id.split_once('.').expect("qualified collection").0;
            let from = if from.contains('.') {
                from.clone()
            } else {
                format!("{owner}.{from}")
            };
            (from, Cow::Borrowed(collection))
        }
        _ => return Err(Error::invalid_input("Select one kind or one collection.")),
    };
    space.permits(actor, &from, false)?;
    let kind = space.kind(&from)?;
    let limit = input.limit.unwrap_or(200);
    if !(1..=200).contains(&limit) {
        return Err(Error::invalid_input("A query page holds up to 200 rows."));
    }
    for sort in &collection.sort {
        if !matches!(sort.field.as_str(), "manual" | "edited" | "id" | "kind")
            && !kind.fields.contains_key(&sort.field)
            && !collection.fields.contains_key(&sort.field)
        {
            return Err(Error::invalid_input("Unknown sort field."));
        }
    }
    let cursor = input
        .cursor
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| Error::invalid_input("Invalid query cursor."))?;
    let mut budget = Budget::default();
    let clock = declarations::clock()?;
    let mut search = search::Search::new(input.search.as_deref(), &kind.search)?;
    let mut rows = Vec::new();
    let mut inputs = Vec::new();
    for row in space.rows.values().filter(|row| row.head.kind == from) {
        budget.row(row)?;
        let records = if kind.records.is_empty() {
            Vec::new()
        } else {
            budget.records(space, row.head.id)?
        };
        inputs.push(query_input(
            space,
            actor,
            &kind,
            row,
            &records,
            &clock,
            &mut budget,
        )?);
        search.index_body(space, row)?;
        rows.push(row);
    }
    let result = execute(
        &collection,
        &kind,
        &inputs,
        format::Page {
            offset: 0,
            limit: rows.len(),
        },
        &clock,
        &|index, row| search.matches(index, row),
        &mut budget,
    )?;
    projection::page(
        &kind,
        &rows,
        &inputs,
        result,
        cursor,
        limit,
        &clock,
        &mut budget,
    )
}

pub(crate) fn read(space: &Space, actor: &Actor, input: Read) -> Result<Value> {
    let row = space.row(input.item)?;
    space.permits(actor, &row.head.kind, false)?;
    let kind = space.kind(&row.head.kind)?;
    let mut budget = Budget::default();
    let records = budget.records(space, input.item)?;
    let file = space.open_regular(&row.head.path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::conflict("The item file is missing from disk.")
        } else {
            internal(error)
        }
    })?;
    let bytes = crate::space::bounded_read(file, budget.limit())?;
    // The parser and projected values can each own a copy of the source.
    budget.bytes(bytes.len().saturating_mul(3) as u64)?;
    let views = space.record_views(&row.head, &bytes, &records, &kind)?;
    let prefix = bytes
        .get(..row.head.prefix as usize)
        .ok_or_else(|| Error::conflict("The item file changed outside the host."))?;
    let prefix = std::str::from_utf8(prefix).map_err(internal)?;
    let body = declarations::item_body(prefix)?;
    let mut result = if matches!(actor.via, Some(Via::Run { .. })) {
        projection::base(row, &mut budget)?
    } else {
        let mut collection = ad_hoc(None, &[])?;
        collection.ignoring_default = true;
        let clock = declarations::clock()?;
        let input = query_input(space, actor, &kind, row, &records, &clock, &mut budget)?;
        let result = execute(
            &collection,
            &kind,
            std::slice::from_ref(&input),
            format::Page {
                offset: 0,
                limit: 1,
            },
            &clock,
            &|_, _| true,
            &mut budget,
        )?;
        projection::row(
            &kind,
            row,
            &input,
            &result.rows[0].computed,
            &clock,
            &mut budget,
        )?
    };
    result["body"] = Value::String(body.to_owned());
    result["records"] = Value::Array(views);
    Ok(result)
}

struct QueryInput {
    fields: format::Fields,
    metadata: format::Fields,
    state: Option<format::Fields>,
    records: Vec<format::Fields>,
    record_positions: BTreeMap<String, usize>,
}

fn query_input(
    space: &Space,
    actor: &Actor,
    kind: &Kind<'_>,
    row: &Row,
    records: &[Record],
    clock: &(String, String),
    budget: &mut Budget,
) -> Result<QueryInput> {
    let owner = kind.id.split_once('.').expect("qualified kind").0;
    let state_allowed = match &actor.via {
        None => true,
        Some(Via::Plugin { plugin }) => plugin == owner,
        _ => false,
    };
    let mut state = state_allowed.then(format::Fields::new);
    let mut record_positions = BTreeMap::new();
    if let Some(state) = &mut state
        && !kind.state.is_empty()
    {
        let mut stmt = space
            .db
            .prepare_cached(
                "SELECT name,value,ordinal FROM viewer_state WHERE plugin=?1 AND item=?2 AND account=?3",
            )
            .map_err(internal)?;
        let mut rows = stmt
            .query(params![
                owner,
                row.head.id.to_string(),
                actor.account.to_string()
            ])
            .map_err(internal)?;
        while let Some(entry) = rows.next().map_err(internal)? {
            let key = entry
                .get_ref(0)
                .map_err(internal)?
                .as_str()
                .map_err(internal)?;
            let raw = entry
                .get_ref(1)
                .map_err(internal)?
                .as_str()
                .map_err(internal)?;
            let position: Option<i64> = entry.get(2).map_err(internal)?;
            budget.bytes((key.len() + 64 + raw.len().saturating_mul(2)) as u64)?;
            let Some(field) = kind.state.get(key) else {
                continue;
            };
            let value: Value = serde_json::from_str(raw).map_err(internal)?;
            if let Some(position) = position {
                let anchor = value
                    .as_str()
                    .ok_or_else(|| internal("invalid stored anchor"))?;
                record_positions.insert(
                    anchor.to_owned(),
                    records.partition_point(|row| row.ordinal as i64 <= position),
                );
            }
            let value = if field.field_type == Some(FieldType::Time) {
                format::Value::Time(
                    value
                        .as_str()
                        .ok_or_else(|| internal("invalid stored time"))?
                        .to_owned(),
                )
            } else {
                declarations::as_format(&value)?
            };
            state.insert(key.to_owned(), value);
        }
        let empty = format::Fields::new();
        let row = format::Row {
            fields: &empty,
            metadata: &empty,
        };
        let context = format::EvalContext {
            today: Some(&clock.0),
            now: Some(&clock.1),
            offset_seconds: Some(0),
            ..format::EvalContext::default()
        };
        for (name, field) in &kind.state {
            if !state.contains_key(name)
                && let Some(default) = &field.default
            {
                budget.bytes((name.len() + 64) as u64)?;
                let value =
                    budget.work(|steps| format::evaluate_bounded(default, row, &context, steps))?;
                state.insert(name.clone(), value);
            }
        }
    }

    let at = |milliseconds| -> Result<format::Value> {
        let stamp =
            time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
                .map_err(internal)?
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(internal)?;
        Ok(format::Value::Time(stamp))
    };
    let stamps = records
        .iter()
        .map(|record| {
            Ok(format::Fields::from([
                ("anchor".into(), format::Value::Text(record.anchor.clone())),
                ("author".into(), format::Value::Text(record.author.clone())),
                ("at".into(), at(record.at)?),
            ]))
        })
        .collect::<Result<Vec<_>>>()?;
    for (key, value) in &row.fields {
        budget.json(key, value)?;
    }
    Ok(QueryInput {
        fields: declarations::as_fields(&row.fields)?,
        metadata: format::Fields::from([
            ("id".into(), format::Value::Text(row.head.id.to_string())),
            ("kind".into(), format::Value::Text(row.head.kind.clone())),
            ("edited".into(), at(row.head.edited)?),
        ]),
        state,
        records: stamps,
        record_positions,
    })
}

pub(crate) fn contains(
    space: &Space,
    actor: &Actor,
    collection: &Collection,
    row: &Row,
) -> Result<bool> {
    let kind = space.kind(&row.head.kind)?;
    let mut budget = Budget::default();
    budget.row(row)?;
    let records = if kind.records.is_empty() {
        Vec::new()
    } else {
        budget.records(space, row.head.id)?
    };
    let clock = declarations::clock()?;
    let input = query_input(space, actor, &kind, row, &records, &clock, &mut budget)?;
    let page = format::Page {
        offset: 0,
        limit: 1,
    };
    Ok(!execute(
        collection,
        &kind,
        &[input],
        page,
        &clock,
        &|_, _| true,
        &mut budget,
    )?
    .rows
    .is_empty())
}

fn execute(
    collection: &Collection,
    kind: &Kind<'_>,
    inputs: &[QueryInput],
    page: format::Page,
    clock: &(String, String),
    matches_row: &dyn Fn(usize, format::Row<'_>) -> bool,
    budget: &mut Budget,
) -> Result<format::QueryResult> {
    let empty = format::Fields::new();
    let rows = inputs
        .iter()
        .map(|input| format::Row {
            fields: &input.fields,
            metadata: &input.metadata,
        })
        .collect::<Vec<_>>();
    let records_by_item = inputs
        .iter()
        .map(|input| {
            input
                .records
                .iter()
                .map(|record| format::Row {
                    fields: &empty,
                    metadata: record,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let contexts = inputs
        .iter()
        .zip(&records_by_item)
        .map(|(input, records)| {
            let format::Value::Text(id) = &input.metadata["id"] else {
                unreachable!("host item ID")
            };
            (
                id.clone(),
                format::QueryRowContext {
                    state: input.state.as_ref(),
                    records,
                    record_positions: input.state.as_ref().map(|_| &input.record_positions),
                    ..format::QueryRowContext::default()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let context = format::EvalContext {
        today: Some(&clock.0),
        now: Some(&clock.1),
        offset_seconds: Some(0),
        rows: Some(&contexts),
        ..format::EvalContext::default()
    };
    budget.work(|steps| {
        format::evaluate_query_bounded(
            collection,
            kind.compiled,
            &rows,
            page,
            &context,
            matches_row,
            steps,
        )
    })
}

pub(crate) fn state(
    space: &mut Space,
    actor: &Actor,
    input: State,
    key: Option<String>,
) -> Result<Value> {
    let (item, name, set) = match input {
        State::Get { item, key } => (item, key, None),
        State::Set { item, key, value } => (item, key, Some(value)),
    };
    let row = space.row(item)?;
    space.permits(actor, &row.head.kind, true)?;
    let kind = space.kind(&row.head.kind)?;
    let field = kind
        .state
        .get(&name)
        .ok_or_else(|| Error::invalid_input("State key is undeclared."))?;
    let owner = row
        .head
        .kind
        .split_once('.')
        .expect("qualified kind")
        .0
        .to_owned();
    let Some(value) = set else {
        return Ok(json!({"value":state_value(space, actor, &kind, item, &name)?}));
    };
    if !value.is_null()
        && !field
            .field_type
            .as_ref()
            .is_some_and(|field| declarations::accepts(field, &value))
    {
        return Err(Error::invalid_input("State value has the wrong type."));
    }
    let effective = if value.is_null() {
        declarations::field_default(field)?.unwrap_or_default()
    } else {
        value.clone()
    };
    let response = json!({"value":effective});
    let serialized = value.to_string();
    if serialized.len() > 65_536 {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "Viewer state exceeds 64 KiB for this plugin.",
        ));
    }
    let receipt = space.receipt(
        &format!("state:{}", serde_json::to_string(actor).map_err(internal)?),
        key.as_deref(),
        &serde_json::to_vec(&(item, &name, &value)).map_err(internal)?,
    )?;
    if let Some(replay) = receipt.as_ref().and_then(|receipt| receipt.replay.clone()) {
        return Ok(replay);
    }
    let account = actor.account.to_string();
    let is_anchor = field.field_type == Some(FieldType::Anchor);
    let tx = space.db.transaction().map_err(internal)?;
    let ordinal = if is_anchor && !value.is_null() {
        let ordinal: Option<i64> = tx
            .query_row(
                "SELECT ordinal FROM records WHERE item=?1 AND anchor=?2",
                params![item.to_string(), value.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(internal)?;
        Some(ordinal.ok_or_else(|| Error::not_found("Record anchor was not found on this item."))?)
    } else {
        None
    };
    if value.is_null() {
        tx.execute(
            "DELETE FROM viewer_state WHERE plugin=?1 AND item=?2 AND name=?3 AND account=?4",
            params![owner, item.to_string(), name, account],
        )
        .map_err(internal)?;
    } else {
        let size: i64 = tx.query_row("SELECT COALESCE(SUM(length(CAST(value AS BLOB)) + length(CAST(name AS BLOB)) + length(item)),0)
            FROM viewer_state WHERE plugin=?1 AND account=?4 AND NOT(item=?2 AND name=?3)",
            params![owner, item.to_string(), name, account], |row| row.get(0)).map_err(internal)?;
        if size as usize + serialized.len() + name.len() + 36 > 65_536 {
            return Err(Error::new(
                ErrorCode::TooLarge,
                "Viewer state exceeds 64 KiB for this plugin.",
            ));
        }
        tx.execute(
            "INSERT INTO viewer_state VALUES(?1,?2,?3,?4,?5,?6)
            ON CONFLICT DO UPDATE SET value=excluded.value, ordinal=excluded.ordinal",
            params![owner, item.to_string(), name, serialized, ordinal, account],
        )
        .map_err(internal)?;
    }
    if let Some(receipt) = receipt {
        receipt.store(&tx, &response, calternal_types::now_ms())?;
    }
    tx.commit().map_err(internal)?;
    Ok(response)
}

fn state_value(space: &Space, actor: &Actor, kind: &Kind, item: Uuid, name: &str) -> Result<Value> {
    let field = kind
        .state
        .get(name)
        .ok_or_else(|| Error::invalid_input("State key is undeclared."))?;
    let owner = kind.id.split_once('.').expect("qualified kind").0;
    let stored: Option<String> = space
        .db
        .query_row(
            "SELECT value FROM viewer_state WHERE plugin=?1 AND item=?2 AND name=?3 AND account=?4",
            params![owner, item.to_string(), name, actor.account.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(internal)?;
    stored
        .map(|raw| serde_json::from_str(&raw).map_err(internal))
        .unwrap_or_else(|| Ok(declarations::field_default(field)?.unwrap_or_default()))
}
