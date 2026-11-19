//! JSON map decoding for documents that reject duplicate keys.
use serde::{Deserialize, Deserializer, de};
use std::{collections::BTreeMap, fmt, marker::PhantomData};

/// Collect exactly one value for each object key. The value type controls
/// whether child objects are decoded or retained verbatim.
pub fn collect_unique_members<'de, M: de::MapAccess<'de>, V: Deserialize<'de>>(
    mut map: M,
) -> Result<BTreeMap<String, V>, M::Error> {
    let mut out = BTreeMap::new();
    while let Some((key, value)) = map.next_entry::<String, V>()? {
        if out.insert(key, value).is_some() {
            return Err(de::Error::custom("duplicate object key"));
        }
    }
    Ok(out)
}

/// Serde map decoder that rejects duplicate keys before any normalization.
/// Cost: one pass over the map, with a sorted entry per member.
pub fn deserialize_unique_members<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
    d: D,
) -> Result<BTreeMap<String, V>, D::Error> {
    struct Unique<V>(PhantomData<V>);

    impl<'de, V: Deserialize<'de>> de::Visitor<'de> for Unique<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object with unique keys")
        }

        fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
            collect_unique_members(map)
        }
    }

    d.deserialize_map(Unique(PhantomData))
}
