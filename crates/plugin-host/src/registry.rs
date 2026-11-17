//! Manifest parsing, semantic validation and atomic registration.
use crate::Error;
use calternal_core::*;
use calternal_fsio::names::{fold, nfc};
use std::collections::{BTreeMap, BTreeSet};

/// Maximum UTF-8 manifest size before JSON parsing (256 KiB).
pub const MAX_MANIFEST_BYTES: usize = 256 * 1024;

pub(crate) fn identifier(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && id.len() <= 80
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}
fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    ids.into_iter().all(|id| identifier(id) && seen.insert(id))
}
fn folder_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name == name.trim()
        && !name.starts_with('.')
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':'))
        && nfc(name) == name
        && !matches!(fold(name).as_str(), "trash" | ".calternal")
}
fn visible_folders(m: &ManifestV1) -> BTreeSet<String> {
    m.kinds
        .iter()
        .filter_map(|k| match &k.storage {
            ManifestPlacement::Visible { folder } => Some(fold(folder)),
            _ => None,
        })
        .collect()
}
fn schema(value: &serde_json::Value) -> bool {
    (value.is_object() || value.is_boolean()) && jsonschema::validator_for(value).is_ok()
}
/// An operation target names an identifier kind and a top-level input property that
/// the input schema declares as a required string, so the host can always find the
/// item ID before running plugin code. Cost: O(required list), no allocation.
fn target(t: &ManifestTarget, input: &serde_json::Value) -> bool {
    let field = t.field.as_str();
    identifier(&t.kind)
        && !field.is_empty()
        && field.len() <= 64
        && field
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && field != "space_id"
        && input["type"] == "object"
        && input["properties"][field]["type"] == "string"
        && input["required"]
            .as_array()
            .is_some_and(|r| r.iter().any(|v| v == field))
}
/// Cost: O(manifest bytes + schema compilation), bounded allocations, no I/O or locks.
/// Parse a bounded manifest with strict unknown-field rejection, without executing code.
pub fn parse_manifest(bytes: &[u8]) -> Result<ManifestV1, Error> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(Error::TooLarge);
    }
    let manifest = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}
