//! Transport-independent, single-worker session state machine and newline JSON frames.
use crate::{Error, validate_manifest};
use calternal_core::{HostCallKind, ManifestCapability, ManifestV1};
use calternal_types::{AccountId, SpaceId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Maximum bytes per JSON frame, excluding its terminating newline (256 KiB).
pub const MAX_FRAME_BYTES: usize = 256 * 1024;
/// Maximum admitted invocations per worker.
pub const MAX_IN_FLIGHT: usize = 32;
/// Maximum queued host messages or worker deliveries per direction.
pub const MAX_QUEUE: usize = 32;
/// Maximum callbacks per invocation, bounding replay tracking memory.
pub const MAX_CALLBACKS: usize = 128;

/// Host-issued invocation authority. Values are correlation data, never worker-minted grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Authority {
    /// Acting account's independent UUIDv7.
    pub account_id: AccountId,
    /// Space UUIDv7, independent of the account.
    pub space_id: SpaceId,
    /// Opaque host-owned reference to current session/grant attribution.
    pub grant: String,
}
/// One strict worker-v2 envelope. The invocation ID also identifies its stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    /// Must equal 2; other protocols are refused before session mutation.
    pub protocol_version: u32,
    /// Host-issued invocation correlation identifier.
    pub invocation_id: String,
    /// Direction-specific payload.
    pub message: Message,
}
/// Worker-v2 messages; serde rejects unknown keys in every message variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    /// Host calls a worker's declared operation handler.
    Invoke {
        /// Declared public operation ID.
        operation: String,
        /// Host-validated operation input.
        input: Value,
        /// Authority to echo on every callback; it is rechecked by the broker.
        authority: Authority,
    },
    /// Worker requests one declared broker method.
    HostCall {
        /// Unique callback identifier within this invocation.
        call_id: String,
        /// Manifest-declared host method.
        method: HostCallKind,
        /// Method-specific input, checked by the trusted broker.
        input: Value,
        /// Exact authority of the parent invocation.
        authority: Authority,
    },
    /// Host replies to one callback with a bounded JSON result or a safe error.
    HostReply {
        /// Callback correlation identifier.
        call_id: String,
        /// Whether the callback succeeded.
        ok: bool,
        /// JSON result on success, null on refusal. Never raw secrets.
        data: Value,
    },
    /// Ordered ephemeral job output; durability/checkpoints belong to host job services.
    Chunk {
        /// Job handle previously issued by the host for this invocation.
        job_id: String,
        /// Zero-based strictly increasing sequence per invocation.
        sequence: u32,
        /// Bounded chunk payload.
        data: Value,
    },
    /// Successful operation completion; no subsequent message for this invocation is accepted.
    Complete {
        /// Operation result; the session validates the declared output schema before delivery.
        data: Value,
    },
    /// Safe unsuccessful completion, with no plugin diagnostic detail.
    Failed {},
    /// Host cancellation invalidates authority immediately; the adapter enforces a kill deadline.
    Cancel {},
}
/// Cost: O(frame bytes), one serialization buffer allocation; no I/O/locks.
/// Serialize one frame with newline delimiter, rejecting oversized messages before transport.
pub fn encode_frame(frame: &Frame) -> Result<Vec<u8>, Error> {
    validate_frame(frame)?;
    let mut bytes = crate::bounded::json(frame, MAX_FRAME_BYTES)?;
    bytes.push(b'\n');
    Ok(bytes)
}
/// Cost: O(frame bytes), allocates decoded fields, no I/O/locks.
/// Parse one bounded frame. Transport adapters must bound buffering before finding a newline.
pub fn decode_frame(bytes: &[u8]) -> Result<Frame, Error> {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(Error::TooLarge);
    }
    let frame = serde_json::from_slice(bytes).map_err(|_| Error::Invalid)?;
    validate_frame(&frame)?;
    Ok(frame)
}
fn validate_frame(frame: &Frame) -> Result<(), Error> {
    use crate::registry::identifier;
    if frame.protocol_version != 2 || !identifier(&frame.invocation_id) {
        return Err(Error::Invalid);
    }
    match &frame.message {
        Message::Invoke {
            operation,
            authority,
            ..
        } => {
            if !identifier(operation) || !valid_authority(authority) {
                return Err(Error::Invalid);
            }
        }
        Message::HostCall {
            call_id, authority, ..
        } => {
            if !identifier(call_id) || !valid_authority(authority) {
                return Err(Error::Invalid);
            }
        }
        Message::HostReply { call_id, .. } => {
            if !identifier(call_id) {
                return Err(Error::Invalid);
            }
        }
        Message::Chunk { job_id, .. } => {
            if !identifier(job_id) {
                return Err(Error::Invalid);
            }
        }
        _ => {}
    }
    Ok(())
}
fn check_frame(frame: &Frame) -> Result<(), Error> {
    validate_frame(frame)?;
    crate::bounded::check(frame, MAX_FRAME_BYTES)
}
fn valid_authority(authority: &Authority) -> bool {
    authority.space_id.as_uuid() != authority.account_id.as_uuid()
        && !authority.grant.is_empty()
        && authority.grant.len() <= 256
}
/// Trusted boundary implemented by storage/job/secret adapters, not by plugin code.
pub trait HostBroker {
    /// Cost: adapter-defined; may query protected state. Called synchronously with no library lock.
    /// Recheck current session, installation, membership, plugin activation and attenuated grants.
    /// Called for every callback and worker delivery, including chunks and completion.
    fn authorize(&mut self, authority: &Authority, required: ManifestCapability) -> bool;
    /// Cost: adapter-defined validation, I/O and durability; exactly one broker call per callback.
    /// Validate method input/target and apply the effect under current authority atomically.
    /// Secret requests return only `{ "handle": "opaque-id" }`; malformed responses are redacted.
    /// Durable writes require host receipts.
    fn call(
        &mut self,
        authority: &Authority,
        method: HostCallKind,
        input: &Value,
    ) -> Result<Value, Error>;
}
struct Invocation {
    required: ManifestCapability,
    operation: String,
    authority: Authority,
    sequence: u32,
    calls: BTreeSet<String>,
    jobs: BTreeSet<String>,
}
/// A manifest's operation input/output schemas, compiled once per plugin version
/// and shared by every worker session of it.
pub struct CompiledSchemas(BTreeMap<String, (jsonschema::Validator, jsonschema::Validator)>);
impl std::fmt::Debug for CompiledSchemas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.keys()).finish()
    }
}
impl CompiledSchemas {
    /// Validates `manifest` and compiles each operation's two schemas.
    ///
    /// # Errors
    /// Invalid for a manifest or schema that does not validate.
    /// Cost: manifest validation and schema compilation; no I/O.
    pub fn compile(manifest: &ManifestV1) -> Result<Self, Error> {
        validate_manifest(manifest)?;
        manifest
            .operations
            .iter()
            .map(|op| {
                Ok((
                    op.id.clone(),
                    (
                        jsonschema::validator_for(&op.input_schema).map_err(|_| Error::Invalid)?,
                        jsonschema::validator_for(&op.output_schema).map_err(|_| Error::Invalid)?,
                    ),
                ))
            })
            .collect::<Result<_, Error>>()
            .map(Self)
    }
}
/// Long-lived worker session pinned to one manifest version and one space.
/// A process adapter routes frames, drains both bounded queues and enforces deadlines.
pub struct WorkerSession {
    manifest: ManifestV1,
    schemas: std::sync::Arc<CompiledSchemas>,
    space: SpaceId,
    active: BTreeMap<String, Invocation>,
    seen: BTreeSet<String>,
    outgoing: VecDeque<Frame>,
    deliveries: VecDeque<(Frame, Authority, ManifestCapability)>,
}
impl WorkerSession {
    /// Create a session without launching code; one adapter-owned process serves this
    /// pair. `schemas` must be [`CompiledSchemas::compile`] of this same manifest.
    /// Cost: owned state allocations only; no validation, compilation or I/O.
    pub fn new(
        manifest: ManifestV1,
        schemas: std::sync::Arc<CompiledSchemas>,
        space: SpaceId,
    ) -> Result<Self, Error> {
        Ok(Self {
            manifest,
            schemas,
            space,
            active: BTreeMap::new(),
            seen: BTreeSet::new(),
            outgoing: VecDeque::new(),
            deliveries: VecDeque::new(),
        })
    }
    /// Cost: O(frame bytes + compiled schema evaluation + log bounded state), one authorization check.
    /// Owns queued input; the byte-budget pass allocates no output buffer. No library I/O/locks.
    /// Admit a host-authorized invocation and queue its operation call.
    /// At 4096 lifetime invocations the adapter must drain and rotate the worker.
    pub fn invoke(
        &mut self,
        id: String,
        operation: String,
        input: Value,
        authority: Authority,
        broker: &mut impl HostBroker,
    ) -> Result<(), Error> {
        if self.outgoing.len() >= MAX_QUEUE
            || self.active.len() >= MAX_IN_FLIGHT
            || self.seen.len() >= 4096
        {
            return Err(Error::BackPressure);
        }
        if self.seen.contains(&id)
            || authority.space_id != self.space
            || authority.account_id.as_uuid() == self.space.as_uuid()
            || authority.grant.is_empty()
            || authority.grant.len() > 256
            || !self
                .schemas
                .0
                .get(&operation)
                .is_some_and(|(input_schema, _)| input_schema.is_valid(&input))
        {
            return Err(Error::Invalid);
        }
        let frame = Frame {
            protocol_version: 2,
            invocation_id: id.clone(),
            message: Message::Invoke {
                operation: operation.clone(),
                input,
                authority: authority.clone(),
            },
        };
        check_frame(&frame)?;
        let required = self
            .manifest
            .operations
            .iter()
            .find(|op| op.id == operation)
            .ok_or(Error::Invalid)?
            .required_capability;
        if !broker.authorize(&authority, required) {
            return Err(Error::Refused);
        }
        self.seen.insert(id.clone());
        self.active.insert(
            id,
            Invocation {
                required,
                operation,
                authority,
                sequence: 0,
                calls: BTreeSet::new(),
                jobs: BTreeSet::new(),
            },
        );
        self.outgoing.push_back(frame);
        Ok(())
    }
    /// Cost: O(log bounded state), one small allocation; no I/O, locks or authority check.
    /// Let an admitted invocation emit chunks for a job the host already admitted, as
    /// when the host invokes a job's run operation. Jobs a worker starts itself are
    /// attached by a successful `start_job` reply instead.
    pub fn attach_job(&mut self, id: &str, job_id: &str) -> Result<(), Error> {
        if !crate::registry::identifier(job_id) {
            return Err(Error::Invalid);
        }
        let invocation = self.active.get_mut(id).ok_or(Error::Invocation)?;
        if invocation.jobs.len() >= MAX_CALLBACKS {
            return Err(Error::BackPressure);
        }
        invocation.jobs.insert(job_id.to_owned());
        Ok(())
    }
    /// Cost: O(frame bytes + schema evaluation + log bounded state), bounded queue allocations.
    /// At most one broker effect plus two authority checks; no library I/O or locks.
    /// Receive one worker frame. Full queues refuse before callbacks or state changes.
    /// Undeclared/forged calls are refused before effects; malformed secret replies are redacted.
    pub fn receive(&mut self, frame: Frame, broker: &mut impl HostBroker) -> Result<(), Error> {
        check_frame(&frame)?;
        let invocation = self
            .active
            .get_mut(&frame.invocation_id)
            .ok_or(Error::Invocation)?;
        match &frame.message {
            Message::HostCall {
                call_id,
                method,
                input,
                authority,
            } => {
                if self.outgoing.len() >= MAX_QUEUE {
                    return Err(Error::BackPressure);
                }
                if !self.manifest.host_calls.contains(method)
                    || authority != &invocation.authority
                    || !crate::registry::identifier(call_id)
                    || invocation.calls.contains(call_id)
                    || invocation.calls.len() >= MAX_CALLBACKS
                {
                    return Err(Error::Refused);
                }
                if *method == HostCallKind::WriteItem
                    && !self
                        .manifest
                        .operations
                        .iter()
                        .any(|op| op.id == invocation.operation && op.mutates)
                {
                    return Err(Error::Refused);
                }
                if *method == HostCallKind::RequestSecretHandle
                    && !(input.as_object().is_some_and(|o| o.len() == 1)
                        && input
                            .get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|name| self.manifest.secrets.iter().any(|s| s == name)))
                {
                    return Err(Error::Refused);
                }
                if !broker.authorize(authority, invocation.required) {
                    return Err(Error::Refused);
                }
                // Reserve replay identity before any effect; even a failed broker effect cannot be retried blindly.
                invocation.calls.insert(call_id.clone());
                let result = broker.call(authority, *method, input);
                let (mut ok, mut data) = match result {
                    Ok(data) => (true, data),
                    Err(_) => (false, Value::Null),
                };
                if !broker.authorize(authority, invocation.required) {
                    ok = false;
                    data = Value::Null;
                }
                if *method == HostCallKind::RequestSecretHandle
                    && !(data.as_object().is_some_and(|o| o.len() == 1)
                        && data
                            .get("handle")
                            .and_then(Value::as_str)
                            .is_some_and(crate::registry::identifier))
                {
                    ok = false;
                    data = Value::Null;
                }
                let job = if *method == HostCallKind::StartJob {
                    data.get("job_id")
                        .and_then(Value::as_str)
                        .filter(|id| crate::registry::identifier(id))
                        .map(str::to_owned)
                } else {
                    None
                };
                let mut reply = Frame {
                    protocol_version: 2,
                    invocation_id: frame.invocation_id,
                    message: Message::HostReply {
                        call_id: call_id.clone(),
                        ok,
                        data,
                    },
                };
                if check_frame(&reply).is_err() {
                    ok = false;
                    reply.message = Message::HostReply {
                        call_id: call_id.clone(),
                        ok: false,
                        data: Value::Null,
                    };
                }
                if ok {
                    if let Some(job) = job {
                        invocation.jobs.insert(job);
                    }
                }
                self.outgoing.push_back(reply);
            }
            Message::Chunk {
                job_id, sequence, ..
            } => {
                if self.deliveries.len() >= MAX_QUEUE {
                    return Err(Error::BackPressure);
                }
                if *sequence != invocation.sequence || !invocation.jobs.contains(job_id) {
                    return Err(Error::Invalid);
                }
                if !broker.authorize(&invocation.authority, invocation.required) {
                    return Err(Error::Refused);
                }
                invocation.sequence = invocation.sequence.checked_add(1).ok_or(Error::Invalid)?;
                self.deliveries.push_back((
                    frame,
                    invocation.authority.clone(),
                    invocation.required,
                ));
            }
            Message::Complete { .. } | Message::Failed {} => {
                if self.deliveries.len() >= MAX_QUEUE {
                    return Err(Error::BackPressure);
                }
                if !broker.authorize(&invocation.authority, invocation.required) {
                    return Err(Error::Refused);
                }
                if let Message::Complete { data } = &frame.message {
                    if !self.schemas.0[&invocation.operation].1.is_valid(data) {
                        return Err(Error::Invalid);
                    }
                }
                let authority = invocation.authority.clone();
                let required = invocation.required;
                self.active.remove(&frame.invocation_id);
                self.deliveries.push_back((frame, authority, required));
            }
            _ => return Err(Error::Invalid),
        }
        Ok(())
    }
    /// Cost: O(bounded queued frames), drops queued payloads, no I/O/locks or rollback.
    /// Cancel locally before sending a cancellation message; late output/callbacks are refused.
    /// Local invalidation happens even if other invocations fill the queue: BackPressure then
    /// means the adapter must stop the worker because its cancellation message could not be sent.
    /// Already delivered effects require reconciliation; cancellation never implies rollback.
    pub fn cancel(&mut self, id: &str) -> Result<(), Error> {
        if self.active.remove(id).is_none()
            && !self
                .deliveries
                .iter()
                .any(|(f, _, _)| f.invocation_id == id)
        {
            return Err(Error::Invocation);
        }
        self.outgoing.retain(|f| f.invocation_id != id);
        self.deliveries.retain(|(f, _, _)| f.invocation_id != id);
        if self.outgoing.len() >= MAX_QUEUE {
            return Err(Error::BackPressure);
        }
        self.outgoing.push_back(Frame {
            protocol_version: 2,
            invocation_id: id.into(),
            message: Message::Cancel {},
        });
        Ok(())
    }
    /// Drain one host frame after checking current authority; cancelled messages bypass that check.
    /// Revoked or completed invocation payloads are discarded before transport delivery.
    /// Cost: O(1) plus one broker authorization check, no allocation or library I/O/locks.
    pub fn next_outgoing(&mut self, broker: &mut impl HostBroker) -> Result<Option<Frame>, Error> {
        let Some(frame) = self.outgoing.pop_front() else {
            return Ok(None);
        };
        if !matches!(frame.message, Message::Cancel {}) {
            let invocation = self
                .active
                .get(&frame.invocation_id)
                .ok_or(Error::Invocation)?;
            if !broker.authorize(&invocation.authority, invocation.required) {
                return Err(Error::Refused);
            }
        }
        Ok(Some(frame))
    }
    /// Drain one result after a fresh authority check; revoked payloads are discarded.
    /// Cost: O(1) queue removal plus one broker authorization check, no library I/O.
    pub fn next_delivery(&mut self, broker: &mut impl HostBroker) -> Result<Option<Frame>, Error> {
        match self.deliveries.pop_front() {
            Some((frame, authority, required)) if broker.authorize(&authority, required) => {
                Ok(Some(frame))
            }
            Some(_) => Err(Error::Refused),
            None => Ok(None),
        }
    }
}
