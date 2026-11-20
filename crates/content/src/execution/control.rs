//! Durable steer and Stop delivery ledger for one exact attempt. A control is
//! committed here before its acknowledgement, dispatched after the writer is
//! released, and settled only by the harness's effect acknowledgement for the
//! same attempt, sequence and request ID. Transport ACK is not effect ACK.
use super::*;

/// Most unsettled steers one attempt may hold (`control_limits`).
pub const PENDING_STEERS_PER_ATTEMPT: i64 = 32;
/// Largest encoded steer delivery, counting the trailing LF.
pub const ENCODED_STEER_BYTES: usize = 65_536;

/// Kind of control delivered to the harness.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlKind {
    /// Text appended to the running attempt's input.
    Steer,
    /// Exact Stop for the attempt.
    Stop,
}

/// Harness-reported effect of one control (`ExecutionControlAck.state`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlState {
    /// The harness applied the control.
    Applied,
    /// The harness refused it; no effect.
    Refused,
    /// The target was already terminal; no effect.
    Stale,
}

impl ControlState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Refused => "refused",
            Self::Stale => "stale",
        }
    }
}

/// One outstanding delivery in the wire shape `ExecutionControlDelivery`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ControlDelivery {
    /// Exact attempt the control is pinned to.
    pub execution: ExecutionRef,
    /// Per-attempt delivery sequence, starting at one.
    pub seq: i64,
    /// Logical request ID the harness deduplicates on.
    pub request_id: String,
    /// Steer or Stop.
    pub kind: ControlKind,
    /// Steer text; absent for Stop.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Append one checked steer for the exact live attempt before replying.
/// Repeating the same request and text returns the same sequence; changed
/// text under the same request is refused. Refuses a retired attempt, more
/// than 32 unsettled steers, or an encoded delivery over 64 KiB with its LF.
/// The Stop slot is reserved separately. Cost: one indexed attempt read, one
/// request lookup, one bounded count and one insert.
pub fn steer(tx: &Transaction<'_>, attempt: &Attempt, request: &str, text: &str) -> Result<i64> {
    if text.is_empty() || text.len() + 1 > ENCODED_STEER_BYTES || request.is_empty() {
        return Err(Error::new(
            calternal_types::ErrorCode::TooLarge,
            "Steer text is empty or exceeds the delivery limit",
        ));
    }
    if live_pin(tx, attempt)?.is_none() {
        return Err(unavailable());
    }
    let existing = tx
        .prepare_cached(
            "SELECT seq,kind,text FROM execution_controls WHERE execution=?1 AND request_id=?2",
        )
        .map_err(internal)?
        .query_row(params![attempt.execution.0, request], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })
        .optional()
        .map_err(internal)?;
    if let Some((seq, kind, stored)) = existing {
        return if kind == "steer" && stored.as_deref() == Some(text) {
            Ok(seq)
        } else {
            Err(Error::request_reused())
        };
    }
    let pending = tx
        .prepare_cached(
            "SELECT count(*) FROM execution_controls
             WHERE execution=?1 AND kind='steer' AND state IN ('pending','sent')",
        )
        .map_err(internal)?
        .query_row([&attempt.execution.0], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    if pending >= PENDING_STEERS_PER_ATTEMPT {
        return Err(pending_limit());
    }
    insert(
        tx,
        &attempt.execution,
        request,
        ControlKind::Steer,
        Some(text),
    )
}

/// Queue the exact Stop for an attempt that was just retired. A second
/// retirement of the same attempt keeps the first Stop. Undelivered steers
/// that were never sent are retired as undelivered. Cost: two indexed writes.
pub(super) fn queue_stop(
    tx: &Transaction<'_>,
    execution: &ExecutionRef,
    request: &str,
) -> Result<()> {
    let exists = tx
        .prepare_cached("SELECT 1 FROM execution_controls WHERE execution=?1 AND kind='stop'")
        .map_err(internal)?
        .exists([&execution.0])
        .map_err(internal)?;
    if !exists {
        insert(tx, execution, request, ControlKind::Stop, None)?;
    }
    tx.prepare_cached(
        "UPDATE execution_controls SET state='undelivered'
         WHERE execution=?1 AND kind='steer' AND state='pending'",
    )
    .map_err(internal)?
    .execute([&execution.0])
    .map_err(internal)?;
    Ok(())
}

