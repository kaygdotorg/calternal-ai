//! Durable explicit workspace deletion; pending requests survive server restart.
use super::{LeaseState, MachineService, State, db, sql};
use calternal_machine::protocol::{DiscardReport, Down};
use calternal_types::AccountId;
use calternal_types::Error;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

impl MachineService {
    /// Request deletion on every retained machine that held this account/thread.
    /// Another active or newer lease refuses the request. Homes remain. Returns pending,
    /// done or failed; only a machine acknowledgement means done. A repeated failed
    /// request explicitly retries with a fresh identity. In-flight loss is failed, never automatically replayed. One FULL request transaction plus one bounded delivery batch per online target,
    /// at most 64 targets and 128 pending requests per machine; runs blocking under
    /// the existing machine registry lock to order deletion before later placement.
    pub fn discard_run(
        &self,
        account: AccountId,
        lease: Uuid,
        retry_failed: bool,
    ) -> Result<String, Error> {
        let mut st = self.lock()?;
        let row = db::lease(&st.db, lease)
            .map_err(sql)?
            .filter(|l| l.account == account)
            .ok_or_else(|| Error::not_found("run not found"))?;
        let newer:bool=st.db.query_row("SELECT EXISTS(SELECT 1 FROM leases WHERE account=?1 AND thread=?2 AND rowid>(SELECT rowid FROM leases WHERE id=?3))",params![account.to_string(),row.thread.to_string(),lease.to_string()],|r|r.get(0)).map_err(sql)?;
        if newer
            || st.leases.values().any(|l| {
                l.row.account == account
                    && l.row.thread == row.thread
                    && l.row.id != lease
                    && l.state != LeaseState::Ended
            })
        {
            return Err(Error::conflict(
                "another turn has used this workspace; discard its latest turn instead",
            ));
        }
        st.authority
            .authorize_discard(account, row.thread, lease)
            .map_err(super::authority_error)?;
        st.end_with_files(
            lease,
            "the owner discarded this workspace",
            &self.settings,
            true,
        );
        let targets = {
            let mut q=st.db.prepare("SELECT DISTINCT machine FROM leases WHERE account=?1 AND thread=?2 AND machine IS NOT NULL LIMIT 65").map_err(sql)?;
            q.query_map(params![account.to_string(), row.thread.to_string()], |r| {
                r.get::<_, String>(0)
            })
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?
        };
        if targets.len() > 64 {
            return Err(Error::unavailable(
                "more than 64 retained machine copies; discard locally on their owners' machines",
            ));
        }
        let tx = st.db.transaction().map_err(sql)?;
        let mut queued = Vec::new();
        let mut pending = false;
        let mut failed = false;
        for target in targets {
            let old: Option<(String, String)> = tx
                .query_row(
                    "SELECT request_id,state FROM workspace_discards WHERE lease=?1 AND machine=?2",
                    params![lease.to_string(), target],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()
                .map_err(sql)?;
            if old.as_ref().is_some_and(|(_, s)| s == "done") {
                continue;
            }
            if old.as_ref().is_some_and(|(_, s)| s == "failed") && !retry_failed {
                failed = true;
                continue;
            }
            pending = true;
            if old
                .as_ref()
                .is_some_and(|(_, s)| matches!(s.as_str(), "pending" | "sent"))
            {
                continue;
            }
            let n:u32=tx.query_row("SELECT count(*) FROM workspace_discards WHERE machine=?1 AND state IN ('pending','sent')",[&target],|r|r.get(0)).map_err(sql)?;
            if n >= 128 {
                return Err(Error::unavailable(
                    "this machine has 128 pending discards; wait for it to reconnect",
                ));
            }
            let request = Uuid::now_v7();
            tx.execute("DELETE FROM workspace_discards WHERE machine=?1 AND state NOT IN ('pending','sent') AND (updated_ms<?2 OR rowid NOT IN (SELECT rowid FROM workspace_discards WHERE machine=?1 AND state NOT IN ('pending','sent') ORDER BY updated_ms DESC LIMIT 1024))",params![target,db::now_ms()-7*86_400_000i64]).map_err(sql)?;
            tx.execute("INSERT INTO workspace_discards VALUES(?1,?2,?3,?4,?5,'pending',?6) ON CONFLICT(lease,machine) DO UPDATE SET request_id=excluded.request_id,state='pending',updated_ms=excluded.updated_ms",params![lease.to_string(),target,request.to_string(),account.to_string(),row.thread.to_string(),db::now_ms()]).map_err(sql)?;
            queued.push((
                Uuid::parse_str(&target).map_err(|_| Error::internal("invalid stored machine"))?,
                request,
            ));
        }
        tx.commit().map_err(sql)?;
        for (machine, _) in queued {
            if st.machines.get(&machine).is_some_and(|m| m.live.is_some()) {
                st.dispatch_discards(machine)?;
            }
        }
        Ok(if failed {
            "failed"
        } else if pending {
            "pending"
        } else {
            "done"
        }
        .into())
    }
}

impl State {
    /// Replay only pending durable deletes before assignments in a new session.
    /// One bounded indexed read; called on a blocking worker under the registry lock.
    pub(super) fn replay_discards(&mut self, machine: Uuid) -> Result<(), Error> {
        // Sent deletions may have completed before a session/server was lost.
        // Never repeat them over newer unreviewed files; a person must inspect/retry.
        self.db.execute("UPDATE workspace_discards SET state='failed',updated_ms=?1 WHERE machine=?2 AND state='sent'",params![db::now_ms(),machine.to_string()]).map_err(sql)?;
        self.dispatch_discards(machine)
    }

