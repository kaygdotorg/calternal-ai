//! Bounded manager diagnostics on each immutable package revision.
use super::activation;
use crate::{api::Shared, auth::Directory};
use calternal_core::{PluginDiagnostics, PluginInput, PluginReadiness, PluginRuntimeFailure};
use calternal_types::{AccountId, Capability, Error, SpaceId, now_ms};
use std::{collections::VecDeque, sync::Mutex};

const SPACES: usize = 64;
const ERRORS: usize = 8;
const RETAIN_MS: i64 = 60 * 60 * 1000;
use calternal_machine::sandbox::worker::STDERR_TAIL_BYTES as STDERR_BYTES;

/// Bounded telemetry keyed by the observed space inside one package revision.
#[derive(Debug, Default)]
pub(crate) struct RuntimeDiagnostics(Mutex<VecDeque<Observation>>);

struct Observation {
    space: SpaceId,
    ready: Option<(i64, Option<&'static str>)>,
    failures: VecDeque<PluginRuntimeFailure>,
    stderr: Option<(i64, String)>,
}

// Avoid rendering worker output through the catalogue's Debug implementation.
impl std::fmt::Debug for Observation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observation")
            .field("space", &self.space)
            .finish_non_exhaustive()
    }
}

impl RuntimeDiagnostics {
    fn update(&self, space: SpaceId, edit: impl FnOnce(&mut Observation)) {
        let mut rows = self.0.lock().unwrap_or_else(|error| error.into_inner());
        let mut row = rows
            .iter()
            .position(|row| row.space == space)
            .and_then(|index| rows.remove(index))
            .unwrap_or_else(|| Observation {
                space,
                ready: None,
                failures: VecDeque::new(),
                stderr: None,
            });
        edit(&mut row);
        if rows.len() >= SPACES {
            rows.pop_back();
        }
        rows.push_front(row);
    }

    /// Record one startup transition, without touching successful warm calls.
    pub(super) fn ready(&self, space: SpaceId) {
        self.update(space, |row| row.ready = Some((now_ms(), None)));
    }

    /// Record a stopped worker's state and stderr for its own space.
    /// Holds one short in-memory lock, with a bounded tail copy.
    pub(super) fn fault(&self, space: SpaceId, reason: &'static str, stderr: &[u8]) {
        self.update(space, |row| {
            let now = now_ms();
            row.ready = Some((now, Some(reason)));
            if !stderr.is_empty() {
                let text = String::from_utf8_lossy(stderr);
                let from = text.ceil_char_boundary(text.len().saturating_sub(STDERR_BYTES));
                let tail = text[from..]
                    .chars()
                    .filter(|&c| !c.is_control() || c == '\n' || c == '\t')
                    .collect();
                row.stderr = Some((now, tail));
            } else {
                row.stderr = None;
            }
        });
    }

    /// Append one bounded failure record. Never retain the request or error text.
    pub(crate) fn failure(&self, space: SpaceId, operation: &str, error: &Error) {
        let reason = reason(error);
        self.update(space, |row| {
            let now = now_ms();
            if row.ready.is_none() {
                row.ready = Some((now, Some(reason)));
            }
            row.failures
                .retain(|failure| failure.at_ms > now - RETAIN_MS);
            if row.failures.len() >= ERRORS {
                row.failures.pop_back();
            }
            row.failures.push_front(PluginRuntimeFailure {
                operation: Some(operation.to_owned()),
                reason: reason.into(),
                at_ms: now,
            });
        });
    }
}

fn reason(error: &Error) -> &'static str {
    match error
        .details
        .as_ref()
        .and_then(|body| body["reason"].as_str())
    {
        Some("input_schema") => "input_schema",
        Some("output_schema") => "output_schema",
        Some("view_schema") => "view_schema",
        Some("worker_busy") => "worker_busy",
        Some("worker_machine_required") => "worker_machine_required",
        Some("worker_capacity") => "worker_capacity",
        _ => match error.message.as_str() {
            "extension worker stopped" => "worker_stopped",
            "extension timed out" => "worker_timeout",
            _ => "worker_failed",
        },
    }
}

/// Read one package revision's bounded telemetry after current capability and
/// activation checks. Cost: a catalog snapshot, activation read and short copy.
pub(crate) fn read<D: Directory>(
    state: &Shared<D>,
    account: AccountId,
    input: PluginInput,
) -> Result<PluginDiagnostics, Error> {
    let space =
        crate::requests::authorize(state, account, &input.space_id, Capability::PluginsManage)?;
    let catalog = state.registry.snapshot();
    let plugin = catalog
        .get(&input.plugin_id)
        .filter(|p| p.available_in(space))
        .ok_or_else(|| Error::not_found("plugin not found"))?;
    let enabled = activation(&state.store, space, plugin)?.enabled;
    let cutoff = now_ms() - RETAIN_MS;
    let rows = plugin
        .installed_diagnostics
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let row = rows.iter().find(|row| row.space == space);
    let last = row.and_then(|row| row.ready).filter(|(at, _)| *at > cutoff);
    let (readiness, reason) = if plugin.unavailable.is_some() {
        (PluginReadiness::Unavailable, Some("bundle_unavailable"))
    } else if !enabled {
        (PluginReadiness::Disabled, None)
    } else if plugin.entry.worker.is_none() {
        (PluginReadiness::Ready, None)
    } else {
        match last {
            Some((_, Some(reason))) => (PluginReadiness::Failed, Some(reason)),
            Some((_, None)) => (PluginReadiness::Ready, None),
            None => (PluginReadiness::NotStarted, None),
        }
    };
    let result = PluginDiagnostics {
        space_id: space.to_string(),
        plugin_id: input.plugin_id,
        revision: plugin.revision,
        readiness,
        reason: reason.map(str::to_owned),
        recent_errors: row
            .into_iter()
            .flat_map(|row| &row.failures)
            .filter(|failure| failure.at_ms > cutoff)
            .cloned()
            .collect(),
        stderr_tail: row
            .and_then(|row| row.stderr.as_ref())
            .filter(|(at, _)| *at > cutoff)
            .map(|(_, text)| text.clone()),
    };
    drop(rows);
    crate::requests::authorize(state, account, &input.space_id, Capability::PluginsManage)?;
    Ok(result)
}
