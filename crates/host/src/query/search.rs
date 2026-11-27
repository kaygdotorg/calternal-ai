//! Search the installed kind's declared fields inside the authorized query window.
use crate::declarations;
use crate::space::{Row, Space};
use calternal_format::{self as format, Value};
use calternal_types::{Error, Result};
use std::io::Read;

pub(super) struct Search<'a> {
    needle: Option<String>,
    fields: &'a [String],
    bodies: Option<Vec<bool>>,
}

impl<'a> Search<'a> {
    pub(super) fn new(input: Option<&str>, fields: &'a [String]) -> Result<Self> {
        let needle = input.map(str::trim).filter(|term| !term.is_empty());
        if needle.is_some_and(|term| term.len() > 256) {
            return Err(Error::invalid_input("Search text can be up to 256 bytes."));
        }
        if needle.is_some() && fields.is_empty() {
            return Err(Error::invalid_input(
                "The kind does not declare search fields.",
            ));
        }
        let bodies = (needle.is_some() && fields.iter().any(|name| name == "body")).then(Vec::new);
        Ok(Self {
            needle: needle.map(str::to_lowercase),
            fields,
            bodies,
        })
    }

    pub(super) fn index_body(&mut self, space: &Space, row: &Row) -> Result<()> {
        let (Some(needle), Some(bodies)) = (&self.needle, &mut self.bodies) else {
            return Ok(());
        };
        // The query budget has charged the indexed prefix length. Read and
        // authenticate only that prefix, without loading or searching records.
        let head = &row.head;
        let file = space.open_regular(&head.path).map_err(crate::internal)?;
        let mut bytes = Vec::new();
        file.take(head.prefix)
            .read_to_end(&mut bytes)
            .map_err(crate::internal)?;
        if bytes.len() as u64 != head.prefix || space.hash(&bytes) != head.hash {
            return Err(Error::conflict("The item file changed outside the host."));
        }
        let source = std::str::from_utf8(&bytes).map_err(crate::internal)?;
        bodies.push(
            declarations::item_body(source)?
                .to_lowercase()
                .contains(needle),
        );
        Ok(())
    }

    pub(super) fn matches(&self, index: usize, row: format::Row<'_>) -> bool {
        let Some(needle) = &self.needle else {
            return true;
        };
        self.bodies.as_ref().is_some_and(|bodies| bodies[index])
            || self.fields.iter().any(|field| {
                row.fields
                    .get(field)
                    .is_some_and(|value| contains(value, needle))
            })
    }
}

fn contains(value: &Value, needle: &str) -> bool {
    match value {
        Value::Text(text) | Value::Date(text) | Value::Time(text) => {
            text.to_lowercase().contains(needle)
        }
        Value::List(items) => items.iter().any(|item| contains(item, needle)),
        _ => false,
    }
}
