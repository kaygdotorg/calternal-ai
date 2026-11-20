//! Exact Stop context behind the renderer's checked activation and confirmation.
use super::update::UpdateGate;
use super::*;
use calternal_content::execution::ArchiveTarget;

const STOP: &str = "host.execution.stop";

/// Host-resolved scope of a member's live item-view component. The renderer
/// route obtains these values from its checked view lease and sign-in.
#[derive(Clone, PartialEq, Eq)]
pub struct ControlScope {
    /// Space of the checked item view.
    pub space: SpaceId,
    /// Member requesting the control.
    pub actor: AccountId,
    /// Installation authenticated on the renderer view.
    pub installation: InstallationId,
    /// Active lease that owns the checked item-view component.
    pub lease: LeaseRef,
    /// Exact component within the lease that owns the stop controls.
    pub component: String,
    /// Item bound to that component.
    pub item: ItemRef,
}

/// A host-minted control target (Stop, or steer for a running attempt). Keep
/// this with the opaque activation context. There is no renderer or broker
/// deserializer for its target.
pub struct ControlContext {
    pub(super) scope: ControlScope,
    pub(super) target: ArchiveTarget,
}

/// Resolve the eligible controls without waking a plugin. The caller supplies
/// live component and item authorization and whether this member can manage
/// other starters' executions. Call from a blocking worker. Cost: one indexed
/// item lookup for at most two targets and no flushes.
pub fn stop_controls(
    content: &Content,
    gate: &UpdateGate,
    scope: &ControlScope,
    authorize: impl FnOnce() -> Result<bool>,
) -> Result<Vec<ControlContext>> {
    let space = scope.space;
    content.execution_transaction(space, &|n| gate.observe(space, n), |tx| {
        let manages = authorize()?;
        Ok(execution::archive_targets(tx, &scope.item)?
            .into_iter()
            .filter(|target| target.starter == scope.actor || manages)
            .map(|target| ControlContext {
                scope: scope.clone(),
                target,
            })
            .collect())
    })
}

impl ControlContext {
    pub(super) fn require_scope(&self, current: &ControlScope) -> Result<()> {
        if &self.scope == current {
            Ok(())
        } else {
            Err(unavailable())
        }
    }

    /// Phase for the renderer control. The first gate supports queued and
    /// starting attempts. No I/O and no exposure of underlying IDs.
    pub fn phase(&self) -> &'static str {
        if self.target.queued {
            "queued"
        } else {
            "starting"
        }
    }

    /// Revalidate the exact candidate and its retirement authority during
    /// activation resolution, before the host issues confirmation material.
    /// Call from a blocking worker. Cost: at most two loaded targets, no flush.
    pub fn resolve(
        &self,
        content: &Content,
        gate: &UpdateGate,
        current: &ControlScope,
        authorize: impl FnOnce() -> Result<bool>,
    ) -> Result<()> {
        self.require_scope(current)?;
        let space = current.space;
        content.execution_transaction(space, &|n| gate.observe(space, n), |tx| {
            require_retirement(current.actor, self.target.starter, authorize()?)?;
            if execution::archive_targets(tx, &current.item)?.contains(&self.target) {
                Ok(())
            } else {
                Err(unavailable())
            }
        })
    }

    /// Bind confirmed dispatch to this context after the renderer activation
    /// service verifies the live proof for the pinned resolution. The callback
    /// must reject reused proofs for another activation or context. No I/O
    /// beyond the proof check.
    pub fn confirm(
        self,
        current: &ControlScope,
        request: RequestId,
        verify_proof: impl FnOnce(&RequestId) -> Result<()>,
    ) -> Result<ConfirmedStop> {
        self.require_scope(current)?;
        verify_proof(&request)?;
        Ok(ConfirmedStop {
            context: self,
            request,
        })
    }
}

/// A Stop authorized by the protected activation confirmation service.
pub struct ConfirmedStop {
    context: ControlContext,
    request: RequestId,
}

