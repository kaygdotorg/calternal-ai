//! Execution broker methods called only after installed package resolution.
use super::*;
use calternal_content::execution::{ClaimedExecution, PinnedExecution};

/// Admission resolved by the host from the invocation, exact installed job,
/// and checked harness and placement. Only the checked carrier broker creates
/// it. The opaque body of a submitted job cannot select these bindings.
pub struct CheckedAdmission {
    pub(super) space: SpaceId,
    pub(super) pin: PinnedExecution,
    pub(super) target_field: String,
}

impl CheckedAdmission {
    /// Resolve a checked ticket and placement against the installed package.
    /// The invoking operation must declare the broker method, and the owning
    /// run must be replayable with one eligible item target. Cost: indexed
    /// catalogue lookups and a scan of at most 32 declared jobs, with no I/O.
    pub fn resolve(
        catalog: &super::super::Catalog,
        space: SpaceId,
        pin: PinnedExecution,
        invoking_operation: &str,
    ) -> Result<Self> {
        let (invoker, invocation) = catalog
            .v2_operation(invoking_operation)
            .ok_or_else(unavailable)?;
        let (owner, run) = catalog
            .v2_operation(&pin.operation)
            .ok_or_else(unavailable)?;
        let (kind_owner, kind) = catalog.v2_kind(&pin.kind).ok_or_else(unavailable)?;
        let package = pin.package.0.as_str();
        let accepts_start = invocation["methods"]
            .as_array()
            .is_some_and(|methods| methods.iter().any(|method| method == "start_job"));
        let executable = owner.declaration()["jobs"].as_array().is_some_and(|jobs| {
            jobs.iter().any(|job| {
                job["id"] == pin.declaration
                    && job["run_operation"] == pin.operation
                    && job["replayable"] == true
                    && job["execution"] == true
            })
        });
        if [invoker.id(), owner.id(), kind_owner.id()] != [package; 3]
            || !accepts_start
            || !executable
            || kind["execution_target"] != true
            || run["target"]["kind"] != pin.kind
        {
            return Err(unavailable());
        }
        let target_field = run["target"]["field"]
            .as_str()
            .ok_or_else(unavailable)?
            .to_owned();
        Ok(Self {
            space,
            pin,
            target_field,
        })
    }

    /// Accept one execution `start_job` broker request. The supplied validators
    /// check the installed run schema and preflight both contribution frames.
    /// `live` checks installed authority and the current item under the content
    /// writer before a receipt or effect. Call from the blocking host pool.
    /// Cost: one canonical fingerprint, bounded validation and one receipt path.
    pub fn start_job(
        mut self,
        content: &Content,
        request: &RequestId,
        input: JsonObject,
        validate_run: impl FnOnce(&Value) -> Result<()>,
        preflight: impl FnOnce(&Value) -> Result<()>,
        live: impl FnOnce(&PinnedExecution) -> Result<()>,
    ) -> Result<Value> {
        const OP: &str = "start_job";
        let fingerprint = crate::api::fingerprint(OP, &input);
        let operands = input.keys().all(|key| {
            matches!(
                key.as_str(),
                "job" | "item" | "input" | "harness" | "placement" | "contribution_input"
            )
        });
        let checked_placement = self.pin.placement.as_ref().map(|p| p.0.as_str());
        if !operands
            || input.get("job").and_then(Value::as_str) != Some(self.pin.declaration.as_str())
            || input.get("item").and_then(Value::as_str) != Some(self.pin.item.0.as_str())
            || input.get("harness").and_then(Value::as_str) != Some(self.pin.harness.0.as_str())
            || match input.get("placement") {
                Some(value) => value
                    .as_str()
                    .is_none_or(|value| Some(value) != checked_placement),
                None => checked_placement.is_some(),
            }
        {
            return Err(invalid());
        }
        let mut input = input;
        self.pin.input = bind_target(
            input.remove("input").ok_or_else(invalid)?,
            &self.target_field,
            &self.pin.item,
        )?;
        self.pin.contribution_input = input.remove("contribution_input").ok_or_else(invalid)?;
        validate_run(&self.pin.input)?;
        preflight(&self.pin.contribution_input)?;
        let via = Via::Plugin {
            plugin: self.pin.package.0.clone(),
        };
        content.execution_command(
            self.space,
            Request {
                account: self.pin.actor,
                via: Some(&via),
                id: &request.0,
                operation: OP,
                fingerprint: &fingerprint,
            },
            || live(&self.pin),
            |tx| {
                let admitted =
                    execution::admit(tx, &self.pin, super::super::jobs::MAX_ACTIVE_JOBS_PER_SPACE)?;
                Ok(json!({"job_id": admitted.job}))
            },
        )
    }
}

