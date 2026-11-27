//! Query the rebuildable field index and evaluate viewer-dependent values.
use crate::declarations::{Fields, Kind};
use crate::internal;
use crate::space::{Row, Space};
use calternal_types::{Actor, Error, ErrorCode, Result};
use rusqlite::{OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cmp::Ordering;
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
    let (from, filter, sort) = match (&input.from, &input.collection) {
        (Some(from), None) => (from, input.filter.as_deref(), &input.sort),
        (None, Some(collection)) if input.filter.is_none() && input.sort.is_empty() => {
            let declaration = space.collection(collection)?;
            (
                &declaration.from,
                declaration.filter.as_deref(),
                &declaration.sort,
            )
        }
        _ => return Err(Error::invalid_input("Select one kind or one collection.")),
    };
    space.permits(actor, from, false)?;
    let kind = space.kind(from)?;
    let page_size = input.limit.unwrap_or(200);
    if !(1..=200).contains(&page_size) {
        return Err(Error::invalid_input("A query page holds up to 200 rows."));
    }
    let sort = sort
        .iter()
        .map(|field| {
            let (field, descending) = if let Some(key) = field.strip_suffix(" desc") {
                (key, true)
            } else {
                (field.strip_suffix(" asc").unwrap_or(field), false)
            };
            if field != "edited" && field != "manual" && !kind.fields.contains_key(field) {
                return Err(Error::invalid_input("Unknown sort field."));
            }
            Ok((field, descending))
        })
        .collect::<Result<Vec<_>>>()?;
    let filters = [kind.filter.as_deref(), filter]
        .into_iter()
        .flatten()
        .map(|condition| Condition::parse(kind, condition))
        .collect::<Result<Vec<_>>>()?;
    let cursor = input
        .cursor
        .as_deref()
        .map(|id| Uuid::parse_str(id).map_err(|_| Error::invalid_input("Invalid query cursor.")))
        .transpose()?;
    let mut rows = Vec::new();
    for row in space.rows.values().filter(|row| row.head.kind == *from) {
        let mut allowed = true;
        for filter in &filters {
            if !filter.matches(space, kind, row)? {
                allowed = false;
                break;
            }
        }
        if allowed {
            let sort_keys = sort
                .iter()
                .map(|(field, _)| {
                    if *field == "edited" {
                        Ok(json!(row.head.edited))
                    } else if *field == "manual" {
                        Ok(Value::Null)
                    } else if let Some(formula) =
                        kind.fields.get(*field).and_then(|f| f.computed.as_deref())
                    {
                        evaluate(space, kind, row, formula)
                    } else {
                        Ok(row.fields.get(*field).cloned().unwrap_or(Value::Null))
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            rows.push((row, sort_keys));
        }
    }
    rows.sort_by(|(a, keys_a), (b, keys_b)| {
        sort.iter()
            .zip(keys_a.iter().zip(keys_b))
            .map(|((_, desc), (a, b))| {
                let order = compare(a, b);
                if *desc { order.reverse() } else { order }
            })
            .find(|order| *order != Ordering::Equal)
            .unwrap_or_else(|| a.head.id.cmp(&b.head.id))
    });
    let start = cursor
        .map(|cursor| {
            rows.iter()
                .position(|(row, _)| row.head.id == cursor)
                .map(|n| n + 1)
                .ok_or_else(|| Error::conflict("The query cursor is no longer present."))
        })
        .transpose()?
        .unwrap_or(0);
    let page = calternal_types::Page::from_lookahead(
        rows.into_iter().skip(start).take(page_size + 1).collect(),
        page_size,
        |(row, _)| row.head.id,
    );
    let results = page
        .rows
        .iter()
        .map(|(row, _)| render_row(space, kind, row))
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({"rows":results,"totals":{},"cursor":page.next}))
}

pub(crate) fn read(space: &Space, actor: &Actor, input: Read) -> Result<Value> {
    let row = space.row(input.item)?;
    space.permits(actor, &row.head.kind, false)?;
    let kind = space.kind(&row.head.kind)?;
    let bytes = space.read_item(&row.head)?;
    let records = space.records(input.item)?;
    let views = space.record_views(&row.head, &bytes, &records, kind)?;
    let prefix = std::str::from_utf8(&bytes[..row.head.prefix as usize]).map_err(internal)?;
    let (_, body) = crate::declarations::parse_item(prefix)?;
    let mut result = render_row(space, kind, row)?;
    result["body"] = json!(body);
    result["records"] = json!(views);
    Ok(result)
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
        return Ok(json!({"value":state_value(space, kind, item, &name)?}));
    };
    if !value.is_null() && !field.accepts(&value) {
        return Err(Error::invalid_input("State value has the wrong type."));
    }
    let effective = if value.is_null() {
        field.default.clone().unwrap_or_default()
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
    let is_anchor = field.anchor();
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
            "DELETE FROM viewer_state WHERE plugin=?1 AND item=?2 AND name=?3",
            params![owner, item.to_string(), name],
        )
        .map_err(internal)?;
    } else {
        let size: i64 = tx.query_row("SELECT COALESCE(SUM(length(CAST(value AS BLOB)) + length(CAST(name AS BLOB)) + length(item)),0)
            FROM viewer_state WHERE plugin=?1 AND NOT(item=?2 AND name=?3)",
            params![owner, item.to_string(), name], |row| row.get(0)).map_err(internal)?;
        if size as usize + serialized.len() + name.len() + 36 > 65_536 {
            return Err(Error::new(
                ErrorCode::TooLarge,
                "Viewer state exceeds 64 KiB for this plugin.",
            ));
        }
        tx.execute(
            "INSERT INTO viewer_state VALUES(?1,?2,?3,?4,?5)
            ON CONFLICT DO UPDATE SET value=excluded.value, ordinal=excluded.ordinal",
            params![owner, item.to_string(), name, serialized, ordinal],
        )
        .map_err(internal)?;
    }
    if let Some(receipt) = receipt {
        receipt.store(&tx, &response, calternal_types::now_ms())?;
    }
    tx.commit().map_err(internal)?;
    Ok(response)
}

