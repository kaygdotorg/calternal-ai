use crate::*;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ValidationError {
    pub code: ErrorCode,
    pub message: String,
}
impl ValidationError {
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidInput,
            message: message.into(),
        }
    }
}
fn require(ok: bool, message: &str) -> Result<(), ValidationError> {
    if ok {
        Ok(())
    } else {
        Err(ValidationError::invalid(message))
    }
}
pub fn validate_id(value: &str) -> Result<(), ValidationError> {
    require(
        !value.is_empty()
            && value.len() <= 80
            && value
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.:-".contains(&c)),
        "identifier must be 1..80 ASCII letters, digits, _, ., :, -",
    )
}
pub fn validate_resource_id(value: &str) -> Result<(), ValidationError> {
    require(
        uuid::Uuid::parse_str(value).is_ok(),
        "resource id must be a UUID",
    )
}
pub fn validate_title(value: &str) -> Result<(), ValidationError> {
    require(
        !value.trim().is_empty()
            && value.chars().count() <= 256
            && !value.chars().any(char::is_control),
        "title must have 1..256 characters and no controls",
    )
}
pub fn validate_markdown(value: &str) -> Result<(), ValidationError> {
    if value.len() > MAX_DOCUMENT_BYTES {
        Err(ValidationError {
            code: ErrorCode::TooLarge,
            message: "document exceeds 1 MiB".into(),
        })
    } else {
        Ok(())
    }
}
fn decode<T: DeserializeOwned>(input: &JsonObject) -> Result<T, ValidationError> {
    serde_json::from_value(Value::Object(input.clone().into_iter().collect()))
        .map_err(|_| ValidationError::invalid("input does not match operation schema"))
}
fn space(value: &str) -> Result<(), ValidationError> {
    validate_id(value)
}
fn resource(space_id: &str, id: &str) -> Result<(), ValidationError> {
    space(space_id)?;
    validate_resource_id(id)
}

pub fn validate_envelope(request: &OperationRequest) -> Result<(), ValidationError> {
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
            .map_err(|_| ValidationError::invalid("invalid JSON"))?
            .len()
            <= MAX_BODY_BYTES,
        "request exceeds 2 MiB",
    )?;
    Ok(())
}
pub fn validate_request(request: &OperationRequest) -> Result<(), ValidationError> {
    validate_envelope(request)?;
    validate_operation_input(&request.operation, &request.input)
}
pub fn is_known_operation(operation: &str) -> bool {
    matches!(
        operation,
        "notes.list"
            | "notes.get"
            | "notes.create"
            | "notes.update"
            | "notes.history"
            | "plugins.list"
            | "plugins.enable"
            | "plugins.disable"
            | "plugins.preview"
            | "notes-insights.summarize"
    )
}
pub fn validate_client_request(request: &OperationRequest) -> Result<(), ValidationError> {
    validate_envelope(request)?;
    if is_known_operation(&request.operation) {
        validate_operation_input(&request.operation, &request.input)?
    }
    Ok(())
}
pub fn parse_request(bytes: &[u8]) -> Result<OperationRequest, ValidationError> {
    if bytes.len() > MAX_BODY_BYTES {
        return Err(ValidationError {
            code: ErrorCode::TooLarge,
            message: "request exceeds 2 MiB".into(),
        });
    }
    let request: OperationRequest = serde_json::from_slice(bytes)
        .map_err(|_| ValidationError::invalid("malformed operation envelope"))?;
    validate_request(&request)?;
    Ok(request)
}
pub fn validate_operation_input(
    operation: &str,
    input: &JsonObject,
) -> Result<(), ValidationError> {
    match operation {
        "notes.list" => {
            let i: NotesListInput = decode(input)?;
            space(&i.space_id)
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
        _ => Err(ValidationError {
            code: ErrorCode::NotFound,
            message: "unknown operation".into(),
        }),
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
pub fn validate_genui(view: &GenUiView) -> Result<(), ValidationError> {
    require(
        view.genui_version == GENUI_VERSION,
        "unsupported GenUI version",
    )?;
    let bytes = serde_json::to_vec(view).map_err(|_| ValidationError::invalid("invalid GenUI"))?;
    require(bytes.len() <= 64 * 1024, "GenUI exceeds 64 KiB")?;
    require(
        string_bytes(&serde_json::to_value(view).unwrap()) <= 32 * 1024,
        "GenUI string budget exceeds 32 KiB",
    )?;
    fn visit(
        node: &GenUiNode,
        depth: usize,
        ids: &mut HashSet<String>,
    ) -> Result<(), ValidationError> {
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
/// Extension views may only refer to operations declared read-only by that manifest.
pub fn validate_extension_view(
    view: &GenUiView,
    manifest: &PluginManifest,
) -> Result<(), ValidationError> {
    validate_genui(view)?;
    fn visit(node: &GenUiNode, manifest: &PluginManifest) -> Result<(), ValidationError> {
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
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), ValidationError> {
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
pub fn validate_worker_request(request: &WorkerRequest) -> Result<(), ValidationError> {
    require(
        request.protocol_version == PROTOCOL_VERSION,
        "unsupported worker protocol",
    )?;
    validate_id(&request.invocation_id)?;
    resource(&request.resource.space_id, &request.resource.id)?;
    validate_title(&request.resource.title)?;
    validate_markdown(&request.resource.markdown)?;
    require(
        !request.resource.revision.is_empty(),
        "missing resource revision",
    )
}
pub fn parse_worker_response(
    bytes: &[u8],
    invocation_id: &str,
) -> Result<WorkerResponse, ValidationError> {
    require(
        bytes.len() <= MAX_WORKER_OUTPUT_BYTES,
        "worker output exceeds 256 KiB",
    )?;
    let response: WorkerResponse = serde_json::from_slice(bytes)
        .map_err(|_| ValidationError::invalid("malformed worker output"))?;
    let (version, id) = match &response {
        WorkerResponse::Success(r) => {
            validate_genui(&r.data.view)?;
            (r.protocol_version, &r.invocation_id)
        }
        WorkerResponse::Failure(r) => (r.protocol_version, &r.invocation_id),
    };
    require(
        version == PROTOCOL_VERSION && id == invocation_id,
        "worker protocol/invocation mismatch",
    )?;
    Ok(response)
}

/// Object keys sorted recursively, independent of serde_json map-order features.
/// No numeric/string coercion: differing typed values remain different inputs.
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
    pub fn new(operation: impl Into<String>, input: JsonObject) -> Result<Self, ValidationError> {
        Self::with_id(uuid::Uuid::new_v4().to_string(), operation, input)
    }
    pub fn with_id(
        id: impl Into<String>,
        operation: impl Into<String>,
        input: JsonObject,
    ) -> Result<Self, ValidationError> {
        let request = OperationRequest {
            request_id: id.into(),
            operation: operation.into(),
            input,
        };
        validate_client_request(&request)?;
        Ok(Self { request })
    }
    pub fn request(&self) -> &OperationRequest {
        &self.request
    }
    pub fn check_base_revision(&self, actual: &str) -> Result<(), ValidationError> {
        if self.request.operation == "notes.update" {
            let input: NotesUpdateInput = decode(&self.request.input)?;
            if input.base_revision != actual {
                return Err(ValidationError {
                    code: ErrorCode::Conflict,
                    message: "stale base revision".into(),
                });
            }
        }
        Ok(())
    }
}

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
    vec![
        entry::<NotesListInput, NotesListResult>("notes.list", "List up to 100 notes", false),
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
    ]
}