fn insert(
    tx: &Transaction<'_>,
    execution: &ExecutionRef,
    request: &str,
    kind: ControlKind,
    text: Option<&str>,
) -> Result<i64> {
    let seq = tx
        .prepare_cached("SELECT coalesce(max(seq),0)+1 FROM execution_controls WHERE execution=?1")
        .map_err(internal)?
        .query_row([&execution.0], |r| r.get::<_, i64>(0))
        .map_err(internal)?;
    tx.prepare_cached(
        "INSERT INTO execution_controls(execution,seq,request_id,kind,text) VALUES(?1,?2,?3,?4,?5)",
    )
    .map_err(internal)?
    .execute(params![
        execution.0,
        seq,
        request,
        match kind {
            ControlKind::Steer => "steer",
            ControlKind::Stop => "stop",
        },
        text
    ])
    .map_err(internal)?;
    Ok(seq)
}

/// Outstanding deliveries for one exact attempt, in dispatch order: an
/// unsettled Stop first, then unsettled steers by sequence while the attempt
/// is live. A retired attempt yields only its Stop (the cleanup-only path).
/// Marks every returned row as sent, so a Stop retirement afterwards keeps
/// them outstanding until the harness reports their effect. Cost: one
/// indexed read bounded by 33 rows and one update.
pub fn outstanding(tx: &Transaction<'_>, attempt: &Attempt) -> Result<Vec<ControlDelivery>> {
    let rows: Vec<ControlDelivery> = tx
        .prepare_cached(
            "SELECT c.seq,c.request_id,c.kind,c.text FROM execution_controls c
             JOIN execution_attempts a ON a.execution=c.execution
             WHERE c.execution=?1 AND a.generation=?2 AND c.state IN ('pending','sent')
               AND (c.kind='stop' OR a.end_reason IS NULL)
             ORDER BY c.kind='steer', c.seq",
        )
        .map_err(internal)?
        .query_map(params![attempt.execution.0, attempt.generation], |r| {
            Ok(ControlDelivery {
                execution: attempt.execution.clone(),
                seq: r.get(0)?,
                request_id: r.get(1)?,
                kind: if r.get::<_, String>(2)? == "stop" {
                    ControlKind::Stop
                } else {
                    ControlKind::Steer
                },
                text: r.get(3)?,
            })
        })
        .map_err(internal)?
        .collect::<rusqlite::Result<_>>()
        .map_err(internal)?;
    tx.prepare_cached(
        "UPDATE execution_controls SET state='sent' WHERE execution=?1 AND state='pending'
           AND (kind='stop' OR EXISTS(SELECT 1 FROM execution_attempts
                WHERE execution=?1 AND end_reason IS NULL))",
    )
    .map_err(internal)?
    .execute([&attempt.execution.0])
    .map_err(internal)?;
    Ok(rows)
}

/// Settle one delivery from the harness's effect acknowledgement. Only an
/// exact attempt, sequence and request match on an unsettled row changes
/// state; anything else (a stale epoch, a forged or duplicate ACK) returns
/// `false` and leaves the ledger as it was. Cost: one indexed update.
pub fn ack_control(
    tx: &Transaction<'_>,
    execution: &ExecutionRef,
    seq: i64,
    request: &str,
    state: ControlState,
) -> Result<bool> {
    tx.prepare_cached(
        "UPDATE execution_controls SET state=?4
         WHERE execution=?1 AND seq=?2 AND request_id=?3 AND state IN ('pending','sent')",
    )
    .map_err(internal)?
    .execute(params![execution.0, seq, request, state.as_str()])
    .map(|changed| changed == 1)
    .map_err(internal)
}