fn state_value(space: &Space, kind: &Kind, item: Uuid, name: &str) -> Result<Value> {
    let field = kind
        .state
        .get(name)
        .ok_or_else(|| Error::invalid_input("State key is undeclared."))?;
    let owner = kind.id.split_once('.').expect("qualified kind").0;
    let stored: Option<String> = space
        .db
        .query_row(
            "SELECT value FROM viewer_state WHERE plugin=?1 AND item=?2 AND name=?3",
            params![owner, item.to_string(), name],
            |row| row.get(0),
        )
        .optional()
        .map_err(internal)?;
    stored
        .map(|raw| serde_json::from_str(&raw).map_err(internal))
        .unwrap_or_else(|| Ok(field.default.clone().unwrap_or_default()))
}

fn render_row(space: &Space, kind: &Kind, row: &Row) -> Result<Value> {
    let computed = kind
        .fields
        .iter()
        .filter_map(|(key, field)| field.computed.as_deref().map(|f| (key, f)))
        .map(|(key, formula)| Ok((key.clone(), evaluate(space, kind, row, formula)?)))
        .collect::<Result<Fields>>()?;
    Ok(
        json!({"id":row.head.id,"kind":row.head.kind,"fields":row.fields,"computed":computed,"version":row.head.version}),
    )
}

