//! Protected authority in auth.db; job execution remains in per-space jobs.db.
use super::{
    Jobs,
    job_sql::{db_err, now_ms},
};
use crate::turns::access::{Authority, Grant};
use calternal_auth::token::{hash, new_session_token};
use calternal_types::Error;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl Jobs {
    /// Issue a token for a turn, revoking the thread's previous token in one FULL
    /// transaction. Only its hash is stored. Lifetime: ten minutes or turn end.
    pub fn agent_token(&self, a: &Authority, run: u32) -> Result<String, Error> {
        let secret =
            new_session_token().map_err(|_| Error::internal("token randomness unavailable"))?;
        let token = format!("ct1_{}_{}", a.space, secret);
        let (a, digest) = (a.clone(), hash(&token));
        self.with_current(&a.clone(),run,move |tx|{
            let current=retained_scope(tx,&a)?;
            let scope=match current {
                Some(saved) if saved.job==a.job=>saved,
                Some(_)=>return Err(Error::conflict("prepare this turn before issuing its credential")),
                None=>a.clone(),
            };
            let scope=serde_json::to_string(&scope).map_err(|_|Error::internal("access encoding"))?;
            let changed=tx.execute("UPDATE agent_access SET hash=?1,scope=?2,expires_ms=?3,run=?4,updated_ms=?5 WHERE account=?6 AND space=?7 AND thread=?8 AND job=?9",params![digest,scope,now_ms()+600_000,run,now_ms(),a.actor.account.to_string(),a.space,a.thread()?.to_string(),a.job]).map_err(db_err)?;
            if changed==0 {tx.execute("INSERT INTO agent_access(thread,job,account,hash,scope,expires_ms,run,space,updated_ms) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![a.thread()?.to_string(),a.job,a.actor.account.to_string(),digest,scope,now_ms()+600_000,run,a.space,now_ms()]).map_err(db_err)?;}
            Ok(())
        })?;
        Ok(token)
    }

    /// Authenticate a thread token, rejecting expiry, rotation and a stopped job.
    /// Cost: one indexed lookup on the token's UUID-selected space connection.
    pub fn agent_authenticate(&self, token: &str) -> Result<Option<Authority>, Error> {
        let Some(rest) = token.strip_prefix("ct1_") else {
            return Ok(None);
        };
        let Some((space, _)) = rest.split_once('_') else {
            return Ok(None);
        };
        if token.len() > 160 || Uuid::parse_str(space).is_err() {
            return Ok(None);
        }
        let digest = hash(token);
        let space = space.to_owned();
        let saved = self.security_read(move |c| {
            c.query_row(
                "SELECT scope,run FROM agent_access WHERE hash=?1 AND expires_ms>?2 AND space=?3",
                params![digest, now_ms(), space],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
            )
            .optional()
            .map_err(db_err)
        })?;
        self.live_authority(saved)
    }

    /// Revoke a turn's token at every end or interruption; one FULL update.
    pub fn agent_revoke(&self, space: &str, job: &str) -> Result<(), Error> {
        let (space, job) = (space.to_owned(), job.to_owned());
        self.security_change(move |tx| {tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,updated_ms=?3 WHERE job=?1 AND space=?2",params![job,space,now_ms()]).map_err(db_err)?;Ok(())})
    }

    /// Current server-owned thread state, bounded to 1024 threads per account.
    /// Cost: one indexed read; never returns the token or its hash.
    pub fn agent_scope(&self, space: &str, job: &str) -> Result<Authority, Error> {
        let (space, job) = (space.to_owned(), job.to_owned());
        let scope = self
            .security_read(move |c| {
                c.query_row(
                    "SELECT scope FROM agent_access WHERE job=?1 AND space=?2",
                    params![job, space],
                    |r| r.get::<_, String>(0),
                )
                .optional()
                .map_err(db_err)
            })?
            .ok_or_else(|| Error::not_found("turn access not found"))?;
        serde_json::from_str(&scope).map_err(|_| Error::internal("invalid agent scope"))
    }

    /// Save a mode/access change and end the old token atomically. A replacement
    /// is delivered on explicit resume so an old process never retains new authority.
    /// Cost: one FULL transaction. Permission mode is independent of sandbox selection.
    pub fn agent_scope_change(&self, a: &Authority) -> Result<(), Error> {
        let a = a.clone();
        self.security_change(move |tx| replace_scope(tx, &a))
    }

    /// Create or reuse a pending scope request. Questions have no expiry; at most
    /// 32 requests per turn, removed when the thread starts its next turn.
    pub fn agent_request(&self, a: &Authority, app: &str, write: bool) -> Result<String, Error> {
        if app.is_empty()
            || app.len() > 80
            || !app.bytes().all(|c| c.is_ascii_lowercase() || c == b'-')
        {
            return Err(Error::invalid_input("app must be a short operation prefix"));
        }
        let a = a.clone();
        let app = app.to_owned();
        self.security_change(move |tx| {
            if let Some(id) = tx
                .query_row(
                    "SELECT id FROM agent_approvals WHERE job=?1 AND app=?2 AND write_access=?3 AND action_hash IS NULL",
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
                return Err(Error::unavailable(
                    "this turn has 32 access requests; finish or cancel it first",
                ));
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO agent_approvals(id,job,app,write_access,approved,action,account) VALUES(?1,?2,?3,?4,0,'{}',?5)",
                params![id, a.job, app, write,a.actor.account.to_string()],
            )
            .map_err(db_err)?;
            Ok(id)
        })
    }

    /// Consume a pending approval for the exact turn, recording its scope. Caller
    /// must prove a fresh person session; agent credentials never reach this method.
    /// Cost: one FULL transaction; grants last this thread's current turn only.
    pub fn agent_approve(&self, a: &mut Authority, id: &str) -> Result<(), Error> {
        let (authority, id) = (a.clone(), id.to_owned());
        *a=self.security_change(move|tx|{
            let scope:String=tx.query_row("SELECT scope FROM agent_access WHERE job=?1 AND space=?2 AND account=?3",params![authority.job,authority.space,authority.actor.account.to_string()],|r|r.get(0)).map_err(db_err)?;
            let mut current:Authority=serde_json::from_str(&scope).map_err(|_|Error::internal("invalid scope"))?;
            let grant:Option<Grant>=tx.query_row("SELECT app,write_access,action_hash FROM agent_approvals WHERE id=?1 AND job=?2 AND account=?3 AND approved=0",params![id,current.job,current.actor.account.to_string()],|r|Ok(Grant{app:r.get(0)?,write:r.get(1)?,action_hash:r.get(2)?})).optional().map_err(db_err)?;
            current.grants.push(grant.ok_or_else(||Error::not_found("approval not found"))?);
            tx.execute("UPDATE agent_approvals SET approved=1 WHERE id=?1 AND job=?2 AND account=?3",params![id,current.job,current.actor.account.to_string()]).map_err(db_err)?;
            replace_scope(tx,&current)?;
            Ok(current)
        })?;
        Ok(())
    }

    /// Explicitly readmit an interrupted turn; final and running turns cannot be
    /// replayed. Cost: one conditional FULL update; checkpoints remain unchanged.
    pub fn agent_resume(&self, space: &str, job: &str) -> Result<(), Error> {
        self.tx(space, |tx| {
            if tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1 AND operation='agent.turn' AND state='interrupted'",[job]).map_err(db_err)? != 1 { return Err(Error::conflict("only an interrupted turn can resume")); }
            Ok(())
        })
    }
}

impl Jobs {
    /// Fence an active turn after an access change; resume is always explicit.
    /// One FULL transaction plus settling its live stream and cancellation flag.
    pub fn agent_interrupt(&self, space: &str, job: &str) -> Result<(), Error> {
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
    ) -> Result<Option<Authority>, Error> {
        let (space, job, digest) = (space.to_owned(), job.to_owned(), hash.to_vec());
        let saved=self.security_read(move |c| c.query_row("SELECT scope,run FROM agent_access WHERE job=?1 AND hash=?2 AND expires_ms>?3 AND space=?4",params![job,digest,now_ms(),space],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u32>(1)?))).optional().map_err(db_err))?;
        self.live_authority(saved)
    }
}

impl Jobs {
    /// Durably request approval for an exact action after person-capability checks.
    /// Pending requests do not expire and are capped at 32 per turn; their bounded
    /// details and hashes are removed when this thread starts its next turn.
    pub fn agent_action_request(
        &self,
        a: &Authority,
        op: &str,
        input: &calternal_core::JsonObject,
        rid: &str,
        risk: calternal_core::OperationRisk,
        write: bool,
    ) -> Result<String, Error> {
        let scope = crate::turns::access::action_scope(op, input, rid, risk);
        let app = crate::turns::access::app(op).to_owned();
        let action = super::jobs::bounded_json(
            &serde_json::json!({"operation":op,"input":input,"request_id":rid,"risk":risk}),
            16 * 1024,
        )?;
        let a = a.clone();
        self.security_change(move |tx| {
            if let Some(id) = tx
                .query_row(
                    "SELECT id FROM agent_approvals WHERE job=?1 AND app=?2 AND action_hash=?3 AND approved=0",
                    params![a.job, app, scope],
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
                return Err(Error::unavailable("this turn has 32 access requests"));
            }
            let id = Uuid::now_v7().to_string();
            tx.execute(
                "INSERT INTO agent_approvals(id,job,app,write_access,approved,action,account,action_hash) VALUES(?1,?2,?3,?4,0,?5,?6,?7)",
                params![id, a.job, app, write, action,a.actor.account.to_string(),scope],
            )
            .map_err(db_err)?;
            Ok(id)
        })
    }

    /// List bounded approval requests on the already-authorized turn. One indexed
    /// read; no token values. Used by the CLI's trusted person approval surface.
    pub fn agent_requests(&self, space: &str, job: &str) -> Result<Vec<serde_json::Value>, Error> {
        let (space, job) = (space.to_owned(), job.to_owned());
        self.security_read(move |c| {
            let mut st=c.prepare("SELECT a.id,a.app,a.write_access,a.approved,a.action,a.action_hash FROM agent_approvals a JOIN agent_access s ON a.job=s.job WHERE a.job=?1 AND s.space=?2 ORDER BY a.id LIMIT 32").map_err(db_err)?;
            st.query_map(params![job,space],|r| {
                let action:String=r.get(4)?;
                Ok(serde_json::json!({"id":r.get::<_,String>(0)?,"scope":r.get::<_,String>(1)?,"write":r.get::<_,bool>(2)?,"approved":r.get::<_,bool>(3)?,"action":serde_json::from_str::<serde_json::Value>(&action).unwrap_or_default(),"action_hash":r.get::<_,Option<String>>(5)?}))
            }).map_err(db_err)?.collect::<Result<Vec<_>,_>>().map_err(db_err)
        })
    }
}

impl Jobs {
    /// Store the turn scope while queued without a live credential. One FULL
    /// transaction. The token lifetime starts only when the machine is ready to execute.
    pub fn agent_prepare_scope(&self, a: &Authority, run: u32) -> Result<Authority, Error> {
        let mut a = a.clone();
        self.with_current(&a.clone(),run,move |tx|{
            if let Some(previous)=retained_scope(tx,&a)? {
                if previous.job==a.job{return Ok(previous)}
                a.mode=crate::turns::access::narrower(a.mode,previous.mode);
                tx.execute("DELETE FROM agent_access WHERE job=?1 AND account=?2 AND space=?3",params![previous.job,a.actor.account.to_string(),a.space]).map_err(db_err)?;
            }
            let scope=serde_json::to_string(&a).map_err(|_|Error::internal("access encoding"))?;
            tx.execute("INSERT INTO agent_access(thread,job,account,hash,scope,expires_ms,run,space,updated_ms) VALUES(?1,?2,?3,NULL,?4,0,?5,?6,?7)",params![a.thread()?.to_string(),a.job,a.actor.account.to_string(),scope,run,a.space,now_ms()]).map_err(db_err)?;
            Ok(a)
        })
    }
}

impl Jobs {
    /// Revoke only the ending run's token. A late cleanup from a fenced run must
    /// never revoke the token of a newer explicit resume. One conditional FULL update.
    pub fn agent_revoke_run(&self, space: &str, job: &str, run: u32) -> Result<(), Error> {
        let (space, job) = (space.to_owned(), job.to_owned());
        self.security_change(move |tx| {tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,updated_ms=?4 WHERE job=?1 AND space=?2 AND run=?3",params![job,space,run,now_ms()]).map_err(db_err)?;Ok(())})
    }
}

impl Jobs {
    /// Interrupt a still-waiting retry only at the observed run. A timer from a
    /// prior run cannot interrupt a newer resume. One conditional FULL update.
    pub fn agent_interrupt_wait(&self, space: &str, job: &str, run: u32) -> Result<(), Error> {
        let changed=self.tx(space,|tx|tx.execute("UPDATE jobs SET state='interrupted' WHERE id=?1 AND runs=?2 AND state='retry_waiting'",params![job,run]).map_err(db_err))?;
        if changed > 0 {
            self.settle_run(job, run);
        }
        Ok(())
    }
}

impl Jobs {
    fn security(&self) -> Result<&calternal_auth::agents::Agents, Error> {
        self.security
            .as_ref()
            .ok_or_else(|| Error::unavailable("protected agent authority unavailable"))
    }
    fn security_read<T: Send + 'static>(
        &self,
        read: impl FnOnce(&rusqlite::Connection) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        self.security()?.read(read)
    }
    fn security_change<T: Send + 'static>(
        &self,
        change: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        self.security()?.change(change)
    }
    /// Hold only this space's job lock across authority publication so an old
    /// issuer cannot race a stop/resume and replace its successor's token. Auth
    /// callbacks never acquire jobs/content locks, so lock order is one-way.
    /// Cost: one indexed job read plus the protected change's existing barrier.
    fn with_current<T: Send + 'static>(
        &self,
        a: &Authority,
        run: u32,
        change: impl FnOnce(&rusqlite::Transaction<'_>) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, Error> {
        let conn = self.conn(&a.space)?;
        let c = conn.lock().unwrap_or_else(|p| p.into_inner());
        let active:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND account=?2 AND state='running' AND runs=?3)",params![a.job,a.actor.account.to_string(),run],|r|r.get(0)).map_err(db_err)?;
        if !active {
            return Err(Error::conflict("turn run is no longer current"));
        }
        self.security_change(change)
    }
    /// Recheck the sign-in that admitted a native turn. Static development/host
    /// callers stay live; an agent cannot delegate a new admitting identity.
    /// One indexed protected-auth read for an installation, otherwise no I/O.
    pub(crate) fn admitting_session_live(
        &self,
        account: calternal_types::AccountId,
        session: &super::Session,
    ) -> Result<bool, Error> {
        match session {
            super::Session::Static => Ok(true),
            super::Session::Installation {
                account: owner,
                installation,
            } if *owner == account => self.security()?.installation_live(account, *installation),
            _ => Ok(false),
        }
    }
    fn live_authority(&self, saved: Option<(String, u32)>) -> Result<Option<Authority>, Error> {
        let Some((scope, run)) = saved else {
            return Ok(None);
        };
        let a: Authority = serde_json::from_str(&scope)
            .map_err(|_| Error::internal("invalid stored authority"))?;
        let Some(job) = self.get(&a.space, &a.job)? else {
            return Ok(None);
        };
        let current = job.account == a.actor.account
            && job.state == calternal_core::JobState::Running
            && job.runs == run;
        Ok((current && self.admitting_session_live(job.account, &job.session)?).then_some(a))
    }
}

const MAX_RETAINED_THREADS_PER_ACCOUNT: i64 = 1024;
/// One bound and one lookup for both queued-scope preparation and token issuance.
fn retained_scope(
    tx: &rusqlite::Transaction<'_>,
    a: &Authority,
) -> Result<Option<Authority>, Error> {
    let thread = a.thread()?.to_string();
    let count: i64 = tx
        .query_row(
            "SELECT count(*) FROM agent_access WHERE account=?1 AND NOT(space=?2 AND thread=?3)",
            params![a.actor.account.to_string(), a.space, thread],
            |r| r.get(0),
        )
        .map_err(db_err)?;
    if count >= MAX_RETAINED_THREADS_PER_ACCOUNT {
        return Err(Error::unavailable(
            "this account has 1024 retained turn authority records",
        ));
    }
    let scope: Option<String> = tx
        .query_row(
            "SELECT scope FROM agent_access WHERE thread=?1 AND space=?2 AND account=?3",
            params![thread, a.space, a.actor.account.to_string()],
            |r| r.get(0),
        )
        .optional()
        .map_err(db_err)?;
    scope
        .map(|s| serde_json::from_str(&s).map_err(|_| Error::internal("access encoding")))
        .transpose()
}
/// All scope replacement and credential rotation binds the complete identity.
fn replace_scope(tx: &rusqlite::Transaction<'_>, a: &Authority) -> Result<(), Error> {
    let scope = serde_json::to_string(a).map_err(|_| Error::internal("access encoding"))?;
    let changed=tx.execute("UPDATE agent_access SET hash=NULL,expires_ms=0,scope=?1,updated_ms=?2 WHERE job=?3 AND account=?4 AND space=?5 AND thread=?6",params![scope,now_ms(),a.job,a.actor.account.to_string(),a.space,a.thread()?.to_string()]).map_err(db_err)?;
    if changed != 1 {
        return Err(Error::not_found("turn access not found"));
    }
    Ok(())
}
