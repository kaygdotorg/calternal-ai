//! Observable turn phases and compact per-attempt timing on the existing checkpoint.
use crate::plugins::{jobs::Job, live::Live};
use calternal_core::ProviderEventKind;
use calternal_types::{Error, now_ms};
use serde_json::{Value, json};
use std::time::Instant;

/// Emit the already admitted phase without another job or content write.
pub(crate) fn event(phase: &str, at_ms: i64) -> Value {
    json!({"type":"turn_phase", "phase":phase, "at_ms":at_ms})
}

/// One attempt's observed phase and cumulative timing. Uses the host monotonic
/// clock for phase durations and stores only integer totals at checkpoints.
pub(super) struct Phases {
    phase: &'static str,
    since: Instant,
    totals: [u64; 4],
    reasoning_ms: u64,
    thinking: bool,
    activity: bool,
}

impl Phases {
    /// Publish the initial queue phase. Admission's timestamp includes time
    /// spent before the runner claimed the first attempt. No I/O.
    pub(super) fn queued(job: &Job, live: &Live) -> Self {
        let now = now_ms();
        let at = if job.runs == 1 {
            job.input["thread_request"]["created_ms"]
                .as_i64()
                .unwrap_or(now)
        } else {
            now
        };
        live.publish(job.runs, &event("queued", at));
        Self {
            phase: "queued",
            since: Instant::now(),
            totals: [now.saturating_sub(at).max(0) as u64, 0, 0, 0],
            reasoning_ms: 0,
            thinking: false,
            activity: false,
        }
    }

    /// Switch display phases only on a change. Retain the current event in the
    /// existing checkpoint so a live stream reset can recover its timestamp.
    pub(super) fn switch(
        &mut self,
        phase: &'static str,
        job: &Job,
        live: &Live,
        checkpoint: &mut Value,
    ) {
        if self.phase != phase {
            self.record();
            self.phase = phase;
            let update = event(phase, now_ms());
            live.publish(job.runs, &update);
            checkpoint["turn_phase"] = update;
        }
    }

    /// Derive the phase from ordered events and pending tool state. Reject an
    /// unpaired reasoning block. Cost: constant work per frame, no I/O.
    pub(super) fn observe(
        &mut self,
        provider: &ProviderEventKind,
        tools_pending: bool,
        job: &Job,
        live: &Live,
        checkpoint: &mut Value,
    ) -> Result<(), Error> {
        match provider {
            ProviderEventKind::ReasoningStarted if !self.thinking => self.thinking = true,
            ProviderEventKind::ReasoningDelta { .. } if self.thinking => {}
            ProviderEventKind::ReasoningFinished { duration_ms } if self.thinking => {
                self.thinking = false;
                self.reasoning_ms = self.reasoning_ms.saturating_add(*duration_ms).min(600_000);
            }
            ProviderEventKind::ReasoningStarted
            | ProviderEventKind::ReasoningDelta { .. }
            | ProviderEventKind::ReasoningFinished { .. } => {
                return Err(Error::invalid_input("unpaired provider reasoning event"));
            }
            ProviderEventKind::Check(_) | ProviderEventKind::Done { .. } => return Ok(()),
            _ => {}
        }
        let phase = if self.thinking {
            "thinking"
        } else if tools_pending {
            "running_tool"
        } else {
            "running"
        };
        self.activity |= matches!(
            provider,
            ProviderEventKind::TextDelta { .. }
                | ProviderEventKind::ReasoningStarted
                | ProviderEventKind::ToolStarted { .. }
        );
        self.switch(phase, job, live, checkpoint);
        Ok(())
    }

    /// Fold one compact timing marker into the existing terminal output write.
    /// Turns that never emitted assistant activity leave no empty message.
    pub(super) fn finish(&mut self, checkpoint: &mut Value) {
        if !self.activity {
            return;
        }
        self.activity = false;
        self.record();
        let [queued, sandbox, running, tools] = self.totals;
        let duration = sandbox + running + tools;
        let text = checkpoint["thread_text"].as_str().unwrap_or("");
        checkpoint["thread_text"] = json!(format!(
            "{text}\n\n<!-- turn-timing duration-ms:{duration} reasoning-ms:{} queued-ms:{queued} sandbox-ms:{sandbox} tool-ms:{tools} -->\n",
            self.reasoning_ms,
        ));
    }

    fn record(&mut self) {
        let elapsed = self.since.elapsed().as_millis() as u64;
        self.since = Instant::now();
        let index = match self.phase {
            "queued" => 0,
            "starting_sandbox" => 1,
            "running_tool" => 3,
            _ => 2,
        };
        self.totals[index] = self.totals[index].saturating_add(elapsed);
    }
}
