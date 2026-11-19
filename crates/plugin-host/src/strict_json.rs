//! Bounded JSON that rejects duplicate object keys at every depth.
use crate::Error;
use serde::de::{self, DeserializeSeed, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};
use std::fmt;

const MAX_DEPTH: u8 = 32;

struct Strict(u8);

impl<'de> DeserializeSeed<'de> for Strict {
    type Value = Value;

    fn deserialize<D: de::Deserializer<'de>>(self, de: D) -> Result<Value, D::Error> {
        de.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Strict {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bounded JSON with unique keys")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
        if self.0 == MAX_DEPTH {
            return Err(de::Error::custom("JSON is too deep"));
        }
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON key"));
            }
            object.insert(key, map.next_value_seed(Strict(self.0 + 1))?);
        }
        Ok(Value::Object(object))
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut seq: S) -> Result<Value, S::Error> {
        if self.0 == MAX_DEPTH {
            return Err(de::Error::custom("JSON is too deep"));
        }
        let mut items = Vec::new();
        while let Some(value) = seq.next_element_seed(Strict(self.0 + 1))? {
            items.push(value);
        }
        Ok(Value::Array(items))
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid JSON number"))
    }
    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
}

pub(crate) fn parse<T: de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let value = Strict(0)
        .deserialize(&mut deserializer)
        .map_err(|_| Error::Invalid)?;
    deserializer.end().map_err(|_| Error::Invalid)?;
    serde_json::from_value(value).map_err(|_| Error::Invalid)
}

#[cfg(test)]
mod tests {
    use crate::{Error, decode_frame};

    #[test]
    fn child_decoder_rejects_ambiguous_and_unbounded_json() {
        let frame = |input: &str| {
            format!(
                r#"{{"protocol_version":2,"invocation_id":"i1","message":{{"type":"complete","data":{input}}}}}"#
            )
        };
        let deepest = format!("{}0{}", "[".repeat(30), "]".repeat(30));
        assert!(decode_frame(frame(&deepest).as_bytes()).is_ok());

        let too_deep = format!("{}0{}", "[".repeat(31), "]".repeat(31));
        for input in [
            too_deep.as_str(),
            r#"[{"first":false,"first":true}]"#,
            r#"{"nested":{"key":null,"\u006bey":null}}"#,
        ] {
            assert_eq!(decode_frame(frame(input).as_bytes()), Err(Error::Invalid));
        }

        let mut invalid_utf8 = frame(r#""text""#).into_bytes();
        let text = invalid_utf8.len() - 7;
        invalid_utf8[text] = 0xff;
        assert_eq!(decode_frame(&invalid_utf8), Err(Error::Invalid));
    }
}
