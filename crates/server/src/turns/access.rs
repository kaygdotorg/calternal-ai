//! Per-thread access policy. Authority is stored in auth.db through its protected recovery barrier.
use calternal_core::{AgentMode, JsonObject};
use calternal_types::Error;
use serde::{Deserialize, Serialize};

/// A token's current, server-owned scope. Never supplied by an adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authority {
    /// Shared attribution. Access comes only from its account; via must name
    /// this agent's thread and never grants authority.
    pub actor: calternal_types::Actor,
    /// Only space this token can address.
    pub space: String,
    /// Only turn whose controls this token may call.
    pub job: String,
    /// Current permission mode, changed only by a person.
    pub mode: AgentMode,
    /// Operation prefix of the originating app.
    pub source_app: String,
    /// Approved additional app prefixes and whether writes are permitted.
    #[serde(skip)]
    pub grants: Vec<Grant>,
}

/// A person-approved app scope, optionally bound to one exact action digest.
/// App grants cover ordinary reads/reversible changes; sensitive effects in
/// approval modes require the action digest. Thread grants are retained until the thread is archived or deleted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// Operation prefix selected by the trusted person or denial request.
    pub app: String,
    /// Whether mutations are included in the grant.
    pub write: bool,
    /// Exact operation/input digest, with retry identity where required.
    pub action_hash: Option<String>,
}

/// One operation prefix, shared by source validation and both grant forms.
pub(crate) fn app(op: &str) -> &str {
    op.split_once('.').map_or(op, |(prefix, _)| prefix)
}

/// Automatic reviewer seam, restricted to irreversible effects. It cannot widen
/// token scope or override external/publication or access-change approval policy.
pub trait Reviewer: Send + Sync {
    /// Decide whether an irreversible action may proceed within existing scope.
    fn allows_irreversible(&self, operation: &str, input: &JsonObject) -> bool;
}

/// Rules-only reviewer: irreversible actions need a person's approval.
pub struct RulesOnly;
impl Reviewer for RulesOnly {
    fn allows_irreversible(&self, _: &str, _: &JsonObject) -> bool {
        false
    }
}

/// Evaluate the permission/risk matrix once token and person capabilities hold.
/// `approved_scope` is a person-approved scope; access widening always asks anew.
/// No I/O or allocation. Reviewer advice applies only to irreversible operations.
pub fn risk_allowed(
    mode: AgentMode,
    risk: calternal_core::OperationRisk,
    in_default_scope: bool,
    approved_scope: bool,
    reviewer: &dyn Reviewer,
    op: &str,
    input: &JsonObject,
) -> bool {
    use calternal_core::OperationRisk::*;
    if risk == WidensAccess {
        return false;
    }
    if mode == AgentMode::FullAccess {
        return true;
    }
    match risk {
        Irreversible => {
            approved_scope
                || (mode == AgentMode::ApproveForMe && reviewer.allows_irreversible(op, input))
        }
        External => approved_scope,
        Read | Reversible => mode != AgentMode::LeastAccess || in_default_scope || approved_scope,
        WidensAccess => false,
    }
}

impl Authority {
    /// The agent thread in the shared attribution. Invalid stored attribution
    /// fails closed; no I/O or allocation.
    pub fn thread(&self) -> Result<uuid::Uuid, Error> {
        match self.actor.via {
            Some(calternal_types::Via::Agent { thread }) => Ok(thread),
            _ => Err(Error::internal("agent thread attribution missing")),
        }
    }
    /// A once grant cannot bypass ownership of thread targets or their space.
    pub(crate) fn can_request_action(&self, op: &str, input: &JsonObject) -> bool {
        !op.starts_with("agent.")
            && !op.starts_with("access.")
            && input.get("space_id").and_then(serde_json::Value::as_str)
                == Some(self.space.as_str())
            && (!op.starts_with("threads.")
                || input
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(|id| {
                        self.thread()
                            .map(|thread| thread.to_string() == id)
                            .unwrap_or(false)
                    }))
    }

    /// Enforce current token scope before a handler or receipt replay. Catalog and
    /// validators are cached once; each call scans only the thread's bounded grants.
    pub fn check(&self, op: &str, input: &JsonObject) -> Result<(), Error> {
        use std::{collections::HashMap, sync::OnceLock};
        static CATALOG: OnceLock<HashMap<String, (bool, calternal_core::OperationRisk)>> =
            OnceLock::new();
        let catalog = CATALOG.get_or_init(|| {
            calternal_core::operation_catalog()
                .into_iter()
                .map(|d| (d.id, (d.mutates, d.risk)))
                .collect()
        });
        let Some((mutates, risk)) = catalog.get(op) else {
            return Err(Error::forbidden(
                "agent operation has no declared risk class",
            ));
        };
        self.check_operation(op, input, "", *mutates, *risk)
    }

