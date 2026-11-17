use crate::*;
use calternal_types::Error;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashSet;

fn require(ok: bool, message: &str) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::invalid_input(message))
    }
}
/// Check a space/plugin/node identifier in O(bytes), without allocation.
///
/// # Errors
/// Returns invalid input unless there are 1..80 ASCII letters, digits or `_.:-`.
pub fn validate_id(value: &str) -> Result<(), Error> {
    require(
        !value.is_empty()
            && value.len() <= 80
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c)),
        "identifier must be 1..80 ASCII letters, digits, _, ., :, -",
    )
}
/// Check that a resource identity parses as a UUID, without storage or access checks.
///
/// # Errors
/// Returns invalid input for a malformed UUID.
pub fn validate_resource_id(value: &str) -> Result<(), Error> {
    require(
        uuid::Uuid::parse_str(value).is_ok(),
        "resource id must be a UUID",
    )
}
/// Check a title in O(bytes): nonblank, at most 256 Unicode scalars and no controls.
///
/// # Errors
/// Returns invalid input when any title constraint fails.
pub fn validate_title(value: &str) -> Result<(), Error> {
    require(
        !value.trim().is_empty()
            && value.chars().count() <= 256
            && !value.chars().any(char::is_control),
        "title must have 1..256 characters and no controls",
    )
}
/// Check the UTF-8 byte length in constant time; does not parse Markdown.
///
/// # Errors
/// Returns too large above the 1 MiB document budget.
pub fn validate_markdown(value: &str) -> Result<(), Error> {
    if value.len() > MAX_DOCUMENT_BYTES {
        Err(Error::new(ErrorCode::TooLarge, "document exceeds 1 MiB"))
    } else {
        Ok(())
    }
}
fn decode<T: DeserializeOwned>(input: &JsonObject) -> Result<T, Error> {
    serde_json::from_value(Value::Object(input.clone().into_iter().collect()))
        .map_err(|_| Error::invalid_input("input does not match operation schema"))
}
fn space(value: &str) -> Result<(), Error> {
    validate_id(value)
}
/// An omitted space (empty) selects the caller's account as owner.
fn optional_space(value: &str) -> Result<(), Error> {
    if value.is_empty() {
        Ok(())
    } else {
        space(value)
    }
}
fn resource(space_id: &str, id: &str) -> Result<(), Error> {
    if !space_id.is_empty() {
        space(space_id)?;
    }
    validate_resource_id(id)
}