impl ConfirmedStop {
    /// Execute or replay the confirmed Stop. The activation service supplies
    /// a live lease, item and retirement check.
    /// Only the pinned intent or attempt is retired. The same writer commits
    /// the receipt, the fence and the exact Stop delivery for the old attempt,
    /// which the post-commit dispatcher (or the next reconnect) sends. A
    /// delayed duplicate replays the receipt and never reaches a successor.
    /// Call from a blocking worker. Cost: one indexed target lookup plus the
    /// usual content receipt path for a fresh effect.
    pub fn dispatch(
        &self,
        content: &Content,
        gate: &UpdateGate,
        current: &ControlScope,
        authorize: impl FnOnce() -> Result<bool>,
    ) -> Result<Value> {
        self.context.require_scope(current)?;
        let target = &self.context.target;
        let mut scoped = JsonObject::from([("item".to_owned(), json!(current.item))]);
        if let Some(attempt) = &target.attempt {
            scoped.insert("execution".to_owned(), json!(attempt.execution));
            scoped.insert("generation".to_owned(), json!(attempt.generation));
        } else {
            scoped.insert("intent".to_owned(), json!(target.intent));
        }
        let fingerprint = crate::api::fingerprint(STOP, &scoped);
        let space = current.space;
        content.execution_command(
            space,
            &|n| gate.observe(space, n),
            Request {
                account: current.actor,
                via: None,
                id: &self.request.0,
                operation: STOP,
                fingerprint: &fingerprint,
            },
            || require_retirement(current.actor, target.starter, authorize()?),
            |tx| {
                let exact = |item: &ItemRef, starter| {
                    if item == &current.item && starter == target.starter {
                        Ok(())
                    } else {
                        Err(unavailable())
                    }
                };
                if let Some(attempt) = &target.attempt {
                    let cursor = execution::stop(tx, attempt, &self.request.0, exact)?;
                    Ok(json!({
                        "execution": attempt.execution,
                        "phase": "stopped",
                        "transition_cursor": cursor,
                    }))
                } else {
                    execution::stop_pending(tx, &target.intent, exact)?;
                    Ok(json!({"intent": target.intent, "phase": "stopped"}))
                }
            },
        )
    }
}

fn require_retirement(actor: AccountId, starter: AccountId, manages: bool) -> Result<()> {
    if actor == starter || manages {
        Ok(())
    } else {
        Err(unavailable())
    }
}

const STEER: &str = "host.execution.steer";

impl ControlContext {
    /// Append a steer to the exact running attempt of this context and commit
    /// its receipt and delivery before replying. Only the attempt's starter
    /// may steer it. A queued intent, a retired attempt, a full ledger or an
    /// oversized delivery is refused. Returns `{execution, seq}`. Call from a
    /// blocking worker. Cost: the receipt path plus the ledger insert.
    pub fn steer(
        &self,
        content: &Content,
        gate: &UpdateGate,
        current: &ControlScope,
        request: &RequestId,
        text: &str,
        authorize: impl FnOnce() -> Result<()>,
    ) -> Result<Value> {
        self.require_scope(current)?;
        let attempt = self.target.attempt.as_ref().ok_or_else(unavailable)?;
        if current.actor != self.target.starter {
            return Err(unavailable());
        }
        let operands = JsonObject::from([
            ("execution".to_owned(), json!(attempt.execution)),
            ("generation".to_owned(), json!(attempt.generation)),
            ("text".to_owned(), json!(text)),
        ]);
        let fingerprint = crate::api::fingerprint(STEER, &operands);
        let space = current.space;
        content.execution_command(
            space,
            &|n| gate.observe(space, n),
            Request {
                account: current.actor,
                via: None,
                id: &request.0,
                operation: STEER,
                fingerprint: &fingerprint,
            },
            authorize,
            |tx| {
                let seq = execution::steer(tx, attempt, &request.0, text)?;
                Ok(json!({"execution": attempt.execution, "seq": seq}))
            },
        )
    }
}
