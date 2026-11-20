//! Transaction-scoped storage for fenced contribution calls and their output.
use crate::{Result, types::internal};
use calternal_types::{Error, ErrorCode, ExecutionRef, MAX_FRAME_BYTES, contribution_limits};
use rusqlite::{OptionalExtension, Transaction, params};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

mod rate;
use rate::{Bucket, Capacity};

// The wire contract is tighter than the enclosing worker message.
const SPACE_BYTES: i64 = 128 * 1024 * 1024;
const SPACE_CALLS: i64 = 4096;
const BATCH_FRAMES: usize = 16;
const MAX_SEQ: i64 = 9_007_199_254_740_991;
// Reserves a failed terminal at the largest supported sequence, including LF.
const TERMINAL_RESERVE: i64 = 64;

/// Result of a producer submission. A fresh frame or sealed terminal must
/// commit before the transport acknowledges or refuses the submitted frame.
#[derive(Debug, PartialEq, Eq)]
pub enum Ingest {
    /// The new producer frame was stored in sequence.
    Stored,
    /// The exact producer frame was already committed.
    Replay,
    /// The host stored (or already stored) a terminal failure. The producer's
    /// refused frame is not committed and the caller must end the producer.
    Sealed,
}

/// Access to one transaction's checked execution attempt. Creation requires
/// the execution engine's live authority check in this same transaction.
pub struct Spool<'tx, 'conn> {
    tx: &'tx Transaction<'conn>,
    execution: ExecutionRef,
    generation: i64,
}