/// Cost: O(manifest bytes + schema compilation); bounded sets/serialization allocations, no I/O.
/// Validate declaration identity, references, primitive defaults and retry consistency.
/// Compiles JSON Schemas with HTTP/file reference retrieval disabled; invalid schemas fail closed.
pub fn validate_manifest(m: &ManifestV1) -> Result<(), Error> {
    crate::bounded::check(m, MAX_MANIFEST_BYTES)?;
    if m.api_version != 1
        || !identifier(&m.id)
        || !identifier(&m.icon)
        || m.name.trim().is_empty()
        || m.name.len() > 256
        || m.version.trim().is_empty()
        || m.version.len() > 80
    {
        return Err(Error::Invalid);
    }
    let groups = [
        m.kinds.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        m.operations.iter().map(|x| x.id.as_str()).collect(),
        m.collections.iter().map(|x| x.id.as_str()).collect(),
        m.views.iter().map(|x| x.id.as_str()).collect(),
        m.attention.iter().map(|x| x.id.as_str()).collect(),
        m.settings.iter().map(|x| x.id.as_str()).collect(),
        m.schedules.iter().map(|x| x.id.as_str()).collect(),
        m.webhooks.iter().map(|x| x.id.as_str()).collect(),
        m.hooks.iter().map(|x| x.id.as_str()).collect(),
        m.themes.iter().map(|x| x.id.as_str()).collect(),
        m.secrets.iter().map(String::as_str).collect(),
    ];
    if groups
        .iter()
        .any(|g| g.len() > 128 || !unique(g.iter().copied()))
        || m.host_calls.iter().collect::<BTreeSet<_>>().len() != m.host_calls.len()
    {
        return Err(Error::Invalid);
    }
    let op = |id: &str| m.operations.iter().any(|x| x.id == id);
    if m.kinds.iter().any(|x| {
        !schema(&x.schema)
            || !match &x.storage {
                ManifestPlacement::Visible { folder } => folder_name(folder),
                ManifestPlacement::Hidden { hidden } => *hidden,
            }
    }) || m.operations.iter().any(|x| {
        !x.target.as_ref().is_none_or(|t| target(t, &x.input_schema))
            || !schema(&x.input_schema)
            || !schema(&x.output_schema)
            || x.mutates == (x.idempotency == ManifestIdempotency::ReadOnly)
            || (x.mutates && x.required_capability == ManifestCapability::SpaceRead)
    }) {
        return Err(Error::Invalid);
    }
    if m.collections
        .iter()
        .any(|x| !identifier(&x.kind) || x.name.trim().is_empty() || x.name.len() > 256)
        || m.views.iter().any(|x| !identifier(&x.component))
        || m.attention.iter().any(|x| !op(&x.operation))
        || m.schedules.iter().any(|x| {
            !op(&x.operation) || x.expression.trim().is_empty() || x.expression.len() > 256
        })
        || m.webhooks.iter().any(|x| !op(&x.operation))
    {
        return Err(Error::Invalid);
    }
    if m.settings.iter().any(|x| !match x.value_type {
        ManifestSettingType::String => x.default.is_string(),
        ManifestSettingType::Boolean => x.default.is_boolean(),
        ManifestSettingType::Number => x.default.is_number(),
    }) {
        return Err(Error::Invalid);
    }
    if m.hooks.iter().any(|x| {
        !op(&x.operation)
            || x.patterns.is_empty()
            || x.patterns.len() > 128
            || x.patterns
                .iter()
                .any(|p| !identifier(p.strip_suffix(".*").unwrap_or(p)))
    }) || m.themes.iter().any(|x| {
        x.stylesheet.trim().is_empty()
            || x.stylesheet.len() > 4096
            || x.stylesheet.starts_with('/')
            || x.stylesheet.contains([':', '\\'])
            || x.stylesheet
                .split('/')
                .any(|p| p.is_empty() || p == ".." || p == ".")
            || !x.stylesheet.ends_with(".css")
    }) {
        return Err(Error::Invalid);
    }
    Ok(())
}
/// Data-only authorization row; the consumer must also enforce target ownership and live grants.
#[derive(Debug, Clone, PartialEq)]
pub struct PolicyRow {
    /// Owning implementation identifier.
    pub plugin: String,
    /// Globally registered public operation.
    pub operation: ManifestOperation,
}
/// Validated immutable declarations; insertion is atomic on collision or validation failure.
#[derive(Debug, Default)]
pub struct Registry {
    manifests: BTreeMap<String, ManifestV1>,
    kinds: BTreeSet<String>,
    operations: BTreeSet<String>,
    collections: BTreeSet<String>,
    folders: BTreeSet<String>,
}
impl Registry {
    /// Cost: O(new declarations * log registered IDs + schema compilation); allocates only new index keys.
    /// No existing manifest scan, I/O or library locks.
    /// Validate and register all contributions, rejecting kind/operation/template collisions.
    pub fn register(&mut self, manifest: ManifestV1) -> Result<(), Error> {
        validate_manifest(&manifest)?;
        let folders = visible_folders(&manifest);
        if self.manifests.contains_key(&manifest.id)
            || folders.iter().any(|f| self.folders.contains(f))
            || manifest.kinds.iter().any(|k| self.kinds.contains(&k.id))
            || manifest.operations.iter().any(|op| self.operations.contains(&op.id))
            || manifest.collections.iter().any(|c| self.collections.contains(&c.id))
        { return Err(Error::Collision); }
        self.folders.extend(folders);
        self.kinds.extend(manifest.kinds.iter().map(|k| k.id.clone()));
        self.operations.extend(manifest.operations.iter().map(|op| op.id.clone()));
        self.collections.extend(manifest.collections.iter().map(|c| c.id.clone()));
        self.manifests.insert(manifest.id.clone(), manifest);
        Ok(())
    }
    /// Cost: O(log installed plugins), borrowed output, no allocations, I/O or locks.
    /// Return the complete requested declarations for a trusted grant surface; grants are separate.
    pub fn requests(&self, plugin: &str) -> Option<&ManifestV1> {
        self.manifests.get(plugin)
    }
    /// Cost: O(total declared operations and schema bytes), allocates owned rows, no I/O or locks.
    /// Produce operation policy rows without granting capabilities or executing handlers.
    pub fn policy_rows(&self) -> Vec<PolicyRow> {
        self.manifests
            .values()
            .flat_map(|m| {
                m.operations.iter().map(|op| PolicyRow {
                    plugin: m.id.clone(),
                    operation: op.clone(),
                })
            })
            .collect()
    }
}
