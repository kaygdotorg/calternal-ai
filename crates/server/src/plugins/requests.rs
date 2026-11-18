//! Bounded requests and grant transitions through the protected auth barrier.
use super::{
    Jobs,
    job_sql::{db_err, now_ms},
    jobs::bounded_json,
};
use crate::turns::access::Authority;
use calternal_core::{ApprovalLifetime, JsonObject, OperationRisk, PersonRequest, RequestDetail};
use calternal_types::{AccountId, Error};
use rusqlite::{OptionalExtension, params};
use serde_json::Value;
use uuid::Uuid;

const THREAD_LIMIT: i64 = 32;
const ACCOUNT_LIMIT: i64 = 256;
const DETAIL_LIMIT: usize = 16 * 1024;

// App-level grants cover reads and, when requested, reversible writes. Exact
// action requests carry their declared risk in the protected action payload.
fn approval_lifetimes(
    risk: Option<&str>,
    write: bool,
) -> Result<&'static [ApprovalLifetime], Error> {
    let risk = match risk {
        Some(risk) => serde_json::from_value(Value::String(risk.into()))
            .map_err(|_| Error::internal("stored approval risk"))?,
        None if write => OperationRisk::Reversible,
        None => OperationRisk::Read,
    };
    Ok(risk.approval_lifetimes())
}

/// Close pending prompts under the caller's existing protected transaction.
pub(super) fn cancel_pending_requests(
    tx: &rusqlite::Transaction<'_>,
    space: &str,
    job: &str,
) -> Result<(), Error> {
    tx.execute(
        "UPDATE agent_approvals SET status='cancelled',resolved_ms=?3
         WHERE space=?1 AND job=?2 AND status='pending'",
        params![space, job, now_ms()],
    )
    .map_err(db_err)?;
    Ok(())
}

#[derive(Clone, Copy)]
pub(crate) enum OnceAdmission {
    Fresh,
    Replay,
    Unapproved,
}

