//! Account-owned typed recovery defaults, separate from the auth schema and userdata.
use super::{Jobs, job_sql::db_err};
use calternal_core::AgentRetrySettings;
use calternal_types::Error;
use rusqlite::OptionalExtension;

impl Jobs {
    /// Read or replace this account's bounded recovery defaults. One private
    /// SQLite file per account, opened once per call; no installation-wide lock.
    /// `None` reads, and a missing row returns the manifest's typed defaults.
    /// Cost: one schema/open, one read, and at most one FULL upsert. Runs blocking.
    pub fn agent_settings(
        &self,
        account: &str,
        value: Option<AgentRetrySettings>,
        defaults: AgentRetrySettings,
    ) -> Result<AgentRetrySettings, Error> {
        let account_id = calternal_types::AccountId::parse(account)
            .map_err(|_| Error::not_found("account not found"))?;
        let _step = self.lifecycle.step(account_id.as_uuid())?;
        let root = self
            .root
            .as_ref()
            .and_then(|p| p.parent())
            .ok_or_else(|| Error::unavailable("jobs unavailable"))?
            .join("accounts")
            .join(account);
        calternal_fsio::private::directory(&root)
            .map_err(|_| Error::internal("account settings directory"))?;
        let file = root.join("agent-settings.db");
        calternal_fsio::private::file(&file)
            .map_err(|_| Error::internal("account settings file"))?;
        let c = rusqlite::Connection::open(file).map_err(db_err)?;
        c.pragma_update(None, "synchronous", "FULL")
            .map_err(db_err)?;
        calternal_store::migrations::migrate(&c, UPGRADE_SCHEMA).map_err(db_err)?;
        if let Some(v) = value {
            c.execute("INSERT INTO settings VALUES(1,?1,?2) ON CONFLICT(id) DO UPDATE SET provider_errors=excluded.provider_errors,subscription_limits=excluded.subscription_limits",rusqlite::params![v.provider_errors,v.subscription_limits]).map_err(db_err)?;
        }
        Ok(c.query_row(
            "SELECT provider_errors,subscription_limits FROM settings WHERE id=1",
            [],
            |r| {
                Ok(AgentRetrySettings {
                    provider_errors: r.get(0)?,
                    subscription_limits: r.get(1)?,
                })
            },
        )
        .optional()
        .map_err(db_err)?
        .unwrap_or(defaults))
    }

    /// Change this turn's retry override without changing its authority or mode.
    /// Caller must be the turn's person; one FULL update of the bounded run input.
    pub fn agent_thread_settings(
        &self,
        space: &str,
        id: &str,
        value: AgentRetrySettings,
    ) -> Result<(), Error> {
        self.tx(space, |tx| {
            tx.execute(
                "UPDATE jobs SET input=json_set(input,'$.retry_settings',json(?1)) WHERE id=?2",
                rusqlite::params![
                    serde_json::to_string(&value)
                        .map_err(|_| Error::internal("settings encoding"))?,
                    id
                ],
            )
            .map_err(db_err)?;
            Ok(())
        })
    }

    /// Waiting provider jobs for restart recovery; one bounded read per open
    /// space. No job is executed here and machine-silence interrupts stay parked.
    pub fn agent_retry_waiting(&self) -> Result<Vec<(String, String)>, Error> {
        let mut routes: Vec<_> = self
            .routes
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .map(|(id, space)| (space.clone(), id.clone()))
            .collect();
        routes.retain(|(space, id)| {
            self.get(space, id)
                .ok()
                .flatten()
                .is_some_and(|j| j.state == calternal_core::JobState::RetryWaiting)
        });
        Ok(routes)
    }

    /// Atomically admit a due retry once. A stopped/interrupted run cannot win
    /// this claim; one conditional FULL update retaining its checkpoint.
    pub fn agent_retry_due(&self, space: &str, id: &str, now: i64) -> Result<bool, Error> {
        self.tx(space,|tx|Ok(tx.execute("UPDATE jobs SET state='admitted' WHERE id=?1 AND state='retry_waiting' AND CAST(json_extract(checkpoint,'$.retry.next_attempt_ms') AS INTEGER)<=?2",rusqlite::params![id,now]).map_err(db_err)?==1))
    }
}

pub(crate) const UPGRADE_SCHEMA: calternal_store::migrations::Schema<'_> =
    calternal_store::migrations::Schema::new(
        "agent-settings.db",
        1,
        &[
            "CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),provider_errors INTEGER NOT NULL,subscription_limits INTEGER NOT NULL) STRICT;",
        ],
        &[],
    );