/// Check correlation and operation IDs and the 2 MiB serialized-body budget. Serializes once, using O(body bytes) time and memory.
///
/// # Errors
/// Returns invalid input for malformed IDs, serialization failure or an oversized envelope.
pub fn validate_envelope(request: &OperationRequest) -> Result<(), Error> {
    require(
        !request.request_id.is_empty() && request.request_id.chars().count() <= 128,
        "request_id must have 1..128 characters",
    )?;
    require(
        !request.operation.is_empty() && request.operation.len() <= 128,
        "invalid operation identifier",
    )?;
    require(
        serde_json::to_vec(request)
            .map_err(|_| Error::invalid_input("invalid JSON"))?
            .len()
            <= MAX_BODY_BYTES,
        "request exceeds 2 MiB",
    )?;
    Ok(())
}
/// Validate an envelope and its known operation input, allocating temporary serialized/decoded values proportional to the input. Does not authorize the caller.
///
/// # Errors
/// Returns envelope/input errors, or not found for an unknown operation.
pub fn validate_request(request: &OperationRequest) -> Result<(), Error> {
    validate_envelope(request)?;
    validate_operation_input(&request.operation, &request.input)
}
/// Test membership in the fixed preview operation catalog without allocation or authority checks.
pub fn is_known_operation(operation: &str) -> bool {
    is_account_input_operation(operation)
        || matches!(
            operation,
            "spaces.list"
                | "spaces.create"
                | "spaces.members"
                | "spaces.set_role"
                | "spaces.remove_member"
                | "spaces.leave"
                | "spaces.transfer"
                | "spaces.delete"
                | "notes.list"
                | "notes.get"
                | "resolve"
                | "notes.create"
                | "notes.update"
                | "notes.history"
                | "plugins.list"
                | "plugins.enable"
                | "plugins.disable"
                | "plugins.preview"
                | "notes-insights.summarize"
                | "jobs.get"
                | "jobs.cancel"
                | "secrets.request"
                | "secrets.list"
                | "secrets.revoke"
        )
}
/// Validate the envelope and known operation inputs; leave unknown operation schemas to server discovery for forward compatibility. Cost is proportional to serialized input.
///
/// # Errors
/// Returns envelope errors or input errors for a known operation.
pub fn validate_client_request(request: &OperationRequest) -> Result<(), Error> {
    validate_envelope(request)?;
    if is_known_operation(&request.operation) {
        validate_operation_input(&request.operation, &request.input)?
    }
    Ok(())
}
/// Decode and validate an operation body, bounding bytes before JSON allocation. Cost and allocations are O(body bytes).
/// Inputs of operations in this catalog are checked here; any other operation (a
/// plugin's manifest-declared one) is left to the server, which answers an
/// unregistered name like a hidden target and checks the declared input schema.
///
/// # Errors
/// Returns too large above 2 MiB, or invalid input for a malformed envelope or a
/// catalog operation's malformed input.
pub fn parse_request(bytes: &[u8]) -> Result<OperationRequest, Error> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(Error::new(ErrorCode::TooLarge, "request exceeds 2 MiB"));
    }
    let request: OperationRequest = serde_json::from_slice(bytes)
        .map_err(|_| Error::invalid_input("malformed operation envelope"))?;
    validate_client_request(&request)?;
    Ok(request)
}
/// Decode a known operation input and enforce its value constraints; allocates a temporary JSON clone proportional to input size. Does not check access.
///
/// # Errors
/// Returns invalid input for schema/value violations, too large for oversized Markdown, or not found for unknown operations.
pub fn validate_operation_input(operation: &str, input: &JsonObject) -> Result<(), Error> {
    if is_account_input_operation(operation) {
        return validate_account_input(operation, input);
    }
    if matches!(
        operation,
        "notes.get" | "notes.history" | "notes.update" | "notes-insights.summarize"
    ) && let Some(hint) = input.get("space_id")
    {
        space(
            hint.as_str()
                .ok_or_else(|| Error::invalid_input("invalid space_id"))?,
        )?;
    }
    match operation {
        "spaces.list" => {
            let _: SpacesListInput = decode(input)?;
            Ok(())
        }
        "spaces.create" => {
            let i: SpacesCreateInput = decode(input)?;
            validate_title(&i.name)
        }
        "spaces.members" | "spaces.leave" | "spaces.delete" => {
            let i: SpaceInput = decode(input)?;
            space(&i.space_id)
        }
        "spaces.remove_member" | "spaces.transfer" => {
            let i: SpaceMemberInput = decode(input)?;
            space(&i.space_id)?;
            validate_resource_id(&i.account_id)
        }
        "spaces.set_role" => {
            let i: SpaceSetRoleInput = decode(input)?;
            space(&i.space_id)?;
            validate_resource_id(&i.account_id)?;
            require(
                i.role != SpaceRole::Owner,
                "use explicit ownership transfer",
            )
        }
        "resolve" => {
            let i: ResolveInput = decode(input)?;
            validate_id(&i.kind)?;
            resource("", &i.id)
        }
        "notes.list" => {
            let i: NotesListInput = decode(input)?;
            space(&i.space_id)
        }
        "jobs.get" | "jobs.cancel" => {
            let i: JobInput = decode(input)?;
            resource("", &i.job_id)
        }
        "notes.get" | "notes-insights.summarize" => {
            let i: NotesGetInput = decode(input)?;
            resource(&i.space_id, &i.id)
        }
        "notes.create" => {
            let i: NotesCreateInput = decode(input)?;
            space(&i.space_id)?;
            validate_title(&i.title)?;
            validate_markdown(&i.markdown)
        }
        "notes.update" => {
            let i: NotesUpdateInput = decode(input)?;
            resource(&i.space_id, &i.id)?;
            require(
                !i.base_revision.is_empty() && i.base_revision.len() <= 256,
                "invalid base_revision",
            )?;
            if let Some(title) = &i.title {
                validate_title(title)?
            }
            validate_markdown(&i.markdown)
        }
        "notes.history" => {
            let i: NotesHistoryInput = decode(input)?;
            resource(&i.space_id, &i.id)?;
            require(
                i.limit.is_none_or(|n| (1..=100).contains(&n)),
                "history limit must be 1..100",
            )?;
            require(
                i.cursor
                    .as_ref()
                    .is_none_or(|s| !s.is_empty() && s.len() <= 256),
                "invalid history cursor",
            )
        }
        "plugins.list" => {
            let i: PluginsListInput = decode(input)?;
            space(&i.space_id)
        }
        "plugins.enable" | "plugins.disable" | "plugins.preview" => {
            let i: PluginInput = decode(input)?;
            space(&i.space_id)?;
            validate_id(&i.plugin_id)
        }
        // Names, labels and values get their semantic checks in the secrets store.
        "secrets.request" => {
            let i: SecretsRequestInput = decode(input)?;
            optional_space(&i.space_id)
        }
        "secrets.list" => {
            let i: SecretsListInput = decode(input)?;
            optional_space(&i.space_id)
        }
        "secrets.revoke" => {
            let i: SecretsRevokeInput = decode(input)?;
            optional_space(&i.space_id)?;
            require(
                i.name.is_empty() != i.request_id.is_empty(),
                "give exactly one of name or request_id",
            )
        }
        _ => Err(Error::new(ErrorCode::NotFound, "unknown operation")),
    }
}

