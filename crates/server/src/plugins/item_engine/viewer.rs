//! The acting person's private record for one item (catalogue
//! `viewer_item_state`): declared viewer fields under a versioned
//! compare-and-set, stored as one space control row per item, owning
//! package and account. Host-maintained reading positions and drafts share
//! the record; declared field patches cannot reach them.
use super::*;

/// UTF-8 byte bound of one Text viewer field.
const TEXT_BYTES: usize = 4096;
/// Declared scalars kept per record.
const FIELDS: usize = 16;

#[derive(Deserialize)]
pub(super) struct ViewerRead {
    item: ItemRef,
}

#[derive(Deserialize)]
pub(super) struct ViewerWrite {
    item: ItemRef,
    expected: String,
    request_id: RequestId,
    patch: serde_json::Map<String, Value>,
}

impl ItemEngine {
    fn viewer_key(&self, item: Uuid) -> String {
        format!(
            "viewer/{item}/{}/{}",
            self.package.id(),
            self.actor.account
        )
    }

    /// The record shape returned to the worker: `version` and fields.
    /// A record that was never written reads as version `0` with no fields.
    fn viewer_record(stored: Option<Value>) -> Value {
        let mut record = stored.unwrap_or_else(|| json!({"version":"0","fields":{}}));
        record
            .as_object_mut()
            .map(|fields| fields.retain(|key, _| key == "version" || key == "fields"));
        record
    }

    /// Read the acting person's own record under live item access.
    /// Cost: one indexed ref lookup and one control read; no flush.
    pub(super) fn viewer_read(&self, input: ViewerRead) -> Result<Value, Error> {
        let item = self.resolve(&input.item)?;
        self.access(ItemCheck::Head, Some(item), None)?;
        let record = self.content.control(self.space, &self.viewer_key(item))?;
        self.access(ItemCheck::Deliver, Some(item), None)?;
        Ok(Self::viewer_record(record))
    }

    /// Apply a declared-field patch with compare-and-set on `expected`.
    /// `null` clears a field. Host-sourced counts, undeclared names and
    /// cross-type values are refused before any effect. Cost: one receipt
    /// read and one FULL control commit through the space writer.
    pub(super) fn viewer_write(&self, input: ViewerWrite, operand: &Value) -> Result<Value, Error> {
        let item = self.resolve(&input.item)?;
        self.access(ItemCheck::Head, Some(item), None)?;
        let declared = &self.kind()["viewer_fields"];
        for (name, value) in &input.patch {
            let field = declared.get(name).ok_or_else(invalid_field)?;
            if field.get("source").is_some() || !admissible(field["type"].as_str(), value) {
                return Err(invalid_field());
            }
        }
        let fingerprint = self.fingerprint("viewer_state_write", operand, None)?;
        let key = receipt_key(&input.request_id);
        let request = Request {
            account: self.actor.account,
            via: self.actor.via.as_ref(),
            id: &key,
            operation: "viewer_state_write",
            fingerprint: &fingerprint,
        };
        let record_key = self.viewer_key(item);
        let result = self.content.set_control(self.space, request, &record_key, |current| {
            let current = Self::viewer_record(current);
            if current["version"] != input.expected.as_str() {
                return Err(Error::conflict("viewer state has a newer version")
                    .with_details(json!({"reason":"stale_version","current":current})));
            }
            let mut fields = current["fields"].as_object().cloned().unwrap_or_default();
            for (name, value) in input.patch {
                if value.is_null() {
                    fields.remove(&name);
                } else {
                    fields.insert(name, value);
                }
            }
            if fields.len() > FIELDS {
                return Err(invalid_field());
            }
            let version = current["version"]
                .as_str()
                .and_then(|version| version.parse::<u64>().ok())
                .ok_or_else(|| Error::internal("corrupt viewer state"))?
                + 1;
            let next = json!({"version": version.to_string(), "fields": fields});
            Ok((next.clone(), next))
        })?;
        self.access(ItemCheck::Deliver, Some(item), None)?;
        Ok(result)
    }
}

fn admissible(kind: Option<&str>, value: &Value) -> bool {
    match (kind, value) {
        (_, Value::Null) => true,
        (Some("Boolean"), Value::Bool(_)) => true,
        (Some("Number"), Value::Number(_)) => true,
        (Some("Text"), Value::String(text)) => text.len() <= TEXT_BYTES,
        (Some("Timestamp"), Value::String(text)) => {
            text.len() <= 40
                && text.get(17..19).is_some_and(|seconds| seconds != "60")
                && time::OffsetDateTime::parse(
                    text,
                    &time::format_description::well_known::Rfc3339,
                )
                .is_ok()
        }
        _ => false,
    }
}

fn invalid_field() -> Error {
    Error::invalid_input("invalid viewer field")
}