    /// Deliver never-sent requests once; one bounded read and one FULL batch commit.
    fn dispatch_discards(&mut self, machine: Uuid) -> Result<(), Error> {
        let rows = {
            let mut q=self.db.prepare("SELECT request_id,account,thread,lease FROM workspace_discards WHERE machine=?1 AND state='pending' LIMIT 128").map_err(sql)?;
            q.query_map([machine.to_string()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?
        };
        let mut batch = Vec::new();
        let mut updates = Vec::new();
        for (request, account, thread, lease) in rows {
            let parse =
                |s: &str| Uuid::parse_str(s).map_err(|_| Error::internal("invalid stored discard"));
            let (request_id, account_id, thread_id, lease_id) = (
                parse(&request)?,
                parse(&account)?,
                parse(&thread)?,
                parse(&lease)?,
            );
            let authorized = self
                .authority
                .discard_authorized(
                    calternal_types::AccountId::from_uuid(account_id)
                        .map_err(|_| Error::internal("invalid stored account"))?,
                    thread_id,
                    lease_id,
                )
                .map_err(super::authority_error)?;
            let conflict = !authorized
                || self.leases.values().any(|l| {
                    l.row.account.as_uuid() == account_id
                        && l.row.thread == thread_id
                        && l.row.id != lease_id
                        && l.state != LeaseState::Ended
                });
            updates.push((request_id, if conflict { "failed" } else { "sent" }));
            if !conflict {
                batch.push(Down::Discard {
                    request_id,
                    account_id,
                    thread_id,
                });
            }
        }
        let tx = self.db.transaction().map_err(sql)?;
        for (request, state) in updates {
            tx.execute("UPDATE workspace_discards SET state=?1,updated_ms=?2 WHERE request_id=?3 AND state='pending'",params![state,db::now_ms(),request.to_string()]).map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        for message in batch {
            self.push(machine, message);
        }
        Ok(())
    }
    /// Commit a bounded completion batch before acknowledging the machine sync.
    /// Stale request identities and reports from another machine change nothing.
    pub(super) fn finish_discards(
        &mut self,
        machine: Uuid,
        reports: &[DiscardReport],
    ) -> Result<(), Error> {
        if reports.len() > calternal_machine::protocol::MAX_DISCARD_REPORTS {
            return Err(Error::invalid_input("too many discard reports"));
        }
        if reports.is_empty() {
            return Ok(());
        }
        let tx = self.db.transaction().map_err(sql)?;
        for r in reports {
            tx.execute("UPDATE workspace_discards SET state=?1,updated_ms=?2 WHERE request_id=?3 AND machine=?4 AND state IN ('sent','failed')",params![if r.success{"done"}else{"failed"},db::now_ms(),r.request_id.to_string(),machine.to_string()]).map_err(sql)?;
        }
        tx.execute("DELETE FROM workspace_discards WHERE machine=?1 AND state NOT IN ('pending','sent') AND (updated_ms<?2 OR rowid NOT IN (SELECT rowid FROM workspace_discards WHERE machine=?1 AND state NOT IN ('pending','sent') ORDER BY updated_ms DESC LIMIT 1024))",params![machine.to_string(),db::now_ms()-7*86_400_000i64]).map_err(sql)?;
        let pending = if self.pending_erasure_machines.contains(&machine) {
            super::erasure_queue::acknowledge(&tx, machine, reports)?
        } else {
            false
        };
        tx.commit().map_err(sql)?;
        if !pending {
            self.pending_erasure_machines.remove(&machine);
        }
        Ok(())
    }
}
