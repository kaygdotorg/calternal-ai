//! Access records share the existing per-space job connection and durable transactions.
use super::{
    Jobs,
    job_sql::{db_err, now_ms},
};
use crate::{error::HostError, turns::access::Authority};
use calternal_auth::token::{hash, new_session_token};
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl Jobs {
    /// Issue a token for a turn, revoking the thread's previous token in one FULL
    /// transaction. Only its hash is stored. Lifetime: ten minutes or turn end.
    pub fn agent_token(&self, a: &Authority, run: u32) -> Result<String, HostError> {
        let secret =
            new_session_token().map_err(|_| HostError::internal("token randomness unavailable"))?;
        let token = format!("ct1_{}_{}", a.space, secret);
        self.tx(&a.space,|tx|{
            let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND account=?2 AND state='running' AND runs=?3)",params![a.job,a.account,run],|r|r.get(0)).map_err(db_err)?;
            if !active{return Err(HostError::conflict("turn run is no longer current"));}
            let current:Option<String>=tx.query_row("SELECT scope FROM agent_access WHERE thread=?1 AND job=?2 AND account=?3",params![a.thread,a.job,a.account],|r|r.get(0)).optional().map_err(db_err)?;
            let scope=match current{Some(scope)=>scope,None=>serde_json::to_string(a).map_err(|_|HostError::internal("access encoding"))?};
            tx.execute("INSERT INTO agent_access(thread,job,account,hash,scope,expires_ms,run) VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(thread) DO UPDATE SET job=excluded.job,account=excluded.account,hash=excluded.hash,scope=excluded.scope,expires_ms=excluded.expires_ms,run=excluded.run",params![a.thread,a.job,a.account,hash(&token),scope,now_ms()+600_000,run]).map_err(db_err)?;
            Ok(())
        })?;
        Ok(token)
    }

    /// Authenticate a thread token, rejecting expiry, rotation and a stopped job.
    /// Cost: one indexed lookup on the token's UUID-selected space connection.
    pub fn agent_authenticate(&self, token: &str) -> Result<Option<Authority>, HostError> {
        let Some(rest) = token.strip_prefix("ct1_") else {
            return Ok(None);
        };
        let Some((space, _)) = rest.split_once('_') else {
            return Ok(None);
        };
        if Uuid::parse_str(space).is_err() || token.len() > 160 {
            return Ok(None);
        }
        // Unknown tokens never create space directories.
        if !self
            .routes
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .any(|s| s == space)
        {
            return Ok(None);
        }
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let scope: Option<String> = c.query_row("SELECT a.scope FROM agent_access a JOIN jobs j ON a.job=j.id WHERE a.hash=?1 AND a.expires_ms>?2 AND j.state='running' AND a.run=j.runs", params![hash(token),now_ms()], |r| r.get(0)).optional().map_err(db_err)?;
        Ok(scope.and_then(|v| serde_json::from_str(&v).ok()))
    }

    /// Revoke a turn's token at every end or interruption; one FULL update.
    pub fn agent_revoke(&self, space: &str, job: &str) -> Result<(), HostError> {
        self.tx(space, |tx| {
            tx.execute(
                "UPDATE agent_access SET hash=NULL,expires_ms=0 WHERE job=?1",
                [job],
            )
            .map_err(db_err)?;
            Ok(())
        })
    }

    /// Current server-owned thread state, retained with the job for seven days.
    /// Cost: one indexed read; never returns the token or its hash.
    pub fn agent_scope(&self, space: &str, job: &str) -> Result<Authority, HostError> {
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let scope: String = c
            .query_row("SELECT scope FROM agent_access WHERE job=?1", [job], |r| {
                r.get(0)
            })
            .map_err(db_err)?;
        serde_json::from_str(&scope).map_err(|_| HostError::internal("invalid agent scope"))
    }

    /// Save a mode/access change and end the old token atomically. A replacement
    /// is delivered on explicit resume so an old process never retains new authority.
    /// Cost: one FULL transaction. Permission mode is independent of sandbox selection.
    pub fn agent_scope_change(&self, a: &Authority) -> Result<(), HostError> {
        let scope = serde_json::to_string(a).map_err(|_| HostError::internal("access encoding"))?;
        self.tx(&a.space, |tx| {
            tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,scope=?1 WHERE job=?2 AND account=?3", params![scope,a.job,a.account]).map_err(db_err)?;
            Ok(())
        })
    }

    /// Create or reuse a pending scope request. Questions have no expiry; at most
    /// 32 requests per turn, removed with that turn's retained job record.
    pub fn agent_request(
        &self,
        a: &Authority,
        app: &str,
        write: bool,
    ) -> Result<String, HostError> {
        if app.is_empty()
            || app.len() > 80
            || !app.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
        {
            return Err(HostError::invalid_input(
                "app must be a short operation prefix",
            ));
        }
        self.tx(&a.space, |tx| {
            if let Some(id) = tx
                .query_row(
                    "SELECT id FROM agent_approvals WHERE job=?1 AND app=?2 AND write_access=?3",
                    params![a.job, app, write],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(db_err)?
            {
                return Ok(id);
            }
            let n: u32 = tx
                .query_row(
                    "SELECT count(*) FROM agent_approvals WHERE job=?1",
                    [&a.job],
                    |r| r.get(0),
                )
                .map_err(db_err)?;
            if n >= 32 {
                return Err(HostError::unavailable(
                    "this turn has 32 access requests; finish or cancel it first",
                ));
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO agent_approvals VALUES(?1,?2,?3,?4,0,'{}')",
                params![id, a.job, app, write],
            )
            .map_err(db_err)?;
            Ok(id)
        })
    }

    /// Consume a pending approval for the exact turn, recording its scope. Caller
    /// must prove a fresh person session; agent credentials never reach this method.
    /// Cost: one FULL transaction; grants last this thread's current turn only.
    pub fn agent_approve(&self, a: &mut Authority, id: &str) -> Result<(), HostError> {
        let space = a.space.clone();
        self.tx(&space, |tx| {
            let grant: Option<(String,bool)> = tx.query_row("SELECT app,write_access FROM agent_approvals WHERE id=?1 AND job=?2 AND approved=0", params![id,a.job], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(db_err)?;
            let grant = grant.ok_or_else(|| HostError::not_found("approval not found"))?;
            a.grants.push(grant);
            let scope = serde_json::to_string(a).map_err(|_| HostError::internal("access encoding"))?;
            tx.execute("UPDATE agent_approvals SET approved=1 WHERE id=?1",[id]).map_err(db_err)?;
            tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,scope=?1 WHERE job=?2",params![scope,a.job]).map_err(db_err)?;
            Ok(())
        })
    }

    /// Explicitly readmit an interrupted turn; final and running turns cannot be
    /// replayed. Cost: one conditional FULL update; checkpoints remain unchanged.
    pub fn agent_resume(&self, space: &str, job: &str) -> Result<(), HostError> {
        self.tx(space, |tx| {
            if tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1 AND operation='agent.turn' AND state='interrupted'",[job]).map_err(db_err)? != 1 { return Err(HostError::conflict("only an interrupted turn can resume")); }
            Ok(())
        })
    }
}

impl Jobs {
    /// Fence an active turn after an access change; resume is always explicit.
    /// One FULL transaction plus settling its live stream and cancellation flag.
    pub fn agent_interrupt(&self, space: &str, job: &str) -> Result<(), HostError> {
        self.tx(space,|tx|{tx.execute("UPDATE jobs SET state='interrupted' WHERE id=?1 AND state IN ('running','admitted','retry_waiting')",[job]).map_err(db_err)?;Ok(())})?;
        self.settle(job);
        Ok(())
    }
}

impl Jobs {
    /// Revalidate a thread session's credential hash at a plugin boundary. Reads
    /// one indexed access row and checks the turn is still running; no token value.
    pub fn agent_current(
        &self,
        space: &str,
        job: &str,
        hash: &[u8],
    ) -> Result<Option<Authority>, HostError> {
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let scope:Option<String>=c.query_row("SELECT a.scope FROM agent_access a JOIN jobs j ON a.job=j.id WHERE a.job=?1 AND a.hash=?2 AND a.expires_ms>?3 AND j.state='running' AND a.run=j.runs",params![job,hash,now_ms()],|r|r.get(0)).optional().map_err(db_err)?;
        Ok(scope.and_then(|s| serde_json::from_str(&s).ok()))
    }
}

impl Jobs {
    /// Durably request approval for an exact action after person-capability checks.
    /// Pending requests do not expire and are capped at 32 per turn; their bounded
    /// details and hashes are deleted with the job after seven days.
    pub fn agent_action_request(
        &self,
        a: &Authority,
        op: &str,
        input: &calternal_core::JsonObject,
        rid: &str,
        risk: calternal_core::OperationRisk,
        write: bool,
    ) -> Result<String, HostError> {
        let scope = crate::turns::access::action_scope(op, input, rid, risk);
        let action = super::jobs::bounded_json(
            &serde_json::json!({"operation":op,"input":input,"request_id":rid,"risk":risk}),
            16 * 1024,
        )?;
        self.tx(&a.space, |tx| {
            if let Some(id) = tx
                .query_row(
                    "SELECT id FROM agent_approvals WHERE job=?1 AND app=?2 AND approved=0",
                    params![a.job, scope],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(db_err)?
            {
                return Ok(id);
            }
            let n: u32 = tx
                .query_row(
                    "SELECT count(*) FROM agent_approvals WHERE job=?1",
                    [&a.job],
                    |r| r.get(0),
                )
                .map_err(db_err)?;
            if n >= 32 {
                return Err(HostError::unavailable("this turn has 32 access requests"));
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO agent_approvals VALUES(?1,?2,?3,?4,0,?5)",
                params![id, a.job, scope, write, action],
            )
            .map_err(db_err)?;
            Ok(id)
        })
    }

    /// List bounded approval requests on the already-authorized turn. One indexed
    /// read; no token values. Used by the CLI's trusted person approval surface.
    pub fn agent_requests(
        &self,
        space: &str,
        job: &str,
    ) -> Result<Vec<serde_json::Value>, HostError> {
        let conn = self.conn(space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut st=c.prepare("SELECT id,app,write_access,approved,action FROM agent_approvals WHERE job=?1 ORDER BY id LIMIT 32").map_err(db_err)?;
        st.query_map([job],|r|{
            let action:String=r.get(4)?;
            Ok(serde_json::json!({"id":r.get::<_,String>(0)?,"scope":r.get::<_,String>(1)?,"write":r.get::<_,bool>(2)?,"approved":r.get::<_,bool>(3)?,"action":serde_json::from_str::<serde_json::Value>(&action).unwrap_or(serde_json::Value::Null)}))
        }).map_err(db_err)?.collect::<Result<Vec<_>,_>>().map_err(db_err)
    }
}

impl Jobs {
    /// Store the turn scope while queued without a live credential. One FULL
    /// upsert. The token lifetime starts only when the machine is ready to execute.
    pub fn agent_prepare_scope(&self, a: &Authority, run: u32) -> Result<Authority, HostError> {
        self.tx(&a.space,|tx|{
            let active:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND account=?2 AND state='running' AND runs=?3)",params![a.job,a.account,run],|r|r.get(0)).map_err(db_err)?;
            if !active{return Err(HostError::conflict("turn run is no longer current"));}
            let current:Option<String>=tx.query_row("SELECT scope FROM agent_access WHERE thread=?1 AND job=?2 AND account=?3",params![a.thread,a.job,a.account],|r|r.get(0)).optional().map_err(db_err)?;
            if let Some(current)=current{return serde_json::from_str(&current).map_err(|_|HostError::internal("access encoding"));}
            let scope=serde_json::to_string(a).map_err(|_|HostError::internal("access encoding"))?;
            tx.execute("INSERT INTO agent_access(thread,job,account,hash,scope,expires_ms,run) VALUES(?1,?2,?3,NULL,?4,0,?5) ON CONFLICT(thread) DO UPDATE SET job=excluded.job,account=excluded.account,hash=NULL,scope=excluded.scope,expires_ms=0,run=excluded.run",params![a.thread,a.job,a.account,scope,run]).map_err(db_err)?;
            Ok(a.clone())
        })
    }
}

impl Jobs {
    /// Revoke only the ending run's token. A late cleanup from a fenced run must
    /// never revoke the token of a newer explicit resume. One conditional FULL update.
    pub fn agent_revoke_run(&self, space: &str, job: &str, run: u32) -> Result<(), HostError> {
        self.tx(space, |tx| {
            tx.execute(
                "UPDATE agent_access SET hash=NULL,expires_ms=0 WHERE job=?1 AND run=?2",
                params![job, run],
            )
            .map_err(db_err)?;
            Ok(())
        })
    }
}

impl Jobs {
    /// Interrupt a still-waiting retry only at the observed run. A timer from a
    /// prior run cannot interrupt a newer resume. One conditional FULL update.
    pub fn agent_interrupt_wait(&self, space: &str, job: &str, run: u32) -> Result<(), HostError> {
        let changed=self.tx(space,|tx|tx.execute("UPDATE jobs SET state='interrupted' WHERE id=?1 AND runs=?2 AND state='retry_waiting'",params![job,run]).map_err(db_err))?;
        if changed > 0 {
            self.settle_run(job, run);
        }
        Ok(())
    }
}