fn string_bytes(value: &Value) -> usize {
    match value {
        Value::String(s) => s.len(),
        Value::Array(a) => a.iter().map(string_bytes).sum(),
        Value::Object(o) => o.values().map(string_bytes).sum(),
        _ => 0,
    }
}
/// Validate portable tree version, unique IDs and budgets: 64 KiB JSON, 32 KiB string values, 8 KiB text, depth 8 and 128 nodes. Serializes the tree and allocates O(tree bytes) temporary storage.
///
/// # Errors
/// Returns invalid input for any tree or budget violation; does not grant operation authority.
pub fn validate_genui(view: &GenUiView) -> Result<(), Error> {
    require(
        view.genui_version == GENUI_VERSION,
        "unsupported GenUI version",
    )?;
    let bytes = serde_json::to_vec(view).map_err(|_| Error::invalid_input("invalid GenUI"))?;
    require(bytes.len() <= 64 * 1024, "GenUI exceeds 64 KiB")?;
    require(
        string_bytes(&serde_json::to_value(view).unwrap()) <= 32 * 1024,
        "GenUI string budget exceeds 32 KiB",
    )?;
    fn visit(node: &GenUiNode, depth: usize, ids: &mut HashSet<String>) -> Result<(), Error> {
        require(depth <= 8, "GenUI depth exceeds 8")?;
        validate_id(node.id())?;
        require(ids.insert(node.id().into()), "duplicate GenUI id")?;
        require(ids.len() <= 128, "GenUI exceeds 128 nodes")?;
        let label = match node {
            GenUiNode::Button { label, .. }
            | GenUiNode::Stat { label, .. }
            | GenUiNode::Resource { label, .. } => Some(label),
            _ => None,
        };
        if let Some(label) = label {
            require(
                !label.is_empty() && label.chars().count() <= 256,
                "label must have 1..256 characters",
            )?;
        }
        match node {
            GenUiNode::Stack { children, .. } => {
                for child in children {
                    visit(child, depth + 1, ids)?
                }
            }
            GenUiNode::Text { text, .. } => require(text.len() <= 8 * 1024, "text exceeds 8 KiB")?,
            GenUiNode::Button { action, .. } => {
                require(
                    !action.operation.is_empty() && action.operation.len() <= 128,
                    "invalid action operation",
                )?;
            }
            GenUiNode::Resource {
                resource_id,
                space_id,
                ..
            }
            | GenUiNode::DocumentEditor {
                resource_id,
                space_id,
                ..
            } => resource(space_id, resource_id)?,
            GenUiNode::Stat { .. } => {}
        }
        Ok(())
    }
    visit(&view.root, 1, &mut HashSet::new())
}
/// Validate the tree and every action against declared read-only operations. Traversal costs O(nodes × declared operations), plus tree validation.
///
/// # Errors
/// Returns tree/input errors or invalid input for undeclared or mutating actions.
pub fn validate_extension_view(view: &GenUiView, manifest: &PluginManifest) -> Result<(), Error> {
    validate_genui(view)?;
    fn visit(node: &GenUiNode, manifest: &PluginManifest) -> Result<(), Error> {
        match node {
            GenUiNode::Stack { children, .. } => {
                for child in children {
                    visit(child, manifest)?
                }
            }
            GenUiNode::Button { action, .. } => {
                require(
                    manifest
                        .operations
                        .iter()
                        .any(|op| op.id == action.operation),
                    "extension action is not declared",
                )?;
                require(
                    !matches!(
                        action.operation.as_str(),
                        "notes.create" | "notes.update" | "plugins.enable" | "plugins.disable"
                    ),
                    "extension action must not invoke a mutating host operation",
                )?;
                validate_operation_input(&action.operation, &action.input)?;
            }
            _ => {}
        }
        Ok(())
    }
    visit(&view.root, manifest)
}
/// Check plugin IDs, version, API compatibility, unique operation/view IDs and all contributed views. Cost includes each view validation and declared-operation lookup. Does not load code or issue grants.
///
/// # Errors
/// Returns invalid input or propagated view/input errors for invalid declarations.
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), Error> {
    validate_id(&manifest.id)?;
    require(
        manifest.api_version == PROTOCOL_VERSION,
        "unsupported plugin API",
    )?;
    require(
        !manifest.version.is_empty() && !manifest.name.is_empty(),
        "plugin version/name required",
    )?;
    let mut ids = HashSet::new();
    for op in &manifest.operations {
        validate_id(&op.id)?;
        require(ids.insert(&op.id), "duplicate plugin operation")?;
    }
    for dep in &manifest.dependencies {
        validate_id(&dep.id)?;
        require(!dep.version.is_empty(), "dependency version required")?;
    }
    let mut views = HashSet::new();
    for view in &manifest.views {
        validate_id(&view.id)?;
        require(views.insert(&view.id), "duplicate plugin view")?;
        validate_extension_view(&view.view, manifest)?;
    }
    Ok(())
}
/// Object keys sorted recursively, independent of serde_json map-order features.
/// No numeric/string coercion: differing typed values remain different inputs.
/// Allocates O(input bytes) temporary values; each object sorts its keys in O(keys log keys).
pub fn canonical_request_input(input: &JsonObject) -> String {
    fn sorted(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let sorted: std::collections::BTreeMap<_, _> =
                    map.iter().map(|(k, v)| (k.clone(), sorted(v))).collect();
                Value::Object(sorted.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.iter().map(sorted).collect()),
            _ => value.clone(),
        }
    }
    serde_json::to_string(&sorted(
        &serde_json::to_value(input).expect("JSON values serialize"),
    ))
    .expect("JSON values serialize")
}