    /// Apply scope and declared risk to a built-in or manifest operation. Approved
    /// scopes bind canonical operation input. Irreversible and widening scopes
    /// also bind the request ID, so a new action asks again.
    pub fn check_operation(
        &self,
        op: &str,
        input: &JsonObject,
        rid: &str,
        mutates: bool,
        risk: calternal_core::OperationRisk,
    ) -> Result<(), Error> {
        let own_turn = input.get("job_id").and_then(|v| v.as_str()) == Some(self.job.as_str());
        if matches!(op, "access.request" | "requests.ask") && own_turn {
            return Ok(());
        }
        if (op.starts_with("requests.") || op == "space.instructions") && op != "requests.ask" {
            return Err(Error::forbidden("a person must call this operation"));
        }
        if op.starts_with("agent.") {
            return if own_turn && matches!(op, "agent.events" | "agent.stop") {
                Ok(())
            } else {
                Err(Error::forbidden("agent cannot change thread authority"))
            };
        }
        if op.starts_with("threads.")
            && input
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| self.thread().map(|t| t.to_string() != id).unwrap_or(true))
        {
            return Err(Error::not_found("item not found"));
        }
        if input.get("space_id").and_then(|v| v.as_str()) != Some(self.space.as_str()) {
            return Err(Error::not_found("space not found"));
        }
        let app = app(op);
        let own_thread = op.starts_with("threads.")
            && input
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| self.thread().map(|t| t.to_string() == id).unwrap_or(false));
        let default = own_thread || (!self.source_app.is_empty() && app == self.source_app);
        let action = action_scope(op, input, rid, risk);
        let matching = self
            .grants
            .iter()
            .filter(|g| g.app == app && (!mutates || g.write));
        let app_grant = matching.clone().any(|g| g.action_hash.is_none());
        let exact_grant = matching
            .into_iter()
            .any(|g| g.action_hash.as_deref() == Some(action.as_str()));
        let approved = match risk {
            calternal_core::OperationRisk::Irreversible
            | calternal_core::OperationRisk::External
            | calternal_core::OperationRisk::WidensAccess => exact_grant,
            _ => app_grant || exact_grant,
        };
        let allowed = exact_grant && risk == calternal_core::OperationRisk::WidensAccess
            || risk_allowed(self.mode, risk, default, approved, &RulesOnly, op, input);
        if allowed {
            Ok(())
        } else {
            Err(Error::forbidden(
                "agent access requires a person approval; use access.request",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calternal_core::OperationRisk::*;
    #[test]
    fn reviewer_only_allows_irreversible_in_approve_for_me() {
        struct Allow;
        impl Reviewer for Allow {
            fn allows_irreversible(&self, _: &str, _: &JsonObject) -> bool {
                true
            }
        }
        for mode in [AgentMode::LeastAccess, AgentMode::ApproveForMe] {
            for risk in [Irreversible, External, WidensAccess] {
                assert_eq!(
                    risk_allowed(mode, risk, true, false, &Allow, "test", &JsonObject::new()),
                    mode == AgentMode::ApproveForMe && risk == Irreversible,
                );
            }
        }
    }
    #[test]
    fn permission_risk_matrix() {
        for risk in [Read, Reversible, Irreversible, External, WidensAccess] {
            let check = |mode, default, approved| {
                risk_allowed(
                    mode,
                    risk,
                    default,
                    approved,
                    &RulesOnly,
                    "test",
                    &JsonObject::new(),
                )
            };
            assert_eq!(
                check(AgentMode::FullAccess, false, false),
                risk != WidensAccess
            );
            assert_eq!(
                check(AgentMode::ApproveForMe, false, false),
                matches!(risk, Read | Reversible)
            );
            assert_eq!(
                check(AgentMode::ApproveForMe, false, true),
                risk != WidensAccess
            );
            assert!(!check(AgentMode::LeastAccess, false, false));
            assert_eq!(
                check(AgentMode::LeastAccess, true, false),
                matches!(risk, Read | Reversible)
            );
            assert_eq!(
                check(AgentMode::LeastAccess, false, true),
                risk != WidensAccess
            );
        }
    }
}

/// Stable conservative approval scope, derived from the requested operation and
/// canonical input. Reversible app grants and sensitive action grants stay separate.
/// Cost: one canonical serialization and shared SHA-256 helper.
pub fn action_scope(
    op: &str,
    input: &JsonObject,
    rid: &str,
    risk: calternal_core::OperationRisk,
) -> String {
    let retry = if matches!(
        risk,
        calternal_core::OperationRisk::Irreversible | calternal_core::OperationRisk::WidensAccess
    ) {
        rid
    } else {
        ""
    };
    crate::store::sha256_hex(
        format!(
            "{op}\n{retry}\n{}",
            calternal_core::canonical_request_input(input)
        )
        .as_bytes(),
    )
}
