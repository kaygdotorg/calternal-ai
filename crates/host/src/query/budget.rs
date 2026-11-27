//! Charge query work before loading or cloning candidates, values and records.

use crate::{
    internal,
    records::Record,
    space::{Row, Space},
};
use calternal_types::{Error, ErrorCode, Result};
use rusqlite::params;
use uuid::Uuid;

const QUERY_CANDIDATES: usize = 2_048;
const QUERY_RECORDS: usize = 4_096;
const QUERY_BYTES: u64 = 16 * 1024 * 1024;

pub(super) struct Budget {
    candidates: usize,
    records: usize,
    bytes: u64,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            candidates: QUERY_CANDIDATES,
            records: QUERY_RECORDS,
            bytes: QUERY_BYTES,
        }
    }
}

impl Budget {
    pub(super) fn row(&mut self, row: &Row) -> Result<()> {
        self.candidates = self.candidates.checked_sub(1).ok_or_else(over_budget)?;
        self.bytes(row.head.prefix.saturating_add(256))
    }

    pub(super) fn bytes(&mut self, amount: u64) -> Result<()> {
        self.bytes = self.bytes.checked_sub(amount).ok_or_else(over_budget)?;
        Ok(())
    }

    pub(super) fn limit(&self) -> usize {
        self.bytes as usize
    }

    pub(super) fn work<T>(
        &mut self,
        evaluate: impl FnOnce(&mut usize) -> std::result::Result<T, calternal_format::Diagnostic>,
    ) -> Result<T> {
        let initial = self.limit() / 64;
        let mut steps = initial;
        let result = evaluate(&mut steps).map_err(|error| {
            if error.message.contains("too costly") {
                over_budget()
            } else {
                crate::declarations::invalid(error)
            }
        });
        self.bytes(((initial - steps) * 64) as u64)?;
        result
    }

    pub(super) fn format(&mut self, key: &str, value: &calternal_format::Value) -> Result<()> {
        self.bytes(key.len().saturating_add(value.copy_bytes()) as u64)
    }

    pub(super) fn json(&mut self, key: &str, value: &serde_json::Value) -> Result<()> {
        use serde_json::Value::*;
        self.bytes((key.len() + 64) as u64)?;
        match value {
            String(text) => self.bytes(text.len() as u64),
            Array(values) => values.iter().try_for_each(|value| self.json("", value)),
            Object(values) => values
                .iter()
                .try_for_each(|(key, value)| self.json(key, value)),
            _ => Ok(()),
        }
    }

    pub(super) fn records(&mut self, space: &Space, item: Uuid) -> Result<Vec<Record>> {
        let count = space
            .db
            .prepare_cached("SELECT count(*) FROM (SELECT 1 FROM records WHERE item=?1 LIMIT ?2)")
            .map_err(internal)?
            .query_row(
                params![item.to_string(), (self.records + 1) as i64],
                |row| row.get::<_, i64>(0),
            )
            .map_err(internal)?;
        self.records = self
            .records
            .checked_sub(count as usize)
            .ok_or_else(over_budget)?;
        self.bytes(count as u64 * 256)?;
        if count == 0 {
            Ok(Vec::new())
        } else {
            space.records(item)
        }
    }
}

fn over_budget() -> Error {
    Error::new(
        ErrorCode::TooLarge,
        "The query exceeds the host work budget.",
    )
}
