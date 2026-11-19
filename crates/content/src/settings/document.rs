//! Versioned settings document and per-setting conflict handling.
use serde::{Deserialize, Serialize, de, ser::SerializeMap};
use serde_json::{Value, value::RawValue};
use std::collections::BTreeMap;

pub(super) const FORMAT: &str = "calternal-plugin-settings/v1";
pub(super) const MAX_FILE_BYTES: usize = 1024 * 1024;
pub(super) const MAX_VALUE_BYTES: usize = 4096;
pub(super) const MAX_ALTERNATIVES: usize = 8;

// Keep untouched records verbatim, including numbers outside Rust's ordinary
// JSON number range. A duplicate member makes the document unsafe to rewrite.
#[derive(Default, Deserialize, Serialize)]
#[serde(transparent)]
struct Records(#[serde(deserialize_with = "setting_members")] BTreeMap<String, Box<RawValue>>);

fn setting_members<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, Box<RawValue>>, D::Error> {
    calternal_types::unique_json::deserialize_unique_members_with_error(d, "duplicate setting key")
}

#[derive(Default)]
pub(super) struct Document {
    settings: Records,
    conflicts: Records,
    additional: Records,
}

impl Serialize for Document {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(3 + self.additional.0.len()))?;
        map.serialize_entry("format", FORMAT)?;
        map.serialize_entry("settings", &self.settings)?;
        map.serialize_entry("conflicts", &self.conflicts)?;
        for (key, value) in &self.additional.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl Document {
    pub(super) fn read(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        let mut envelope: Records = serde_json::from_slice(bytes)?;
        let mut take = |key| {
            envelope
                .0
                .remove(key)
                .ok_or_else(|| de::Error::missing_field(key))
        };
        let version: String = serde_json::from_str(take("format")?.get())?;
        if version != FORMAT {
            return Err(de::Error::custom("unsupported settings format"));
        }
        Ok(Self {
            settings: serde_json::from_str(take("settings")?.get())?,
            conflicts: serde_json::from_str(take("conflicts")?.get())?,
            additional: envelope,
        })
    }

    pub(super) fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }

    pub(super) fn value(&self, key: &str) -> Value {
        self.settings
            .0
            .get(key)
            .and_then(|raw| serde_json::from_str(raw.get()).ok())
            .unwrap_or_default()
    }

    pub(super) fn alternatives(&self, key: &str) -> Result<Vec<Value>, serde_json::Error> {
        self.conflicts
            .0
            .get(key)
            .map(|raw| serde_json::from_str(raw.get()))
            .unwrap_or_else(|| Ok(Vec::new()))
    }

    // The base applies to one setting, so unrelated offline edits merge. A
    // resolving writer echoes every candidate it saw. A racing new candidate
    // remains available until it too has been reviewed.
    pub(super) fn edit(
        &mut self,
        key: &str,
        base: &Value,
        value: &Value,
        resolve: Option<&[Value]>,
    ) -> Result<bool, calternal_types::Error> {
        let mut alternatives = self
            .alternatives(key)
            .map_err(|err| super::repair(err.line(), "repair the conflict array"))?;
        let current = self.value(key);
        let raw = serde_json::value::to_raw_value(value).map_err(crate::types::internal)?;
        if raw.get().len() > MAX_VALUE_BYTES {
            return Err(calternal_types::Error::invalid_input(
                "setting exceeds 4 KiB",
            ));
        }
        if let Some(seen) = resolve {
            if *base != current || seen != alternatives {
                return Err(calternal_types::Error::conflict(
                    "setting changed; read its current values before resolving",
                ));
            }
            self.conflicts.0.remove(key);
        } else if current == *value || alternatives.contains(value) {
            return Ok(!alternatives.is_empty());
        } else if current != *base || !alternatives.is_empty() {
            if alternatives.len() >= MAX_ALTERNATIVES {
                return Err(calternal_types::Error::conflict(
                    "resolve the existing setting alternatives before retrying this edit",
                ));
            }
            alternatives.push(value.clone());
            self.conflicts.0.insert(
                key.into(),
                serde_json::value::to_raw_value(&alternatives).map_err(crate::types::internal)?,
            );
            return Ok(true);
        }
        if value.is_null() {
            self.settings.0.remove(key);
        } else {
            self.settings.0.insert(key.into(), raw);
        }
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_unknown_raw_records_and_rejects_duplicate_members() {
        let bytes = format!(
            r#"{{"format":"{FORMAT}","settings":{{"old/count":{{"nested":18446744073709551616000000000}}}},"conflicts":{{}},"future":[1,2]}}"#
        );
        let mut document = Document::read(bytes.as_bytes()).unwrap();
        document
            .edit("new/enabled", &Value::Null, &json!(true), None)
            .unwrap();
        let saved = String::from_utf8(document.encode().unwrap()).unwrap();
        assert!(saved.contains(r#"{"nested":18446744073709551616000000000}"#));
        assert!(saved.contains(r#"[1,2]"#));
        assert_eq!(
            Document::read(saved.as_bytes())
                .unwrap()
                .value("new/enabled"),
            json!(true)
        );
        let duplicate = bytes.replace(r#""conflicts":{}"#, r#""conflicts":{},"conflicts":{}"#);
        assert!(Document::read(duplicate.as_bytes()).is_err());
        assert!(Document::read(bytes.replace("/v1", "/v2").as_bytes()).is_err());
    }

    #[test]
    fn retains_overlapping_edits_until_an_observed_resolution() {
        let mut document = Document::default();
        let missing = &Value::Null;
        assert!(!document.edit("one", missing, &json!(1), None).unwrap());
        assert!(!document.edit("two", missing, &json!(2), None).unwrap());
        assert!(document.edit("one", missing, &json!(3), None).unwrap());
        assert!(document.edit("one", &json!(1), &json!(4), None).unwrap());
        assert!(
            document
                .edit("one", &json!(1), &json!(3), Some(&[json!(3)]))
                .is_err()
        );
        assert_eq!(document.value("one"), json!(1));
        assert_eq!(document.alternatives("one").unwrap(), [json!(3), json!(4)]);
        assert!(
            !document
                .edit("one", &json!(1), missing, Some(&[json!(3), json!(4)]))
                .unwrap()
        );
        assert_eq!(document.value("one"), *missing);
        assert_eq!(document.value("two"), json!(2));
    }
}