impl<'tx, 'conn> Spool<'tx, 'conn> {
    /// Run the live fence before touching this attempt's spool. Reacquire
    /// after waiting or reconnect. Keep this transaction through commit.
    /// Cost: the supplied exact-attempt check and a short identity clone.
    pub fn checked(
        tx: &'tx Transaction<'conn>,
        execution: &ExecutionRef,
        generation: i64,
        live: impl FnOnce(&Transaction<'conn>, &ExecutionRef, i64) -> Result<bool>,
    ) -> Result<Self> {
        if generation <= 0 || !live(tx, execution, generation)? {
            return Err(unavailable());
        }
        Ok(Self {
            tx,
            execution: execution.clone(),
            generation,
        })
    }

    /// Exact execution checked for this transaction. The broker uses it
    /// when encoding the admitted call. No further reads or allocation.
    pub fn execution(&self) -> &ExecutionRef {
        &self.execution
    }

    /// Allocate one call under the caller's existing operation receipt transaction.
    /// Use the actor's ordinary effect receipt for exact request replay; a new
    /// request cannot reserve a second call for the same attempt. Does no dispatch.
    /// Cost: bounded expiry sweep, one admission insert and one budget update.
    pub fn reserve(&self, now: i64) -> Result<Uuid> {
        prune(self.tx, now)?;
        let budget = budget(self.tx)?;
        if budget.calls >= SPACE_CALLS || budget.bytes > SPACE_BYTES - TERMINAL_RESERVE {
            return Err(Error::unavailable(
                "This space's contribution spool is full",
            ));
        }
        let call = Uuid::now_v7();
        let created = self
            .tx
            .execute(
                "INSERT INTO contribution_calls(call,execution,generation,allocated)
             VALUES(?1,?2,?3,?4) ON CONFLICT(execution) DO NOTHING",
                params![
                    call.to_string(),
                    self.execution.0,
                    self.generation,
                    TERMINAL_RESERVE
                ],
            )
            .map_err(internal)?;
        if created == 0 {
            return Err(Error::conflict(
                "This attempt already has a contribution call",
            ));
        }
        self.tx
            .execute(
                "UPDATE contribution_budget SET calls=calls+1,bytes=bytes+?1 WHERE singleton=1",
                [TERMINAL_RESERVE],
            )
            .map_err(internal)?;
        Ok(call)
    }

    /// Claim launch once. Only the caller receiving true may dispatch after
    /// commit. A crash between commit and dispatch leaves the call claimed;
    /// recovery verifies the exact runtime or seals failure without relaunch.
    /// Cost: one indexed update. Claim replay and reads never launch.
    pub fn claim_launch(&self, call: Uuid) -> Result<bool> {
        let changed = self.tx.prepare_cached(
            "UPDATE contribution_calls SET claimed=1
             WHERE call=?1 AND execution=?2 AND generation=?3 AND claimed=0 AND terminal=0 AND expires IS NULL",
        ).map_err(internal)?.execute(params![call.to_string(), self.execution.0, self.generation])
            .map_err(internal)?;
        Ok(changed == 1)
    }

    /// Persist one checked, ordered producer frame and its replay identity.
    /// The validator checks the data against both installed output declarations.
    /// A cap breach uses the terminal reservation and returns `Sealed`.
    /// Cost: one indexed call read, one receipt read on replay, or one budget
    /// read and up to three bounded writes on a fresh frame.
    pub fn ingest(
        &self,
        call: Uuid,
        frame: &Value,
        now: i64,
        validate_output: impl FnOnce(&Value) -> bool,
    ) -> Result<Ingest> {
        self.ingest_frame(call, frame, now, validate_output, false)
    }

    /// Drain one frame retained by the enrolled machine after exact-attempt
    /// reattachment. The authenticated carrier chooses this path for its
    /// buffered frames. On temporary ingest-credit exhaustion, keep the
    /// frame on the machine and retry after releasing the transaction.
    /// Acknowledge stored or replayed frames only after commit.
    /// Uses the same reads and writes as ordinary ingest.
    pub fn ingest_buffered(
        &self,
        call: Uuid,
        frame: &Value,
        now: i64,
        validate_output: impl FnOnce(&Value) -> bool,
    ) -> Result<Ingest> {
        self.ingest_frame(call, frame, now, validate_output, true)
    }

    fn ingest_frame(
        &self,
        call: Uuid,
        frame: &Value,
        now: i64,
        validate_output: impl FnOnce(&Value) -> bool,
        buffered: bool,
    ) -> Result<Ingest> {
        let (seq, output) = frame_shape(frame)?;
        let state = self.call(call)?;
        let encoded = serde_json::to_vec(frame).map_err(internal)?;
        if seq < state.next {
            let saved = self
                .tx
                .query_row(
                    "SELECT payload,producer FROM contribution_frames WHERE call=?1 AND seq=?2",
                    params![call.to_string(), seq],
                    |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, bool>(1)?)),
                )
                .map_err(internal)?;
            return if saved.0 == encoded && saved.1 {
                Ok(Ingest::Replay)
            } else if !saved.1 {
                Ok(Ingest::Sealed)
            } else {
                Err(Error::request_reused())
            };
        }
        if state.terminal || !state.claimed {
            return Err(unavailable());
        }
        if seq != state.next {
            return Err(Error::conflict("Contribution frames must be contiguous"));
        }
        let size = (encoded.len() + 1) as i64;
        let space = budget(self.tx)?;
        let limits = contribution_limits();
        if size > limits.encoded_frame_bytes
            || (output.is_some()
                && (state.allocated + size > limits.encoded_spool_bytes
                    || space.bytes + size > SPACE_BYTES))
        {
            self.seal_failed(call, seq)?;
            return Ok(Ingest::Sealed);
        }
        if output.is_some_and(|data| !validate_output(data)) {
            return Err(Error::input_field(
                "/data",
                "must match the checked contribution output",
            ));
        }
        let credit = state
            .rate
            .charge(
                Capacity::new(
                    limits.encoded_bytes_per_second_per_attempt,
                    limits.frames_per_second_per_attempt,
                ),
                now,
                size,
            )
            .zip(space.rate.charge(
                Capacity::new(
                    limits.encoded_bytes_per_second_per_space,
                    limits.frames_per_second_per_space,
                ),
                now,
                size,
            ));
        if credit.is_none() {
            if buffered {
                return Err(Error::new(
                    ErrorCode::RetryAfter,
                    "Wait for contribution ingest credit",
                )
                .with_details(json!({"retry_after_ms":1000})));
            }
            self.seal_failed(call, seq)?;
            return Ok(Ingest::Sealed);
        }
        self.store(call, seq, &encoded, output.is_none(), credit)?;
        Ok(Ingest::Stored)
    }

    /// Seal a claimed call when the verified entry exits without a terminal.
    /// Repeated exit notifications replay the sealed state. Cost: one call
    /// lookup and, if needed, one reserved terminal insert with counter updates.
    pub fn failed_exit(&self, call: Uuid) -> Result<()> {
        let state = self.call(call)?;
        if !state.claimed {
            return Err(unavailable());
        }
        if !state.terminal {
            self.seal_failed(call, state.next)?;
        }
        Ok(())
    }

    /// Read at most one bounded, ordered committed page. The caller reserves
    /// its outer broker headers from `limit`. A too-small budget for the first
    /// frame returns an error, so an empty page means there is no committed tail.
    /// Drop the transaction before parking a reader. Cost: two indexed reads.
    pub fn read(&self, call: Uuid, after: i64, limit: usize) -> Result<Vec<Value>> {
        if !(0..=MAX_SEQ).contains(&after) {
            return Err(Error::input_field(
                "/after",
                "must be a contribution sequence",
            ));
        }
        self.call(call)?;
        let mut statement = self.tx.prepare_cached(
            "SELECT payload FROM contribution_frames WHERE call=?1 AND seq>?2 ORDER BY seq LIMIT ?3",
        ).map_err(internal)?;
        let mut rows = statement
            .query(params![call.to_string(), after, BATCH_FRAMES as i64])
            .map_err(internal)?;
        let mut frames = Vec::new();
        let too_large = || {
            Error::new(
                ErrorCode::TooLarge,
                "Contribution reply exceeds its envelope",
            )
        };
        let mut remaining = limit
            .min(MAX_FRAME_BYTES)
            .checked_sub(b"{\"frames\":[]}".len())
            .ok_or_else(too_large)?;
        while let Some(row) = rows.next().map_err(internal)? {
            let bytes: Vec<u8> = row.get(0).map_err(internal)?;
            let size = bytes.len() + usize::from(!frames.is_empty());
            if size > remaining {
                if frames.is_empty() {
                    return Err(too_large());
                }
                break;
            }
            remaining -= size;
            frames.push(serde_json::from_slice(&bytes).map_err(internal)?);
        }
        Ok(frames)
    }

    /// Resolve one manifest-selected string field from a committed output
    /// frame for the item writer's exact-attempt authorship check. The broker
    /// supplies the operation's checked opt-in and delivered sequence. No
    /// worker-supplied text participates. Cost: one scoped indexed read and
    /// one bounded decode.
    pub fn output_text(
        &self,
        call: Uuid,
        seq: i64,
        delivered_through: i64,
        field: &str,
    ) -> Result<Option<String>> {
        if seq <= 0 || seq > delivered_through {
            return Ok(None);
        }
        let bytes: Option<Vec<u8>> = self.tx.prepare_cached(
            "SELECT f.payload FROM contribution_frames f JOIN contribution_calls c ON c.call=f.call
             WHERE c.call=?1 AND f.seq=?2 AND c.execution=?3 AND c.generation=?4
               AND c.expires IS NULL AND f.producer=1",
        ).map_err(internal)?.query_row(
            params![call.to_string(), seq, self.execution.0, self.generation],
            |row| row.get(0),
        ).optional().map_err(internal)?;
        let Some(bytes) = bytes else {
            return Ok(None);
        };
        let frame: Value = serde_json::from_slice(&bytes).map_err(internal)?;
        Ok(frame
            .get("data")
            .and_then(|data| data.get(field))
            .and_then(Value::as_str)
            .map(str::to_owned))
    }

    fn call(&self, call: Uuid) -> Result<Call> {
        self.tx.prepare_cached(
            "SELECT next,allocated,claimed,terminal,rate_ms,byte_credit,frame_credit
             FROM contribution_calls WHERE call=?1 AND execution=?2 AND generation=?3 AND expires IS NULL",
        ).map_err(internal)?.query_row(params![call.to_string(), self.execution.0, self.generation],
            |row| Ok(Call {
                next: row.get(0)?,
                allocated: row.get(1)?,
                claimed: row.get(2)?,
                terminal: row.get(3)?,
                rate: Bucket { ms: row.get(4)?, bytes: row.get(5)?, frames: row.get(6)? },
            })
        ).optional().map_err(internal)?.ok_or_else(unavailable)
    }

    fn seal_failed(&self, call: Uuid, seq: i64) -> Result<()> {
        let terminal = json!({"seq":seq,"kind":"terminal","state":"failed"});
        self.store(
            call,
            seq,
            &serde_json::to_vec(&terminal).map_err(internal)?,
            true,
            None,
        )
    }

    fn store(
        &self,
        call: Uuid,
        seq: i64,
        payload: &[u8],
        terminal: bool,
        credit: Option<(Bucket, Bucket)>,
    ) -> Result<()> {
        let call = call.to_string();
        let change = payload.len() as i64 + 1 - if terminal { TERMINAL_RESERVE } else { 0 };
        let attempt_credit = credit.map(|(attempt, _)| attempt);
        let space_credit = credit.map(|(_, space)| space);
        self.tx
            .prepare_cached("INSERT INTO contribution_frames VALUES(?1,?2,?3,?4)")
            .map_err(internal)?
            .execute(params![call, seq, payload, credit.is_some()])
            .map_err(internal)?;
        self.tx
            .prepare_cached(
                "UPDATE contribution_calls SET next=next+1,allocated=allocated+?2,terminal=?3,
                 rate_ms=coalesce(?4,rate_ms),byte_credit=coalesce(?5,byte_credit),
                 frame_credit=coalesce(?6,frame_credit) WHERE call=?1",
            )
            .map_err(internal)?
            .execute(params![
                call,
                change,
                terminal,
                attempt_credit.map(|b| b.ms),
                attempt_credit.map(|b| b.bytes),
                attempt_credit.map(|b| b.frames),
            ])
            .map_err(internal)?;
        self.tx
            .prepare_cached(
                "UPDATE contribution_budget SET bytes=bytes+?1,rate_ms=coalesce(?2,rate_ms),
                 byte_credit=coalesce(?3,byte_credit),frame_credit=coalesce(?4,frame_credit)
             WHERE singleton=1",
            )
            .map_err(internal)?
            .execute(params![
                change,
                space_credit.map(|b| b.ms),
                space_credit.map(|b| b.bytes),
                space_credit.map(|b| b.frames),
            ])
            .map_err(internal)?;
        Ok(())
    }
}

