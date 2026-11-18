//! Bounded local draft telemetry. Capture only codes, safe schema pointers and timings.
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
    time::Duration,
};

const MAX_SCOPES: usize = 64;
const MAX_ERRORS: usize = 5;
const MAX_FAILED: usize = 16;
const MAX_MESSAGE: usize = 300;

#[derive(Debug, Default)]
pub(super) struct DraftDiagnostics {
    recent: Mutex<VecDeque<OperationErrors>>,
    ready: OnceLock<Value>,
}

#[derive(Debug)]
struct OperationErrors {
    account: AccountId,
    operation: String,
    entries: VecDeque<Value>,
}

impl DraftDiagnostics {
    /// Keep timings once per revision for author inspection. No I/O.
    pub(super) fn ready(&self, validation: Duration, handshake: Duration, total: Duration) {
        let _ = self.ready.set(json!({
            "validation_ms": validation.as_millis(),
            "handshake_ms": handshake.as_millis(),
            "publication_ms": total.saturating_sub(validation + handshake).as_millis(),
            "total_ms": total.as_millis(),
        }));
    }

    /// Keep five recent failures per caller and operation, for up to 64 active
    /// caller/operation pairs in this draft revision. The lock spans bounded
    /// in-memory work on the error path only.
    pub(super) fn record(
        &self,
        account: AccountId,
        operation: &str,
        error: &Error,
        elapsed: Duration,
    ) {
        let details = error.details.as_ref();
        let reason = match details.and_then(|d| d["reason"].as_str()) {
            Some(
                reason @ ("input_schema"
                | "output_schema"
                | "worker_busy"
                | "worker_machine_required"
                | "worker_capacity"
                | "view_schema"),
            ) => reason,
            _ => "worker_failed",
        };
        let path = if matches!(reason, "input_schema" | "output_schema" | "view_schema") {
            details
                .and_then(|d| d["path"].as_str())
                .map(|p| p.chars().take(160).collect::<String>())
        } else {
            None
        };
        let entry =
            json!({"code":error.code,"reason":reason,"path":path,"elapsed_ms":elapsed.as_millis()});
        let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        let mut scope = recent
            .iter()
            .position(|scope| scope.account == account && scope.operation == operation)
            .and_then(|index| recent.remove(index))
            .unwrap_or_else(|| OperationErrors {
                account,
                operation: operation.into(),
                entries: VecDeque::new(),
            });
        if scope.entries.len() >= MAX_ERRORS {
            scope.entries.pop_back();
        }
        scope.entries.push_front(entry);
        if recent.len() >= MAX_SCOPES {
            recent.pop_back();
        }
        recent.push_front(scope);
    }

    /// Copy one bounded operation snapshot for its own caller after authorization.
    pub(super) fn snapshot(&self, account: AccountId, operation: &str, revision: u32) -> Value {
        let recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
        let errors = recent
            .iter()
            .find(|scope| scope.account == account && scope.operation == operation)
            .map(|scope| scope.entries.clone())
            .unwrap_or_default();
        json!({"phase":"ready","ready_revision":revision,"timing":self.ready.get(),"errors":errors})
    }
}

impl super::Plugin {
    /// Record a failed draft operation only for the caller that ran it. The
    /// draft is single-space. Costs bounded in-memory work on failure only.
    #[cfg(feature = "dev-plugin-drafts")]
    pub(crate) fn record_draft_error(
        &self,
        space: SpaceId,
        account: calternal_types::AccountId,
        operation: &str,
        error: &Error,
        elapsed: std::time::Duration,
    ) {
        if self.draft_space == Some(space) {
            self.diagnostics.record(account, operation, error, elapsed);
        }
    }
}

/// The newest failed candidate per (space, package), for development inspection.
/// In memory only: at most [`MAX_FAILED`] entries, each message cut to
/// [`MAX_MESSAGE`] characters, cleared by the next success and by a restart.
#[derive(Debug, Default)]
pub(crate) struct FailedCandidates(Mutex<VecDeque<Failed>>);

#[derive(Debug)]
struct Failed {
    space: SpaceId,
    id: String,
    value: Value,
}

impl FailedCandidates {
    /// Remember why a candidate stopped: `phase` is validation, handshake or
    /// publication. Replaces the package's earlier entry. Bounded memory only.
    pub(crate) fn record(
        &self,
        space: SpaceId,
        id: &str,
        phase: &str,
        message: &str,
        elapsed: Duration,
    ) {
        let value = json!({
            "phase": phase,
            "message": message.chars().take(MAX_MESSAGE).collect::<String>(),
            "elapsed_ms": elapsed.as_millis(),
        });
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|f| !(f.space == space && f.id == id));
        if all.len() >= MAX_FAILED {
            all.pop_back();
        }
        all.push_front(Failed {
            space,
            id: id.into(),
            value,
        });
    }

    /// Forget a package's failed candidate after a successful publication.
    pub(crate) fn clear(&self, space: SpaceId, id: &str) {
        let mut all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|f| !(f.space == space && f.id == id));
    }

    /// The package's last failed candidate in this space, if any.
    pub(crate) fn get(&self, space: SpaceId, id: &str) -> Option<Value> {
        let all = self.0.lock().unwrap_or_else(|e| e.into_inner());
        all.iter()
            .find(|f| f.space == space && f.id == id)
            .map(|f| f.value.clone())
    }
}