/// Retain this identity after an unknown transport outcome; never mutate input on retry.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingRequest {
    request: OperationRequest,
}
impl PendingRequest {
    /// Create a validated pending request with a fresh UUID identity; retain it unchanged after an uncertain result. Validation costs O(input bytes).
    ///
    /// # Errors
    /// Returns envelope or known-operation validation failures.
    pub fn new(operation: impl Into<String>, input: JsonObject) -> Result<Self, Error> {
        Self::with_id(uuid::Uuid::new_v4().to_string(), operation, input)
    }
    /// Create a validated pending request using an existing retry identity; callers must preserve the operation and input on retries. Cost is O(input bytes).
    ///
    /// # Errors
    /// Returns envelope or known-operation validation failures.
    pub fn with_id(
        id: impl Into<String>,
        operation: impl Into<String>,
        input: JsonObject,
    ) -> Result<Self, Error> {
        let request = OperationRequest {
            request_id: id.into(),
            operation: operation.into(),
            input,
        };
        validate_client_request(&request)?;
        Ok(Self { request })
    }
    /// Borrow the validated immutable request without allocation, preserving its retry identity.
    pub fn request(&self) -> &OperationRequest {
        &self.request
    }
    /// Check a pending `notes.update` against the actual opaque revision; other operations pass unchanged. Decoding clones O(input bytes). This is advisory; the host must check atomically again.
    ///
    /// # Errors
    /// Returns conflict on mismatch or invalid input if update decoding fails.
    pub fn check_base_revision(&self, actual: &str) -> Result<(), Error> {
        if self.request.operation == "notes.update" {
            let input: NotesUpdateInput = decode(&self.request.input)?;
            if input.base_revision != actual {
                return Err(Error::new(ErrorCode::Conflict, "stale base revision"));
            }
        }
        Ok(())
    }
}

