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
fn selector(value: &str, limit: usize) -> bool {
    value.len() <= limit
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-:@/".contains(&b))
}

pub(crate) fn unique<'a>(ids: impl Iterator<Item = &'a str>) -> bool {
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
            ManifestPlacement::Visible { folder }
            | ManifestPlacement::VisibleTree { folder, .. } => Some(fold(folder)),
            _ => None,
        })
        .collect()
}
fn schema(value: &serde_json::Value) -> bool {
    (value.is_object() || value.is_boolean()) && jsonschema::validator_for(value).is_ok()
}
/// Claimed text is Markdown (`md` or absent); any other kind names its extension.
/// Cost: O(extension bytes), no allocation.
fn extension(claimed_text: bool, extension: Option<&str>) -> bool {
    match extension {
        None => claimed_text,
        Some(e) => {
            !e.is_empty()
                && e.len() <= 16
                && e.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
                && (!claimed_text || e == "md")
        }
    }
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
    validate_manifest_detailed(m).map_err(|problem| {
        if problem.starts_with("/: manifest exceeds") {
            Error::TooLarge
        } else {
            Error::Invalid
        }
    })
}

/// Validate the same manifest rules as registration, with a JSON Pointer and
/// a plain-language rule on failure. Diagnostics contain declaration paths,
/// never worker input or user data. Cost: bounded validation and schema compilation.
pub fn validate_manifest_detailed(m: &ManifestV1) -> Result<(), String> {
    crate::bounded::check(m, MAX_MANIFEST_BYTES)
        .map_err(|_| "/: manifest exceeds 256 KiB".to_owned())?;
    let invalid = |path: &str, rule: &str| Err(format!("{path}: {rule}"));
    for (path, valid, rule) in [
        (
            "/api_version",
            m.api_version == 1,
            "expected manifest API version 1",
        ),
        (
            "/id",
            identifier(&m.id),
            "expected a lowercase package identifier",
        ),
        (
            "/icon",
            identifier(&m.icon),
            "expected a lowercase icon identifier",
        ),
        (
            "/name",
            !m.name.trim().is_empty() && m.name.len() <= 256,
            "expected a nonempty name of at most 256 bytes",
        ),
        (
            "/version",
            !m.version.trim().is_empty() && m.version.len() <= 80,
            "expected a nonempty version of at most 80 bytes",
        ),
    ] {
        if !valid {
            return invalid(path, rule);
        }
    }
    let groups = [
        (
            "/kinds",
            m.kinds.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
        ),
        (
            "/operations",
            m.operations.iter().map(|x| x.id.as_str()).collect(),
        ),
        (
            "/collections",
            m.collections.iter().map(|x| x.id.as_str()).collect(),
        ),
        ("/views", m.views.iter().map(|x| x.id.as_str()).collect()),
        (
            "/attention",
            m.attention.iter().map(|x| x.id.as_str()).collect(),
        ),
        (
            "/settings",
            m.settings.iter().map(|x| x.id.as_str()).collect(),
        ),
        (
            "/schedules",
            m.schedules.iter().map(|x| x.id.as_str()).collect(),
        ),
        (
            "/webhooks",
            m.webhooks.iter().map(|x| x.id.as_str()).collect(),
        ),
        ("/hooks", m.hooks.iter().map(|x| x.id.as_str()).collect()),
        ("/themes", m.themes.iter().map(|x| x.id.as_str()).collect()),
        ("/secrets", m.secrets.iter().map(String::as_str).collect()),
    ];
    for (path, group) in groups {
        if group.len() > 128 || !unique(group.iter().copied()) {
            return invalid(path, "expected at most 128 entries with unique valid IDs");
        }
    }
    if m.host_calls.iter().collect::<BTreeSet<_>>().len() != m.host_calls.len() {
        return invalid("/host_calls", "host calls must be unique");
    }
    if m.provider.as_ref().is_some_and(|p| {
        p.protocol_version != 1
            || !identifier(&p.executable)
            || p.executable.contains('.')
            || !p.models.iter().any(|model| model.id == p.default_model)
            || p.models.len() > 128
            || !unique(p.models.iter().map(|model| model.id.as_str()))
            || p.models.iter().any(|model| {
                !selector(&model.id, 128)
                    || model.name.trim().is_empty()
                    || model.name.len() > 128
                    || model.name.chars().any(char::is_control)
                    || model.reasoning_levels.len() > 16
                    || !unique(model.reasoning_levels.iter().map(String::as_str))
                    || model
                        .reasoning_levels
                        .iter()
                        .any(|level| !selector(level, 32))
            })
    }) {
        return invalid(
            "/provider",
            "check protocol version, executable name, default model and model declarations",
        );
    }
    if let Some(menu) = &m.palette {
        let label =
            |s: &str| !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control);
        let address =
            |s: &str| label(s) && s.starts_with('/') && !s.starts_with("//") && !s.contains('\\');
        if menu.commands.len() > 128
            || menu.settings_sections.len() > 128
            || !unique(
                menu.commands.iter().map(|c| c.id.as_str()).chain(
                    menu.settings_sections
                        .iter()
                        .map(|section| section.id.as_str()),
                ),
            )
            || menu.commands.iter().any(|c| {
                !label(&c.label)
                    || !label(&c.group)
                    || c.words.len() > 32
                    || c.words.iter().any(|word| !label(word))
                    || !match (&c.go, &c.op) {
                        (Some(go), None) => address(go),
                        (None, Some(op)) => identifier(op),
                        _ => false,
                    }
            })
            || menu
                .settings_sections
                .iter()
                .any(|s| !label(&s.label) || !address(&s.go))
        {
            return invalid(
                "/palette",
                "check command and section IDs, labels, routes and command targets",
            );
        }
    }
    crate::slots::validate(m)?;
    let op = |id: &str| m.operations.iter().any(|x| x.id == id);
    for (index, x) in m.kinds.iter().enumerate() {
        if (x.searchable == Some(true) || x.live_editing()) && !x.claimed_text
            || x.schema
                .get(calternal_core::ManifestKind::LIVE_EDITING)
                .is_some_and(|value| !value.is_boolean())
            || !extension(x.claimed_text, x.extension.as_deref())
            || !schema(&x.schema)
            || !match &x.storage {
                ManifestPlacement::Visible { folder } => folder_name(folder),
                ManifestPlacement::VisibleTree {
                    folder,
                    subdirectories,
                } => {
                    folder_name(folder)
                        && !subdirectories.is_empty()
                        && subdirectories.len() <= 8
                        && subdirectories.iter().all(|s| folder_name(s))
                        && !calternal_types::is_reserved_space_path(
                            &std::iter::once(folder.as_str())
                                .chain(subdirectories.iter().map(String::as_str))
                                .collect::<Vec<_>>()
                                .join("/"),
                        )
                }
                ManifestPlacement::Hidden { hidden } => *hidden,
            }
        {
            return invalid(
                &format!("/kinds/{index}"),
                "check the JSON schema, file extension, text claim and storage path",
            );
        }
    }
    for (index, x) in m.operations.iter().enumerate() {
        for (field, valid, rule) in [
            (
                "target",
                x.target.as_ref().is_none_or(|t| target(t, &x.input_schema)),
                "the item target must refer to a required string field in input_schema",
            ),
            (
                "input_schema",
                schema(&x.input_schema),
                "expected a valid JSON Schema",
            ),
            (
                "output_schema",
                schema(&x.output_schema),
                "expected a valid JSON Schema",
            ),
            (
                "risk",
                x.mutates != (x.risk == OperationRisk::Read),
                "read operations must use read risk and mutations must declare a write risk",
            ),
            (
                "idempotency",
                x.mutates != (x.idempotency == ManifestIdempotency::ReadOnly),
                "use read_only for reads and request_id or never for mutations",
            ),
            (
                "required_capability",
                !x.mutates || x.required_capability != ManifestCapability::SpaceRead,
                "a mutation requires a write capability",
            ),
        ] {
            if !valid {
                return invalid(&format!("/operations/{index}/{field}"), rule);
            }
        }
    }
    for (path, valid, rule) in [
        (
            "/collections",
            m.collections
                .iter()
                .all(|x| identifier(&x.kind) && !x.name.trim().is_empty() && x.name.len() <= 256),
            "each collection requires a valid kind ID and a nonempty name of at most 256 bytes",
        ),
        (
            "/views",
            m.views.iter().all(|x| identifier(&x.component)),
            "each view requires a valid component ID",
        ),
        (
            "/attention",
            m.attention.iter().all(|x| op(&x.operation)),
            "each entry must point to a declared operation",
        ),
        (
            "/schedules",
            m.schedules.iter().all(|x| {
                op(&x.operation) && !x.expression.trim().is_empty() && x.expression.len() <= 256
            }),
            "each schedule requires a declared operation and a nonempty expression of at most 256 bytes",
        ),
        (
            "/webhooks",
            m.webhooks.iter().all(|x| op(&x.operation)),
            "each webhook must point to a declared operation",
        ),
        (
            "/settings",
            m.settings.iter().all(|x| x.accepts(&x.default)),
            "each default must match its declared value_type",
        ),
        (
            "/hooks",
            m.hooks.iter().all(|x| {
                op(&x.operation)
                    && !x.patterns.is_empty()
                    && x.patterns.len() <= 128
                    && x.patterns
                        .iter()
                        .all(|p| identifier(p.strip_suffix(".*").unwrap_or(p)))
            }),
            "each hook requires a declared operation and 1 to 128 valid ID patterns",
        ),
        (
            "/themes",
            m.themes.iter().all(|x| {
                !x.stylesheet.trim().is_empty()
                    && x.stylesheet.len() <= 4096
                    && !x.stylesheet.starts_with('/')
                    && !x.stylesheet.contains([':', '\\'])
                    && x.stylesheet
                        .split('/')
                        .all(|p| !p.is_empty() && p != ".." && p != ".")
                    && x.stylesheet.ends_with(".css")
            }),
            "each stylesheet must use a safe relative CSS path",
        ),
    ] {
        if !valid {
            return invalid(path, rule);
        }
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
    palette_commands: BTreeSet<String>,
}
impl Registry {
    /// Cost: O(new declarations * log registered IDs + schema compilation); allocates only new index keys.
    /// No existing manifest scan, I/O or library locks.
    /// Validate and register all contributions, rejecting kind/operation/template collisions.
    pub fn register(&mut self, manifest: ManifestV1) -> Result<(), Error> {
        validate_manifest(&manifest)?;
        let folders = visible_folders(&manifest);
        let commands = manifest.palette.iter().flat_map(|menu| {
            menu.commands
                .iter()
                .map(|c| &c.id)
                .chain(menu.settings_sections.iter().map(|section| &section.id))
        });
        if self.manifests.contains_key(&manifest.id)
            || folders.iter().any(|f| self.folders.contains(f))
            || commands
                .clone()
                .any(|id| self.palette_commands.contains(id))
            || manifest.kinds.iter().any(|k| self.kinds.contains(&k.id))
            || manifest
                .operations
                .iter()
                .any(|op| self.operations.contains(&op.id))
            || manifest
                .collections
                .iter()
                .any(|c| self.collections.contains(&c.id))
        {
            return Err(Error::Collision);
        }
        self.folders.extend(folders);
        self.palette_commands.extend(commands.cloned());
        self.kinds
            .extend(manifest.kinds.iter().map(|k| k.id.clone()));
        self.operations
            .extend(manifest.operations.iter().map(|op| op.id.clone()));
        self.collections
            .extend(manifest.collections.iter().map(|c| c.id.clone()));
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
