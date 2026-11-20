//! The consumer side of a call's bounded relay window: reading the committed
//! suffix, consuming a prefix, and reporting the durable cursor and credit.
use super::{
    BATCH_FRAMES, Call, MAX_SEQ, SPACE_BYTES, Spool, TERMINAL_RESERVE, budget, unavailable,
};
use crate::{Result, types::internal};
use calternal_types::{Error, ErrorCode, MAX_FRAME_BYTES, contribution_limits};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use uuid::Uuid;

/// Where one call stands, for the carrier that relays its machine log.
#[derive(Debug, PartialEq, Eq)]
pub struct Progress {
    /// Highest contiguous producer sequence durably committed here. The
    /// carrier sends it as the resume position and acknowledges through it
    /// only after the committing transaction has returned.
    pub committed: i64,
    /// Highest sequence the owning job has consumed. Its payload is retired.
    pub consumed: i64,
    /// A terminal frame is committed; no further producer frame is accepted.
    pub terminal: bool,
    /// Encoded bytes of output the relay may still submit: the lesser of the
    /// call's and the space's free window. Zero means pause relay.
    pub credit_bytes: i64,
}

impl Spool<'_, '_> {
    /// Report the committed cursor, consumed cursor and window credit.
    /// Call it after reattachment (the resume position) and after every
    /// [`Spool::consume`] (the credit to return). Cost: two indexed reads.
    pub fn progress(&self, call: Uuid) -> Result<Progress> {
        let Call {
            next,
            allocated,
            terminal,
            consumed,
            ..
        } = self.call(call)?;
        let space = budget(self.tx)?;
        let limits = contribution_limits();
        Ok(Progress {
            committed: next - 1,
            consumed,
            terminal,
            credit_bytes: (limits.encoded_spool_bytes - allocated)
                .min(SPACE_BYTES - space.bytes)
                .max(0),
        })
    }

    /// Mark the committed prefix through `through` as consumed by the owning
    /// job. Its payloads are retired and their bytes return to the call's and
    /// the space's window; each frame keeps a 32-byte replay digest, so a
    /// repeated frame still answers `Replay` and a changed one is refused.
    /// Call it in the transaction that records the job's own checkpoint, after
    /// any attested write that resolves output text from these frames. A
    /// cursor at or behind the consumed one is a no-op, so retries are safe.
    /// Cost: one range read and update over the retired frames (at most the
    /// window), plus two counter updates.
    pub fn consume(&self, call: Uuid, through: i64) -> Result<()> {
        let state = self.call(call)?;
        if through <= state.consumed {
            return Ok(());
        }
        if through >= state.next {
            return Err(Error::conflict("Cannot consume uncommitted output"));
        }
        let (id, floor) = (call.to_string(), state.consumed);
        let freed: i64 = self
            .tx
            .prepare_cached(
                "SELECT coalesce(sum(length(payload)+1),0) FROM contribution_frames
                 WHERE call=?1 AND seq>?2 AND seq<=?3",
            )
            .map_err(internal)?
            .query_row(params![id, floor, through], |row| row.get(0))
            .map_err(internal)?;
        self.tx
            .prepare_cached(
                "UPDATE contribution_frames SET payload=NULL WHERE call=?1 AND seq>?2 AND seq<=?3",
            )
            .map_err(internal)?
            .execute(params![id, floor, through])
            .map_err(internal)?;
        self.tx
            .prepare_cached(
                "UPDATE contribution_calls SET consumed=?2,allocated=allocated-?3 WHERE call=?1",
            )
            .map_err(internal)?
            .execute(params![id, through, freed])
            .map_err(internal)?;
        self.tx
            .execute(
                "UPDATE contribution_budget SET bytes=bytes-?1 WHERE singleton=1",
                [freed],
            )
            .map_err(internal)?;
        debug_assert!(freed >= 0 && TERMINAL_RESERVE > 0);
        Ok(())
    }

    /// Read at most one bounded, ordered committed page. The caller reserves
    /// its outer broker headers from `limit`. A too-small budget for the first
    /// frame returns an error, so an empty page means there is no committed tail.
    /// A position behind the consumed cursor is refused: those payloads are
    /// retired. Drop the transaction before parking a reader. Cost: two indexed reads.
    pub fn read(&self, call: Uuid, after: i64, limit: usize) -> Result<Vec<Value>> {
        if !(0..=MAX_SEQ).contains(&after) {
            return Err(Error::input_field(
                "/after",
                "must be a contribution sequence",
            ));
        }
        if after < self.call(call)?.consumed {
            return Err(Error::conflict(
                "Contribution output before the consumed position is retired",
            ));
        }
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
    /// worker-supplied text participates. A consumed frame resolves nothing,
    /// so the write must precede [`Spool::consume`]. Cost: one scoped indexed
    /// read and one bounded decode.
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
               AND c.expires IS NULL AND f.producer=1 AND f.payload IS NOT NULL",
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
}