impl Jobs {
    /// Insert or reuse a request for the issuing thread. A protected transaction
    /// verifies that the issuing turn still owns its scope and enforces both quotas.
    pub(crate) fn agent_wait_request(
        &self,
        a: &Authority,
        kind: &'static str,
        app: &str,
        write: bool,
        action_hash: Option<String>,
        action: &Value,
    ) -> Result<String, Error> {
        let thread = a.thread()?.to_string();
        let account = a.actor.account.to_string();
        let (space, job, app) = (a.space.clone(), a.job.clone(), app.to_owned());
        let action = bounded_json(action, DETAIL_LIMIT)?;
        let cutoff = self.approval_cutoff();
        self.security_change(move |tx| {
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_access WHERE account=?1 AND space=?2 AND thread=?3 AND job=?4)",
                params![account,space,thread,job], |r| r.get(0),
            ).map_err(db_err)?;
            if !valid {
                return Err(Error::not_found("turn not found"));
            }
            if let Some(id) = tx.query_row(
                "SELECT id FROM agent_approvals WHERE job=?1 AND kind=?2 AND app=?3 AND write_access=?4 AND action_hash IS ?5 AND status='pending'",
                params![job,kind,app,write,action_hash], |r| r.get(0),
            ).optional().map_err(db_err)? {
                return Ok(id);
            }
            tx.execute(
                "DELETE FROM agent_approvals WHERE (account=?1 OR (space=?3 AND thread=?4)) AND
                 ((status IN ('answered','denied','cancelled') AND resolved_ms<=?2)
                 OR (status='once' AND claimed_ms<=?2))",
                params![account,cutoff,space,thread],
            ).map_err(db_err)?;
            let (account_count, thread_count): (i64, i64) = tx.query_row(
                "SELECT (SELECT count(*) FROM agent_approvals WHERE account=?1),
                        (SELECT count(*) FROM agent_approvals WHERE space=?2 AND thread=?3)",
                params![account,space,thread], |r| Ok((r.get(0)?,r.get(1)?)),
            ).map_err(db_err)?;
            if account_count >= ACCOUNT_LIMIT || thread_count >= THREAD_LIMIT {
                return Err(Error::unavailable("request limit reached for this account or thread"));
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO agent_approvals(id,job,account,space,thread,app,write_access,kind,action,action_hash) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![id,job,account,space,thread,app,write,kind,action,action_hash],
            ).map_err(db_err)?;
            Ok(id)
        })
    }

    /// Read one account's pending requests from the protected store. Membership
    /// remains a separate delivery check made against each returned space.
    pub(crate) fn personal_requests(
        &self,
        account: AccountId,
    ) -> Result<Vec<PersonRequest>, Error> {
        if self.security.is_none() {
            return Ok(Vec::new());
        }
        self.security_read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id,space,thread,kind,app,write_access,action FROM agent_approvals WHERE account=?1 AND status='pending' ORDER BY id",
            ).map_err(db_err)?;
            let rows = stmt.query_map([account.to_string()], |r| {
                Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get::<_,String>(3)?,r.get(4)?,r.get(5)?,r.get::<_,String>(6)?))
            }).map_err(db_err)?;
            rows.map(|row| {
                let (id,space,thread,kind,app,write,action) = row.map_err(db_err)?;
                let detail = if kind == "question" {
                    serde_json::from_str(&action).map_err(|_| Error::internal("stored question"))?
                } else {
                    let action: JsonObject = serde_json::from_str(&action)
                        .map_err(|_| Error::internal("stored approval"))?;
                    let risk = action.get("risk").and_then(Value::as_str);
                    RequestDetail::Approval {
                        app, write, lifetimes: approval_lifetimes(risk, write)?.to_vec(), action,
                    }
                };
                Ok(PersonRequest { id, space_id: Some(space), thread_id: Some(thread), detail })
            }).collect()
        })
    }

    /// Locate one request owned by the authenticated account without exposing
    /// another account's ID. One indexed protected lookup.
    fn personal_request_route(
        &self,
        account: AccountId,
        id: &str,
    ) -> Result<(String, String), Error> {
        let id = id.to_owned();
        self.security_read(move |c| {
            c.query_row(
                "SELECT space,job FROM agent_approvals WHERE account=?1 AND id=?2",
                params![account.to_string(), id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(db_err)?
            .ok_or_else(|| Error::not_found("request not found"))
        })
    }

    /// Resolve an owned request idempotently and return its space and job. The
    /// callback authorizes that space. An approval also requires fresh person
    /// proof at the caller. The space jobs lock keeps the named turn waiting on
    /// that request across the recovery commit for grant publication and token
    /// revocation. A failed commit acknowledges neither change.
    pub fn agent_decide(
        &self,
        account: AccountId,
        id: &str,
        rid: &str,
        status: &'static str,
        value: Option<JsonObject>,
        authorize: impl FnOnce(&str) -> Result<(), Error>,
    ) -> Result<(String, String), Error> {
        let cutoff = self.approval_cutoff();
        let (id, rid) = (id.to_owned(), rid.to_owned());
        let value = value
            .map(|v| bounded_json(&serde_json::json!(v), DETAIL_LIMIT))
            .transpose()?;
        let (space, job) = self.personal_request_route(account, &id)?;
        authorize(&space)?;
        self.tx(&space, |jobs| {
            let waiting: bool = jobs.query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND account=?2 AND operation='agent.turn'
                 AND state='waiting' AND json_extract(checkpoint,'$.person_request')=?3)",
                params![job,account.to_string(),id], |r| r.get(0),
            ).map_err(db_err)?;
            let job = job.clone();
            self.security_change(move |tx| {
                let row = tx.query_row(
                    "SELECT kind,status,decision_request,decision_value,resolved_ms,json_extract(action,'$.risk'),write_access
                     FROM agent_approvals WHERE id=?1 AND account=?2 AND job=?3",
                    params![id,account.to_string(),job],
                    |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,
                        r.get::<_,Option<String>>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,bool>(6)?)),
                ).optional().map_err(db_err)?.ok_or_else(|| Error::not_found("request not found"))?;
                let (kind, prior, prior_rid, prior_value, resolved_ms, risk, write) = row;
                if (kind == "question") != (status == "answered") {
                    return Err(Error::not_found("request not found"));
                }
                let lifetime = match status {
                    "once" => Some(ApprovalLifetime::Once),
                    "this_thread" => Some(ApprovalLifetime::ThisThread),
                    _ => None,
                };
                if let Some(lifetime) = lifetime
                    && !approval_lifetimes(risk.as_deref(), write)?.contains(&lifetime)
                {
                    return Err(Error::invalid_input("this action only allows once approval"));
                }
                if prior != "pending" {
                    return if prior == status && prior_rid.as_deref() == Some(&rid) && prior_value == value
                        && resolved_ms.is_some_and(|at| at > cutoff) {
                        Ok(())
                    } else {
                        Err(Error::conflict("request already resolved"))
                    };
                }
                if !waiting {
                    return Err(Error::conflict("turn is no longer waiting on this request"));
                }
                let reused: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_approvals WHERE account=?1 AND decision_request=?2 AND id!=?3 AND resolved_ms>?4)",
                    params![account.to_string(),rid,id,cutoff], |r| r.get(0),
                ).map_err(db_err)?;
                if reused {
                    return Err(Error::request_reused());
                }
                if lifetime.is_some() {
                    // The jobs lock keeps the requesting turn parked while its credential ends.
                    let changed = tx.execute(
                        "UPDATE agent_access SET hash=NULL,expires_ms=0,updated_ms=?1 WHERE job=?2 AND account=?3",
                        params![now_ms(),job,account.to_string()],
                    ).map_err(db_err)?;
                    if changed != 1 {
                        return Err(Error::conflict("turn authority has ended"));
                    }
                }
                tx.execute(
                    "UPDATE agent_approvals SET status=?1,decision_request=?2,decision_value=?3,resolved_ms=?4 WHERE id=?5",
                    params![status,rid,value,now_ms(),id],
                ).map_err(db_err)?;
                Ok(())
            })
        })?;
        Ok((space, job))
    }

    /// Private form/approval outcomes for the next execution of this same job.
    /// One bounded read, with no delivery to shared job events or checkpoints.
    pub(crate) fn agent_request_outcomes(
        &self,
        account: AccountId,
        job: &str,
    ) -> Result<Vec<Value>, Error> {
        let job = job.to_owned();
        self.security_read(move |c| {
            let mut stmt = c.prepare(
                "SELECT id,kind,status,decision_value,action FROM agent_approvals WHERE account=?1 AND job=?2 AND status NOT IN ('pending','cancelled') ORDER BY resolved_ms DESC, id DESC LIMIT 1"
            ).map_err(db_err)?;
            let rows = stmt.query_map(params![account.to_string(),job], |r| {
                Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,String>(4)?))
            }).map_err(db_err)?;
            rows.map(|row| {
                let (id,kind,status,answer,action) = row.map_err(db_err)?;
                let answer = answer.map(|v| serde_json::from_str::<Value>(&v)).transpose().map_err(|_| Error::internal("stored answer"))?;
                let action = if kind == "approval" {
                    serde_json::from_str::<Value>(&action).map_err(|_| Error::internal("stored approval"))?
                } else {
                    Value::Null
                };
                Ok(serde_json::json!({"id":id,"kind":kind,"status":status,"answer":answer,"action":action}))
            }).collect()
        })
    }
}