struct Call {
    next: i64,
    allocated: i64,
    claimed: bool,
    terminal: bool,
    rate: Bucket,
}

struct Budget {
    bytes: i64,
    calls: i64,
    rate: Bucket,
}

fn budget(tx: &Transaction<'_>) -> Result<Budget> {
    tx.query_row(
        "SELECT bytes,calls,rate_ms,byte_credit,frame_credit FROM contribution_budget WHERE singleton=1",
        [],
        |row| Ok(Budget {
            bytes: row.get(0)?, calls: row.get(1)?,
            rate: Bucket { ms: row.get(2)?, bytes: row.get(3)?, frames: row.get(4)? },
        }),
    ).map_err(internal)
}

fn frame_shape(frame: &Value) -> Result<(i64, Option<&Value>)> {
    let fields = frame
        .as_object()
        .filter(|v| v.len() == 3)
        .ok_or_else(|| Error::input_field("/", "must be one contribution frame"))?;
    let seq = fields
        .get("seq")
        .and_then(Value::as_i64)
        .filter(|n| (1..=MAX_SEQ).contains(n))
        .ok_or_else(|| Error::input_field("/seq", "must be a contribution sequence"))?;
    match fields.get("kind").and_then(Value::as_str) {
        Some("output") if fields.contains_key("data") => Ok((seq, fields.get("data"))),
        Some("terminal")
            if matches!(
                fields.get("state").and_then(Value::as_str),
                Some("ok" | "failed" | "stopped")
            ) =>
        {
            Ok((seq, None))
        }
        _ => Err(Error::input_field(
            "/",
            "must be a closed contribution output or terminal",
        )),
    }
}

