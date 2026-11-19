//! JSON map decoding for documents that reject duplicate keys.
use serde::{Deserialize, Deserializer, de};
use std::{collections::BTreeMap, fmt, marker::PhantomData};

/// Collect exactly one value for each object key. The value type controls
/// whether child objects are decoded or retained verbatim.
pub fn collect_unique_members<'de, M: de::MapAccess<'de>, V: Deserialize<'de>>(
    map: M,
) -> Result<BTreeMap<String, V>, M::Error> {
    collect_members(map, "duplicate object key")
}

fn collect_members<'de, M: de::MapAccess<'de>, V: Deserialize<'de>>(
    mut map: M,
    duplicate: &'static str,
) -> Result<BTreeMap<String, V>, M::Error> {
    let mut out = BTreeMap::new();
    while let Some((key, value)) = map.next_entry::<String, V>()? {
        if out.insert(key, value).is_some() {
            return Err(de::Error::custom(duplicate));
        }
    }
    Ok(out)
}

/// Serde map decoder that rejects duplicate keys before any normalization.
/// Cost: one pass over the map, with a sorted entry per member.
pub fn deserialize_unique_members<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
    d: D,
) -> Result<BTreeMap<String, V>, D::Error> {
    deserialize_unique_members_with_error(d, "duplicate object key")
}

/// Decode a unique-key object using a caller-specific duplicate error.
/// Cost: the same one-pass map decoder as `deserialize_unique_members`.
pub fn deserialize_unique_members_with_error<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
    d: D,
    duplicate: &'static str,
) -> Result<BTreeMap<String, V>, D::Error> {
    struct Unique<V>(PhantomData<V>, &'static str);

    impl<'de, V: Deserialize<'de>> de::Visitor<'de> for Unique<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object with unique keys")
        }

        fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
            collect_members(map, self.1)
        }
    }

    d.deserialize_map(Unique(PhantomData, duplicate))
}
