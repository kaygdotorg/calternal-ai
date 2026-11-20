//! Exact reattachment on a fresh enrolled carrier, and the control ledger
//! round trip. The machine-session layer supplies the authenticated machine
//! and boot; Slice 1 supplies the committed output cursor (`after`). Every
//! call runs on a blocking worker and reports the unsettled count to the gate.
use super::update::UpdateGate;
use super::*;
use calternal_content::execution::{
    self, Attached, Attempt, ControlDelivery, ControlState, LaunchBinding, PinnedExecution,
    Reattach,
};
use uuid::Uuid;

/// What the carrier must do after a reconnect offers one reservation.
#[derive(Debug, PartialEq)]
pub enum Recovery {
    /// Send `ResumeEntry { origin, after }` on the fresh carrier, verify the
    /// reply with [`verify_attached`], then deliver `controls` in order.
    Resume {
        /// Exact attempt.
        attempt: Attempt,
        /// Committed launch identity; `origin` is its reservation.
        binding: LaunchBinding,
        /// Outstanding controls, Stop first.
        controls: Vec<ControlDelivery>,
    },
    /// Attach for cleanup only: deliver the exact Stop in `controls`, accept
    /// no new effects or output publication, then await teardown proof.
    CleanupOnly {
        /// Exact retired attempt.
        attempt: Attempt,
        /// Committed launch identity.
        binding: LaunchBinding,
        /// The outstanding Stop, if not yet settled.
        controls: Vec<ControlDelivery>,
    },
    /// New boot: interruption is committed; the cleanup dispatcher takes over.
    /// No replacement starts; a fresh run is the owning plugin's choice.
    Failed {
        /// Exact attempt now retired.
        attempt: Attempt,
    },
}

/// Look up the committed binding for an offered reservation, re-authorize
/// through `live`, and return the recovery plan. `None` means unknown or
/// foreign: send nothing and change nothing. Cost: one execution transaction
/// with two indexed reads, at most one retirement and one bounded ledger read.
#[allow(clippy::too_many_arguments)]
pub fn reattach(
    content: &Content,
    gate: &UpdateGate,
    space: SpaceId,
    reservation: Uuid,
    machine: Uuid,
    boot: Uuid,
    live: impl FnOnce(&PinnedExecution) -> Result<bool>,
) -> Result<Option<Recovery>> {
    content.execution_transaction(space, &|n| gate.observe(space, n), |tx| {
        Ok(
            match execution::reattach(tx, &reservation, &machine, &boot, live)? {
                None => None,
                Some(Reattach::Resume {
                    attempt, binding, ..
                }) => Some(Recovery::Resume {
                    controls: execution::outstanding(tx, &attempt)?,
                    attempt,
                    binding,
                }),
                Some(Reattach::CleanupOnly { attempt, binding }) => Some(Recovery::CleanupOnly {
                    controls: execution::outstanding(tx, &attempt)?,
                    attempt,
                    binding,
                }),
                Some(Reattach::Failed { attempt }) => Some(Recovery::Failed { attempt }),
            },
        )
    })
}

/// Verify `EntryAttached { origin, buffered_through }` against the committed
/// reservation, boot and generation, given the `after` sent in ResumeEntry.
/// A refusal changes nothing; close the carrier. Cost: two indexed reads.
#[allow(clippy::too_many_arguments)]
pub fn verify_attached(
    content: &Content,
    gate: &UpdateGate,
    space: SpaceId,
    reservation: Uuid,
    boot: Uuid,
    origin: Uuid,
    buffered_through: u64,
    after: u64,
) -> Result<Attached> {
    content.execution_transaction(space, &|n| gate.observe(space, n), |tx| {
        execution::verify_attached(tx, &reservation, &boot, &origin, buffered_through, after)
    })
}

/// Apply one harness `ExecutionControlAck`. Returns whether it settled an
/// outstanding delivery; a stale, forged or duplicate ACK returns `false`.
/// Cost: one schema check and one indexed update.
pub fn ack_control(
    content: &Content,
    gate: &UpdateGate,
    space: SpaceId,
    ack: &Value,
) -> Result<bool> {
    let object = ack.as_object().ok_or_else(invalid)?;
    let execution = object.get("execution").and_then(Value::as_str);
    let seq = object.get("seq").and_then(Value::as_i64);
    let request = object.get("request_id").and_then(Value::as_str);
    let state: Option<ControlState> = object
        .get("state")
        .and_then(|state| serde_json::from_value(state.clone()).ok());
    let (Some(execution), Some(seq), Some(request), Some(state)) = (execution, seq, request, state)
    else {
        return Err(invalid());
    };
    if object.len() != 4 || seq < 1 {
        return Err(invalid());
    }
    content.execution_transaction(space, &|n| gate.observe(space, n), |tx| {
        execution::ack_control(
            tx,
            &calternal_types::ExecutionRef(execution.to_owned()),
            seq,
            request,
            state,
        )
    })
}