impl Jobs {
    /// Park a current turn with its chosen machine recorded in its checkpoint.
    /// Lock order is space jobs, then protected auth or machine release. Neither
    /// callback can re-enter jobs. A stale run cannot park its successor.
    pub fn agent_park_request(
        &self,
        job: &super::jobs::Job,
        publish: impl FnOnce() -> Result<String, Error>,
    ) -> Result<String, Error> {
        let id = self.tx(&job.space, |tx| {
            let changed = tx.execute(
                "UPDATE jobs SET checkpoint=json_set(checkpoint,'$.request_machine',json_extract(checkpoint,'$.machine_id'))
                 WHERE id=?1 AND runs=?2 AND state='running' AND json_type(checkpoint,'$.machine_id')='text'",
                params![job.id, job.runs],
            ).map_err(db_err)?;
            if changed != 1 {
                return Err(Error::conflict("turn is no longer running on its machine"));
            }
            let request = publish()?;
            tx.execute("UPDATE jobs SET state='waiting',checkpoint=json_set(checkpoint,'$.person_request',?2) WHERE id=?1",
                params![job.id,request],
            ).map_err(db_err)?;
            Ok(request)
        })?;
        self.settle_run(&job.id, job.runs);
        Ok(id)
    }

    /// Recover decisions that committed immediately before a crash. Startup
    /// scans only spaces with known jobs and each space's indexed waiting set.
    /// A parked job has already released its lease, even when the old lease
    /// record's retention has elapsed.
    pub(crate) fn recover_request_waits(&self) -> Result<Vec<(String, String)>, Error> {
        let spaces: std::collections::BTreeSet<_> = self
            .routes
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .cloned()
            .collect();
        let mut woken = Vec::new();
        for space in spaces {
            let waits = {
                let conn = self.conn(&space)?;
                let conn = conn.lock().map_err(db_err)?;
                let mut query = conn
                    .prepare(
                        "SELECT id,json_extract(checkpoint,'$.person_request') FROM jobs
                     WHERE state='waiting' AND operation='agent.turn'",
                    )
                    .map_err(db_err)?;
                query
                    .query_map([], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?))
                    })
                    .map_err(db_err)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(db_err)?
            };
            for (job, request) in waits {
                let Some(request) = request else {
                    continue;
                };
                let (checked_job, checked_request) = (job.clone(), request.clone());
                let resolved: bool = self.security_read(move |conn| conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_approvals WHERE id=?1 AND job=?2 AND status IN ('answered','denied','once','this_thread'))",
                    params![checked_request,checked_job], |r| r.get(0),
                ).map_err(db_err))?;
                if resolved && self.agent_wake_request(&space, &job, &request)? {
                    woken.push((space.clone(), job));
                }
            }
        }
        Ok(woken)
    }

    /// Interrupt the removed member's active native jobs after the protected
    /// membership commit. One space-local transaction; no other accounts stop.
    pub(crate) fn interrupt_revoked_account(
        &self,
        space: &str,
        account: AccountId,
    ) -> Result<(), Error> {
        if self.root.is_none() {
            return Ok(());
        }
        let interrupted = self.tx(space, |tx| {
            let mut update = tx.prepare(
                "UPDATE jobs SET state='interrupted' WHERE operation='agent.turn' AND account=?1
                 AND state IN ('admitted','running','waiting','retry_waiting') RETURNING id",
            ).map_err(db_err)?;
            update
                .query_map([account.to_string()], |r| r.get::<_, String>(0))
                .map_err(db_err)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(db_err)
        })?;
        for id in interrupted {
            self.settle(&id);
        }
        Ok(())
    }

    /// Readmit a parked turn after its resolution, preserving the machine pin.
    /// The conditional update prevents duplicate decisions from starting two runs.
    pub(crate) fn agent_wake_request(
        &self,
        space: &str,
        job: &str,
        request: &str,
    ) -> Result<bool, Error> {
        self.tx(space, |tx| tx.execute(
            "UPDATE jobs SET state='admitted' WHERE id=?1 AND state='waiting' AND operation='agent.turn'
             AND json_extract(checkpoint,'$.person_request')=?2",
            params![job,request],
        ).map(|n| n == 1).map_err(db_err))
    }
}

