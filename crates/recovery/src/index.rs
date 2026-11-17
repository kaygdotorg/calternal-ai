use crate::{
    Result, err, invalid,
    model::{decode, encode},
    storage::Storage,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Authenticated current version of one namespaced logical security record.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RecordRef {
    /// Namespace nibble and canonical UUID identifying this logical record.
    pub key: String,
    /// Hash of the complete immutable serialized record file.
    pub digest: String,
}
impl RecordRef {
    /// Selects the credential-private or space-authority directory for this namespace.
    pub(crate) fn sub(&self) -> &str {
        if self.key.starts_with('2') {
            "spaces"
        } else {
            "accounts"
        }
    }
    /// Derives the immutable version filename from its canonical key and digest.
    pub(crate) fn name(&self) -> String {
        format!("{}-{}.json", self.key, self.digest)
    }
    /// Returns the protected relative path used in a retirement inventory.
    pub(crate) fn path(&self) -> String {
        format!("{}/{}", self.sub(), self.name())
    }
}
/// One immutable 16-way branch or terminal record reference in the 33-nibble index.
#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub(crate) enum Node {
    /// Authenticated references for the next hexadecimal key digit.
    Branch([Option<String>; 16]),
    /// Terminal current record version, bound to the complete index key.
    Leaf(RecordRef),
}
/// Hashes exact serialized file bytes, including the checksum envelope.
pub(crate) fn digest(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
/// Checks a canonical lowercase BLAKE3 digest before using it as a path component.
pub(crate) fn hash_ok(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
/// Checks the fixed namespace-and-UUID radix key without accepting path characters.
pub(crate) fn key_ok(key: &str) -> bool {
    key.len() == 33
        && matches!(key.as_bytes()[0], b'0'..=b'4')
        && key
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
/// Loads a node only if its actual bytes match the parent-provided digest.
pub(crate) fn load(store: &Storage, hash: &str) -> Result<Node> {
    if !hash_ok(hash) {
        return Err(invalid());
    }
    let bytes = store.read("index", &format!("{hash}.json"))?;
    if digest(&bytes) != hash {
        return Err(invalid());
    }
    decode(&bytes)
}
/// Looks up one current record in at most 34 authenticated node reads.
pub(crate) fn get(store: &Storage, root: Option<&str>, key: &str) -> Result<Option<RecordRef>> {
    if !key_ok(key) {
        return Err(invalid());
    }
    let mut hash = root.map(str::to_owned);
    for depth in 0..=33 {
        let Some(current) = hash else { return Ok(None) };
        match load(store, &current)? {
            Node::Leaf(r) if depth == 33 && r.key == key => return Ok(Some(r)),
            Node::Branch(children) if depth < 33 => {
                let i = usize::from_str_radix(&key[depth..depth + 1], 16).map_err(err)?;
                hash = children[i].clone();
            }
            _ => return Err(invalid()),
        }
    }
    Err(invalid())
}

/// New index nodes and exact replaced paths for bounded, replayable collection.
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct Changes {
    /// Only new nodes needed by affected paths, indexed by their file digest.
    pub nodes: BTreeMap<String, Node>,
    /// Replaced immutable paths, safe to unlink only after new-head publication.
    pub retired: Vec<String>,
}
/// Copies only affected radix paths and retires replaced nodes/record versions.
pub(crate) fn update(
    store: &Storage,
    old: Option<String>,
    changes: &[(String, Option<RecordRef>)],
    depth: usize,
    output: &mut Changes,
) -> Result<Option<String>> {
    if changes.is_empty() {
        return Ok(old);
    }
    let node = old.as_deref().map(|h| load(store, h)).transpose()?;
    let next = if depth == 33 {
        if changes.len() != 1 {
            return Err(invalid());
        }
        if let Some(Node::Leaf(previous)) = &node {
            if previous.key != changes[0].0 {
                return Err(invalid());
            }
            if changes[0].1.as_ref() != Some(previous) {
                output.retired.push(previous.path());
            }
        } else if node.is_some() {
            return Err(invalid());
        }
        changes[0].1.clone().map(Node::Leaf)
    } else {
        let mut children = match node {
            None => Default::default(),
            Some(Node::Branch(c)) => c,
            _ => return Err(invalid()),
        };
        for (i, child) in children.iter_mut().enumerate() {
            let subset = changes
                .iter()
                .filter(|(k, _)| usize::from_str_radix(&k[depth..depth + 1], 16).ok() == Some(i))
                .cloned()
                .collect::<Vec<_>>();
            if !subset.is_empty() {
                *child = update(store, child.clone(), &subset, depth + 1, output)?;
            }
        }
        children
            .iter()
            .any(Option::is_some)
            .then_some(Node::Branch(children))
    };
    let hash = next
        .as_ref()
        .map(|node| encode(node).map(|b| digest(&b)))
        .transpose()?;
    if hash != old {
        if let Some(old) = old {
            output.retired.push(format!("index/{old}.json"));
        }
        if let (Some(hash), Some(node)) = (&hash, next) {
            output.nodes.insert(hash.clone(), node);
        }
    }
    Ok(hash)
}
/// Traverses and validates every current key for an explicit full-snapshot read.
pub(crate) fn visit(
    store: &Storage,
    root: Option<&str>,
    prefix: &mut String,
    records: &mut Vec<RecordRef>,
) -> Result<()> {
    let Some(hash) = root else { return Ok(()) };
    match load(store, hash)? {
        Node::Leaf(record)
            if prefix.len() == 33
                && record.key == *prefix
                && key_ok(&record.key)
                && hash_ok(&record.digest) =>
        {
            records.push(record)
        }
        Node::Branch(children) if prefix.len() < 33 => {
            for (i, child) in children.iter().enumerate() {
                prefix.push(char::from_digit(i as u32, 16).ok_or_else(invalid)?);
                visit(store, child.as_deref(), prefix, records)?;
                prefix.pop();
            }
        }
        _ => return Err(err("invalid authenticated index path")),
    }
    Ok(())
}
