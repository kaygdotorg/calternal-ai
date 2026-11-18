//! Typed row keys and receipt-only capture for format four.
use crate::{Cell, Result, err, invalid};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(super) struct Key(pub Uuid, pub Uuid, pub String);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Receipt {
    operation: String,
    fingerprint: String,
    result: String,
    at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Changes {
    pub(super) at_ms: i64,
    pub(super) rows: Vec<(Key, Option<Receipt>)>,
}

#[derive(Clone, Default)]
pub(super) struct Rows(pub BTreeMap<Key, Receipt>);

impl Rows {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Serialize for Rows {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter())
    }
}

impl<'de> Deserialize<'de> for Rows {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let rows = Vec::<(Key, Receipt)>::deserialize(deserializer)?;
        if !ordered(&rows) {
            return Err(serde::de::Error::custom(
                "unordered or duplicate receipt keys",
            ));
        }
        Ok(Self(rows.into_iter().collect()))
    }
}

pub(super) fn ordered<T>(rows: &[(Key, T)]) -> bool {
    rows.windows(2).all(|pair| pair[0].0 < pair[1].0)
}

fn row(r: &rusqlite::Row<'_>, offset: usize) -> rusqlite::Result<Receipt> {
    Ok(Receipt {
        operation: r.get(offset)?,
        fingerprint: r.get(offset + 1)?,
        result: r.get(offset + 2)?,
        at_ms: r.get(offset + 3)?,
    })
}

pub(super) fn trigger_key(prefix: &str) -> String {
    format!("'r'||json_array({prefix}.actor,{prefix}.scope,{prefix}.request_id)")
}

pub(super) fn dirty_key(dirty: &str) -> Result<Option<Key>> {
    dirty
        .strip_prefix('r')
        .map(|tuple| serde_json::from_str(tuple).map_err(err))
        .transpose()
}

pub(super) fn mark(c: &Connection, key: &Key) -> Result<()> {
    let encoded = format!("r{}", serde_json::to_string(key).map_err(err)?);
    c.execute(
        "INSERT OR IGNORE INTO security_recovery_dirty VALUES(?1)",
        [encoded],
    )
    .map_err(err)?;
    Ok(())
}

pub(super) fn get(c: &Connection, key: &Key) -> Result<Option<Receipt>> {
    c.prepare_cached("SELECT operation,fingerprint,result,at_ms FROM space_receipts WHERE actor=?1 AND scope=?2 AND request_id=?3")
        .map_err(err)?
        .query_row(params![key.0.to_string(),key.1.to_string(),key.2], |r| row(r, 0))
        .optional()
        .map_err(err)
}

pub(super) fn capture(c: &Connection) -> Result<Rows> {
    let mut statement = c
        .prepare(
            "SELECT actor,scope,request_id,operation,fingerprint,result,at_ms FROM space_receipts",
        )
        .map_err(err)?;
    let mut rows = statement.query([]).map_err(err)?;
    let mut result = BTreeMap::new();
    while let Some(r) = rows.next().map_err(err)? {
        let parse = |index| -> Result<Uuid> {
            let text: String = r.get(index).map_err(err)?;
            let id = Uuid::parse_str(&text).map_err(err)?;
            if text != id.to_string() {
                return Err(invalid());
            }
            Ok(id)
        };
        let key = Key(parse(0)?, parse(1)?, r.get(2).map_err(err)?);
        if result.insert(key, row(r, 3).map_err(err)?).is_some() {
            return Err(invalid());
        }
    }
    Ok(Rows(result))
}

pub(super) fn cells(key: &Key, value: &Receipt) -> Vec<Cell> {
    vec![
        Cell::Text(key.0.to_string()),
        Cell::Text(key.1.to_string()),
        Cell::Text(key.2.clone()),
        Cell::Text(value.operation.clone()),
        Cell::Text(value.fingerprint.clone()),
        Cell::Text(value.result.clone()),
        Cell::Integer(value.at_ms),
    ]
}