impl Jobs {
    fn approval_cutoff(&self) -> i64 {
        now_ms()
            .saturating_sub(i64::try_from(self.receipt_retention.as_millis()).unwrap_or(i64::MAX))
    }

    /// Spend one matching approval before its action runs. A spent claim can
    /// identify a receipt, but cannot admit a handler again. An explicitly
    /// approved retry may spend another grant if the prior result is absent.
    /// The space jobs lock holds the running state stable across the one protected
    /// admission commit, so a stopped run cannot spend the approval.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn claim_once(
        &self,
        a: &Authority,
        op: &str,
        input: &JsonObject,
        rid: &str,
        risk: OperationRisk,
        mutates: bool,
        token_hash: Vec<u8>,
        fingerprint: String,
        has_receipt: bool,
    ) -> Result<OnceAdmission, Error> {
        let account = a.actor.account.to_string();
        let (space, thread, job) = (a.space.clone(), a.thread()?.to_string(), a.job.clone());
        let app = crate::turns::access::app(op).to_owned();
        let action = crate::turns::access::action_scope(op, input, rid, risk);
        let rid = rid.to_owned();
        let app_scope = matches!(risk, OperationRisk::Read | OperationRisk::Reversible);
        let cutoff = self.approval_cutoff();
        self.tx(&a.space, |jobs| {
            let active: bool = jobs.query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND account=?2 AND state='running')",
                params![job,account], |r| r.get(0),
            ).map_err(db_err)?;
            if !active {
                return Err(Error::forbidden("thread credential has ended"));
            }
            self.security_change(move |tx| {
                let now = now_ms();
                let valid: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM agent_access WHERE account=?1 AND space=?2 AND thread=?3 AND job=?4 AND hash=?5 AND expires_ms>?6)",
                    params![account,space,thread,job,token_hash,now], |r| r.get(0),
                ).map_err(db_err)?;
                if !valid {
                    return Err(Error::forbidden("thread credential has ended"));
                }
                if let Some(stored) = tx.query_row(
                    "SELECT claimed_hash FROM agent_approvals WHERE account=?1 AND space=?2 AND thread=?3 AND claimed_id=?4
                     AND (claimed_ms>?5 OR ?6) LIMIT 1",
                    params![account,space,thread,rid,cutoff,has_receipt], |r| r.get::<_,String>(0),
                ).optional().map_err(db_err)? {
                    if stored != fingerprint {
                        return Err(Error::request_reused());
                    }
                    if has_receipt {
                        return Ok(OnceAdmission::Replay);
                    }
                }
                if has_receipt {
                    return Ok(OnceAdmission::Unapproved);
                }
                let changed = tx.execute(
                    "UPDATE agent_approvals SET claimed_id=?1,claimed_hash=?2,claimed_ms=?3 WHERE id=(
                     SELECT id FROM agent_approvals WHERE account=?4 AND space=?5 AND thread=?6 AND app=?7 AND status='once'
                     AND claimed_id IS NULL AND (write_access=1 OR ?8=0)
                     AND (action_hash=?9 OR (action_hash IS NULL AND ?10)) ORDER BY id LIMIT 1)",
                    params![rid,fingerprint,now,account,space,thread,app,mutates,action,app_scope],
                ).map_err(db_err)?;
                Ok(if changed == 1 { OnceAdmission::Fresh } else { OnceAdmission::Unapproved })
            })
        })
    }
}
