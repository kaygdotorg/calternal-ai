use crate::*;
use calternal_types::Error;
use serde::de::DeserializeOwned;
use serde_json::Value;

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
/// Maximum byte length of a wire cursor, including its signature and encoding.
pub const MAX_PAGE_CURSOR_BYTES: usize = 512 * 1024;

/// Bound a wire cursor before decoding. The limit accommodates a base64-encoded
/// tag key from the 256 KiB content metadata budget. Cost is constant time.
pub fn validate_page_cursor(cursor: &str) -> Result<(), Error> {
    require(
        !cursor.is_empty() && cursor.len() <= MAX_PAGE_CURSOR_BYTES,
        "invalid page cursor",
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
pub(crate) fn decode<T: DeserializeOwned>(input: &JsonObject) -> Result<T, Error> {
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
    crate::AGENT_OPERATIONS
        .iter()
        .any(|(id, _, _)| *id == operation)
        || crate::PeopleOperation::from_id(operation).is_some()
        || crate::is_appearance_operation(operation)
        || is_account_input_operation(operation)
        || crate::note_operation(operation).is_some()
        || crate::SearchOperation::from_id(operation).is_some()
        || request_operation(operation).is_some()
        || matches!(
            operation,
            WALLPAPERS_LIST
                | ONBOARDING_STATE
                | ONBOARDING_FINISH
                | "spaces.list"
                | "spaces.create"
                | "spaces.leave"
                | "spaces.delete"
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
    if matches!(
        operation,
        crate::MACHINES_ONBOARDING | crate::MACHINES_INSTALL
    ) {
        let _: crate::MachineOnboardingInput = decode(input)?;
        return Ok(());
    }
    if operation == crate::MACHINES_VERIFY {
        let i: crate::MachineVerifyInput = decode(input)?;
        space(&i.space_id)?;
        return require(
            uuid::Uuid::parse_str(&i.machine_id).is_ok(),
            "machine_id must be a UUID",
        );
    }
    if operation == crate::AGENT_READINESS {
        let _: crate::ReadinessInput = decode(input)?;
        return Ok(());
    }
    if crate::AGENT_OPERATIONS
        .iter()
        .any(|(id, _, _)| *id == operation)
    {
        let _: crate::AgentInput = decode(input)?;
        return Ok(());
    }
    if let Some(people) = crate::PeopleOperation::from_id(operation) {
        return people.validate(input);
    }
    if let Some(op) = crate::SearchOperation::from_id(operation) {
        return op.validate(input);
    }
    if let Some(op) = crate::note_operation(operation) {
        return op.validate(input);
    }
    if let Some(row) = request_operation(operation) {
        return row.validate(input);
    }
    if crate::is_appearance_operation(operation) {
        return crate::appearance::validate(operation, input);
    }
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
        ONBOARDING_STATE | ONBOARDING_FINISH => {
            let _: OnboardingInput = decode(input)?;
            Ok(())
        }
        WALLPAPERS_LIST => {
            let _: WallpapersListInput = decode(input)?;
            Ok(())
        }
        "spaces.list" => {
            let _: SpacesListInput = decode(input)?;
            Ok(())
        }
        "spaces.create" => {
            let i: SpacesCreateInput = decode(input)?;
            validate_title(&i.name)
        }
        "spaces.leave" | "spaces.delete" => {
            let i: SpaceInput = decode(input)?;
            space(&i.space_id)
        }
        "resolve" => {
            let i: ResolveInput = decode(input)?;
            validate_id(&i.kind)?;
            resource("", &i.id)
        }
        "jobs.get" | "jobs.cancel" => {
            if let Some(field) = input.keys().find(|key| key.as_str() != "job_id") {
                return Err(Error::input_field(
                    &format!("/{field}"),
                    "field is not accepted here, send only /job_id as a UUID string",
                ));
            }
            let job = input
                .get("job_id")
                .and_then(Value::as_str)
                .ok_or_else(|| Error::input_field("/job_id", "expected a job UUID string"))?;
            validate_resource_id(job)
                .map_err(|_| Error::input_field("/job_id", "expected a job UUID string"))
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

/// Checks a generated-UI view's version and size. The host additionally compiles
/// the source (`calternal-genui`) and refuses any diagnostic.
///
/// # Errors
/// Invalid input for another version; too large above 64 KiB of source.
/// Cost: O(1).
pub fn validate_genui(view: &GenUiView) -> Result<(), Error> {
    require(
        view.genui_version == GENUI_VERSION,
        "unsupported GenUI version",
    )?;
    if view.source.len() > 64 * 1024 {
        return Err(Error::new(
            ErrorCode::TooLarge,
            "GenUI source exceeds 64 KiB",
        ));
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

pub(crate) fn entry<I: schemars::JsonSchema, O: schemars::JsonSchema>(
    id: &str,
    description: &str,
    mutates: bool,
) -> OperationDescription {
    let object = |value: Value| serde_json::from_value(value).expect("schema is object");
    OperationDescription {
        risk: crate::builtin_operation_risk(id, mutates),
        required_capability: None,
        target: None,
        id: id.into(),
        description: description.into(),
        mutates,
        input_schema: object(serde_json::to_value(schemars::schema_for!(I)).unwrap()),
        output_schema: object(serde_json::to_value(schemars::schema_for!(O)).unwrap()),
    }
}

/// Build discovery metadata and JSON Schemas for all preview operations including the stored space directory. Allocates fresh schema trees on each call; callers may cache the result outside the pure core.
pub fn operation_catalog() -> Vec<OperationDescription> {
    let mut catalog = vec![
        entry::<OnboardingInput, OnboardingState>(
            ONBOARDING_STATE,
            "Read your onboarding status",
            false,
        ),
        entry::<OnboardingInput, OnboardingState>(
            ONBOARDING_FINISH,
            "Mark your onboarding finished",
            true,
        ),
        entry::<WallpapersListInput, WallpapersListResult>(
            WALLPAPERS_LIST,
            "List built-in wallpapers with Unsplash attribution",
            false,
        ),
        entry::<SpacesListInput, SpacesListResult>(
            "spaces.list",
            "List current account memberships",
            false,
        ),
        entry::<SpacesCreateInput, SpaceResult>("spaces.create", "Create an owned space", true),
        entry::<SpaceInput, SpaceChangedResult>(
            "spaces.leave",
            "Leave after transferring ownership if necessary",
            true,
        ),
        entry::<SpaceInput, SpaceChangedResult>(
            "spaces.delete",
            "Delete directory records; content cleanup is a later step",
            true,
        ),
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
            "List personal secret metadata and requests; space metadata requires secrets.manage",
            false,
        ),
        entry::<SecretsRevokeInput, SecretsRevokeResult>(
            "secrets.revoke",
            "Destroy a personal secret or request; space management requires secrets.manage",
            true,
        ),
    ];
    catalog.extend(
        crate::PeopleOperation::ALL
            .iter()
            .map(|operation| operation.describe()),
    );
    catalog.extend(
        crate::NOTE_OPERATIONS
            .iter()
            .map(crate::NoteOperation::describe),
    );
    catalog.extend(crate::SearchOperation::ALL.iter().map(|op| op.describe()));
    catalog.extend(account_input_catalog());
    catalog.extend(crate::appearance::catalogue());
    catalog.extend(REQUEST_OPERATIONS.iter().map(RequestOperation::description));
    catalog.extend(crate::AGENT_OPERATIONS.iter().map(|(id, text, mutates)| {
        if *id == crate::MACHINES_INSTALL {
            entry::<crate::MachineOnboardingInput, crate::MachineInstallResult>(id, text, *mutates)
        } else if *id == crate::MACHINES_ONBOARDING {
            entry::<crate::MachineOnboardingInput, crate::MachineOnboardingResult>(
                id, text, *mutates,
            )
        } else if *id == crate::MACHINES_VERIFY {
            entry::<crate::MachineVerifyInput, crate::MachineVerifyResult>(id, text, *mutates)
        } else if *id == crate::AGENT_READINESS {
            entry::<crate::ReadinessInput, crate::ReadinessResult>(id, text, *mutates)
        } else {
            entry::<crate::AgentInput, crate::JsonObject>(id, text, *mutates)
        }
    }));
    catalog
}

/// Borrow a built-in operation's compiled schema and risk metadata. Catalog
/// construction happens once; calls scan the bounded operation list without I/O.
pub fn cached_operation(operation: &str) -> Option<&'static OperationDescription> {
    cached_operation_catalog()
        .iter()
        .find(|d| d.id == operation)
}

/// Borrow the generated operation metadata and schemas, initialized once.
/// Cost after initialization: one immutable reference; no I/O or schema rebuilding.
pub fn cached_operation_catalog() -> &'static [OperationDescription] {
    static CATALOG: std::sync::OnceLock<Vec<OperationDescription>> = std::sync::OnceLock::new();
    CATALOG.get_or_init(operation_catalog)
}