fn evaluate(space: &Space, kind: &Kind, row: &Row, formula: &str) -> Result<Value> {
    if let Some(key) = formula
        .strip_prefix("count(records after state.")
        .and_then(|f| f.strip_suffix(')'))
    {
        if !kind.state.get(key).is_some_and(|field| field.anchor()) {
            return Err(Error::invalid_input(
                "Record position needs a declared anchor key.",
            ));
        }
        let owner = kind.id.split_once('.').expect("qualified kind").0;
        let count: i64 = space
            .db
            .query_row(
                "SELECT count(*) FROM records WHERE item=?1 AND ordinal>COALESCE(
                (SELECT ordinal FROM viewer_state WHERE plugin=?2 AND item=?1 AND name=?3),0)",
                params![row.head.id.to_string(), owner, key],
                |row| row.get(0),
            )
            .map_err(internal)?;
        return Ok(json!(count));
    }
    // Temporary format-engine stub for the record-count expression. The
    // declaration supplies the state key, and the ledger supplies the times.
    if let Some(inside) = formula
        .strip_prefix("if(state.")
        .and_then(|f| f.strip_suffix("))"))
        && let Some((key, comparison)) =
            inside.split_once(" = empty, count(records), count(records, at > state.")
        && key == comparison
    {
        let value = state_value(space, kind, row.head.id, key)?;
        let after = value
            .as_str()
            .and_then(|raw| {
                time::OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .map(|at| (at.unix_timestamp_nanos() / 1_000_000) as i64)
            .unwrap_or(i64::MIN);
        let count: i64 = space
            .db
            .query_row(
                "SELECT count(*) FROM records WHERE item=?1 AND at>?2",
                params![row.head.id.to_string(), after],
                |row| row.get(0),
            )
            .map_err(internal)?;
        return Ok(json!(count));
    }
    Err(Error::invalid_input("The formula needs the format engine."))
}

enum Condition<'a> {
    Truth {
        key: &'a str,
        negated: bool,
    },
    Compare {
        key: &'a str,
        count: bool,
        op: &'a str,
        value: Value,
    },
    And(Vec<Condition<'a>>),
}

impl<'a> Condition<'a> {
    fn parse(kind: &Kind, source: &'a str) -> Result<Self> {
        if source.contains(" and ") {
            return Ok(Self::And(
                source
                    .split(" and ")
                    .map(|part| Self::parse(kind, part))
                    .collect::<Result<Vec<_>>>()?,
            ));
        }
        let valid = |key: &str| {
            kind.fields.contains_key(key)
                || key
                    .strip_prefix("state.")
                    .is_some_and(|key| kind.state.contains_key(key))
        };
        for op in [" >= ", " <= ", " != ", " = ", " > ", " < "] {
            if let Some((key, raw)) = source.split_once(op) {
                let count = key
                    .strip_prefix("count(")
                    .and_then(|key| key.strip_suffix(')'));
                let key = count.unwrap_or(key);
                if valid(key) {
                    let value = match raw {
                        "yes" => Value::Bool(true),
                        "no" => Value::Bool(false),
                        "empty" => Value::Null,
                        _ => serde_json::from_str(raw)
                            .map_err(|_| Error::invalid_input("Unsupported filter value."))?,
                    };
                    return Ok(Self::Compare {
                        key,
                        count: count.is_some(),
                        op: op.trim(),
                        value,
                    });
                }
            }
        }
        let (key, negated) = source
            .strip_prefix("not ")
            .map_or((source, false), |key| (key, true));
        if valid(key) {
            return Ok(Self::Truth { key, negated });
        }
        Err(Error::invalid_input("Unsupported query condition."))
    }

    fn matches(&self, space: &Space, kind: &Kind, row: &Row) -> Result<bool> {
        let value = |key: &str| -> Result<Value> {
            if let Some(key) = key.strip_prefix("state.") {
                state_value(space, kind, row.head.id, key)
            } else if let Some(formula) = kind
                .fields
                .get(key)
                .and_then(|field| field.computed.as_deref())
            {
                evaluate(space, kind, row, formula)
            } else {
                Ok(row.fields.get(key).cloned().unwrap_or_default())
            }
        };
        match self {
            Self::Truth { key, negated } => Ok(value(key)?.as_bool().unwrap_or(false) != *negated),
            Self::Compare {
                key,
                count,
                op,
                value: expected,
            } => {
                let mut actual = value(key)?;
                if *count {
                    actual = json!(actual.as_array().map_or(0, Vec::len));
                }
                let order = compare(&actual, expected);
                Ok(match *op {
                    "=" => actual == *expected,
                    "!=" => actual != *expected,
                    ">" => order.is_gt(),
                    "<" => order.is_lt(),
                    ">=" => order.is_ge(),
                    "<=" => order.is_le(),
                    _ => unreachable!(),
                })
            }
            Self::And(conditions) => {
                for condition in conditions {
                    if !condition.matches(space, kind, row)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
}

fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Less,
        (_, Value::Null) => Ordering::Greater,
        (Value::Number(a), Value::Number(b)) => a
            .as_f64()
            .zip(b.as_f64())
            .map_or(Ordering::Equal, |(a, b)| a.total_cmp(&b)),
        (Value::String(a), Value::String(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => a.to_string().cmp(&b.to_string()),
    }
}