/// Build discovery metadata and JSON Schemas for all preview operations including the stored space directory. Allocates fresh schema trees on each call; callers may cache the result outside the pure core.
pub fn operation_catalog() -> Vec<OperationDescription> {
    fn entry<I: schemars::JsonSchema, O: schemars::JsonSchema>(
        id: &str,
        description: &str,
        mutates: bool,
    ) -> OperationDescription {
        let object = |value: Value| serde_json::from_value(value).expect("schema is object");
        OperationDescription {
            id: id.into(),
            description: description.into(),
            mutates,
            input_schema: object(serde_json::to_value(schemars::schema_for!(I)).unwrap()),
            output_schema: object(serde_json::to_value(schemars::schema_for!(O)).unwrap()),
        }
    }
    let mut catalog = vec![
        entry::<SpacesListInput, SpacesListResult>(
            "spaces.list",
            "List current account memberships",
            false,
        ),
        entry::<SpacesCreateInput, SpaceResult>("spaces.create", "Create an owned space", true),
        entry::<SpaceInput, SpaceMembersResult>("spaces.members", "List current members", false),
        entry::<SpaceSetRoleInput, SpaceChangedResult>(
            "spaces.set_role",
            "Change an existing member role",
            true,
        ),
        entry::<SpaceMemberInput, SpaceChangedResult>(
            "spaces.remove_member",
            "Remove a member",
            true,
        ),
        entry::<SpaceInput, SpaceChangedResult>(
            "spaces.leave",
            "Leave after transferring ownership if necessary",
            true,
        ),
        entry::<SpaceMemberInput, SpaceChangedResult>(
            "spaces.transfer",
            "Transfer ownership to an existing member",
            true,
        ),
        entry::<SpaceInput, SpaceChangedResult>(
            "spaces.delete",
            "Delete directory records; content cleanup is a later step",
            true,
        ),
        entry::<NotesListInput, NotesListResult>("notes.list", "List up to 100 notes", false),
        entry::<ResolveInput, ResolveResult>(
            "resolve",
            "Resolve an item address with current access",
            false,
        ),
        entry::<NotesGetInput, NoteResult>("notes.get", "Read a note", false),
        entry::<NotesCreateInput, NoteResult>("notes.create", "Create a Markdown note", true),
        entry::<NotesUpdateInput, NoteResult>(
            "notes.update",
            "Update a note against an opaque base revision",
            true,
        ),
        entry::<NotesHistoryInput, NotesHistoryResult>(
            "notes.history",
            "Read bounded note history",
            false,
        ),
        entry::<PluginsListInput, PluginsListResult>(
            "plugins.list",
            "List registered plugins",
            false,
        ),
        entry::<PluginInput, PluginResult>("plugins.enable", "Enable a trusted plugin", true),
        entry::<PluginInput, PluginResult>("plugins.disable", "Disable a trusted plugin", true),
        entry::<PluginInput, PluginPreviewResult>(
            "plugins.preview",
            "Preview a trusted plugin using fixture data",
            false,
        ),
        entry::<NotesGetInput, SummarizeResult>(
            "notes-insights.summarize",
            "Compute word, character and line statistics",
            false,
        ),
        entry::<JobInput, JobResult>("jobs.get", "Read a plugin job's durable record", false),
        entry::<JobInput, JobResult>(
            "jobs.cancel",
            "Cancel a plugin job and its active child jobs",
            true,
        ),
        entry::<SecretsRequestInput, SecretsRequestResult>(
            "secrets.request",
            "Ask for a secret; a person supplies the value through the trusted prompt",
            true,
        ),
        entry::<SecretsListInput, SecretsListResult>(
            "secrets.list",
            "List secret metadata and pending requests, never values",
            false,
        ),
        entry::<SecretsRevokeInput, SecretsRevokeResult>(
            "secrets.revoke",
            "Destroy a secret or withdraw a pending request",
            true,
        ),
    ];
    catalog.extend(account_input_catalog());
    catalog
}