pub(super) fn bind_target(mut input: Value, field: &str, item: &ItemRef) -> Result<Value> {
    let object = input.as_object_mut().ok_or_else(invalid)?;
    if object.len() > 32 || object.contains_key(field) {
        return Err(invalid());
    }
    object.insert(field.to_owned(), json!(item));
    Ok(input)
}

/// One checked dispatch whose invocation context is created by the host.
/// A caller must retain it with the owning worker ticket. Worker input cannot
/// construct this handle or select a different job for its `execution_start`.
pub struct OwningJob {
    space: SpaceId,
    job: JobId,
    claimed: ClaimedExecution,
}

/// Claim one eligible execution head through the blocking content writer.
/// A permanent live refusal retires the pin. Cost: one indexed job lookup and
/// at most one FULL commit, plus the supplied live check.
pub fn claim_job(
    content: &Content,
    space: SpaceId,
    job: JobId,
    live: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Option<OwningJob>> {
    content.execution_transaction(space, |tx| {
        Ok(execution::claim(tx, &job, live)?.map(|claimed| OwningJob {
            space,
            job,
            claimed,
        }))
    })
}

impl OwningJob {
    /// Installed operation to dispatch. No I/O.
    pub fn operation(&self) -> &str {
        &self.claimed.pin.operation
    }

    /// Original input validated against that operation. No I/O.
    pub fn input(&self) -> &Value {
        &self.claimed.pin.input
    }

    /// Render the checked job context for the carrier invocation. Allocates one
    /// frame-bounded JSON object. The runner never accepts a copy from a worker.
    pub fn context(&self) -> Value {
        json!({
            "intent": self.claimed.intent,
            "item": self.claimed.pin.item,
            "harness": self.claimed.pin.harness,
            "contribution_identity": self.claimed.pin.contribution,
            "contribution_input": self.claimed.pin.contribution_input,
        })
    }

    /// Accept `execution_start` for the owning job on a blocking worker. The
    /// live check must verify the whole pinned authority even on receipt replay.
    /// Returns starting status. Contribution launch is a separate checked call.
    /// Cost: one fingerprint, the receipt path and one exact attempt lookup.
    pub fn start_attempt(
        &self,
        content: &Content,
        input: JsonObject,
        live: impl FnOnce(&PinnedExecution) -> Result<()>,
    ) -> Result<Value> {
        const OP: &str = "execution_start";
        let request_id = input
            .get("request_id")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let harness = input
            .get("harness")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        if input.len() != 2 || request_id.is_empty() || request_id.len() > 128 {
            return Err(invalid());
        }
        let fingerprint = crate::api::fingerprint(OP, &input);
        let pin = &self.claimed.pin;
        let via = Via::Plugin {
            plugin: pin.package.0.clone(),
        };
        // Different jobs can legitimately author the same request ID.
        let receipt = format!("execution/{}/{request_id}", self.job.0);
        content.execution_command(
            self.space,
            Request {
                account: pin.actor,
                via: Some(&via),
                id: &receipt,
                operation: OP,
                fingerprint: &fingerprint,
            },
            || live(pin),
            |tx| {
                let started =
                    execution::start(tx, &self.job, &HarnessRef(harness.to_owned()), |stored| {
                        Ok(stored == pin)
                    })?;
                Ok(json!({
                    "execution": started.attempt.execution,
                    "phase": "starting",
                    "transition_cursor": started.transition,
                }))
            },
        )
    }
}