/// Start the spool's receipt retention at exact-attempt retirement. Slice 3
/// calls this only from its fenced completion or teardown transaction and
/// supplies the operator's receipt lifetime. Replays preserve expiry.
/// Cost: one indexed update, regardless of the call's frame count.
pub fn retire(
    tx: &Transaction<'_>,
    execution: &ExecutionRef,
    generation: i64,
    now: i64,
    retention: Duration,
) -> Result<()> {
    let expiry = now.saturating_add(i64::try_from(retention.as_millis()).unwrap_or(i64::MAX));
    tx.execute(
        "UPDATE contribution_calls SET expires=?3 WHERE execution=?1 AND generation=?2 AND expires IS NULL",
        params![execution.0, generation, expiry],
    ).map_err(internal)?;
    Ok(())
}

/// Retire at most eight expired call receipts and their frame records, and
/// release their budget in the same transaction. The indexed sweep bounds
/// admission work even with a full retained spool.
pub fn prune(tx: &Transaction<'_>, now: i64) -> Result<()> {
    let mut statement = tx
        .prepare_cached(
            "DELETE FROM contribution_calls WHERE call IN (
             SELECT call FROM contribution_calls WHERE expires<=?1 ORDER BY expires LIMIT 8
         ) RETURNING allocated",
        )
        .map_err(internal)?;
    let mut rows = statement.query([now]).map_err(internal)?;
    let mut count = 0_i64;
    let mut bytes = 0_i64;
    while let Some(row) = rows.next().map_err(internal)? {
        count += 1;
        bytes += row.get::<_, i64>(0).map_err(internal)?;
    }
    if count != 0 {
        tx.execute(
            "UPDATE contribution_budget SET calls=calls-?1,bytes=bytes-?2 WHERE singleton=1",
            params![count, bytes],
        )
        .map_err(internal)?;
    }
    Ok(())
}

fn unavailable() -> Error {
    Error::not_found("This contribution call is unavailable")
}

#[cfg(test)]
mod tests;
