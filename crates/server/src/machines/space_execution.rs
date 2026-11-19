//! Space-owned placement on the current owner's enrolled sandbox machine.
use super::*;

use rusqlite::{OptionalExtension, params};

/// Space binding checked against the owner's live enrollment at placement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SpaceExecution {
    /// The space owner's account, which sponsors usage independently of the actor.
    pub sponsor: AccountId,
    /// Required enrolled sandbox machine.
    pub machine: Uuid,
    /// Whether the current Owner enabled isolated tools on this binding.
    pub tools_enabled: bool,
}

impl State {
    /// Resolve one space binding under the registry lock.
    fn space_execution(&self, space: SpaceId) -> Result<Option<SpaceExecution>, Error> {
        let row: Option<(String, String, bool)> = self
            .db
            .query_row(
                "SELECT sponsor,machine,tools_enabled FROM space_execution WHERE space=?1",
                [space.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(sql)?;
        row.map(|(sponsor, machine, tools_enabled)| {
            Ok(SpaceExecution {
                sponsor: AccountId::parse(&sponsor)
                    .map_err(|_| Error::internal("invalid stored execution sponsor"))?,
                machine: Uuid::parse_str(&machine)
                    .map_err(|_| Error::internal("invalid stored execution machine"))?,
                tools_enabled,
            })
        })
        .transpose()
    }

    /// Only current, isolated enrollment can host a shared login.
    pub(super) fn shared_eligible(&self, target: SpaceExecution) -> bool {
        self.machines.get(&target.machine).is_some_and(|machine| {
            machine.row.owner == (Owner::Account { id: target.sponsor })
                && !machine.row.revoked
                && !machine.row.direct
                && machine.live.as_ref().is_none_or(|session| {
                    session.shared_provider && (!target.tools_enabled || session.shared_tools)
                })
        })
    }

    /// Reject a stale or revoked binding before admitting work. The caller
    /// separately checks the space's current Owner through the directory.
    pub(super) fn require_execution(
        &self,
        space: SpaceId,
        target: SpaceExecution,
    ) -> Result<(), Error> {
        if !self.space_execution(space)?.is_some_and(|current| {
            current.machine == target.machine
                && current.sponsor == target.sponsor
                && (!target.tools_enabled || current.tools_enabled)
        }) || !self.shared_eligible(target)
        {
            return Err(Error::unavailable("shared space machine is unavailable"));
        }
        Ok(())
    }
}

impl MachineService {
    /// Read one shared binding without exposing machine state for other accounts.
    pub(crate) fn space_execution(&self, space: SpaceId) -> Result<Option<SpaceExecution>, Error> {
        self.lock()?.space_execution(space)
    }

    /// Set or clear one binding, with a fresh enrollment check. Holds the
    /// existing placement lock for one indexed read and one FULL write so
    /// configuration and machine revocation cannot race within this registry.
    pub(crate) fn set_space_execution(
        &self,
        space: SpaceId,
        sponsor: AccountId,
        machine: Option<Uuid>,
        tools_enabled: Option<bool>,
    ) -> Result<(), Error> {
        let mut st = self.lock()?;
        let db = &st.db;
        let previous = st.space_execution(space)?;
        if let Some(machine) = machine {
            let tools_enabled = tools_enabled.unwrap_or_else(|| {
                previous.is_some_and(|old| old.sponsor == sponsor && old.machine == machine && old.tools_enabled)
            });
            let target = SpaceExecution { sponsor, machine, tools_enabled };
            if !st.shared_eligible(target) {
                return Err(Error::not_found("sandbox machine not found"));
            }
            if previous.is_none() {
                db.execute(
                    "INSERT INTO space_execution (space,sponsor,machine,tools_enabled) VALUES (?1,?2,?3,?4)",
                    params![space.to_string(), sponsor.to_string(), machine.to_string(), tools_enabled],
                )
            } else {
                db.execute(
                    "UPDATE space_execution SET sponsor=?2,machine=?3,tools_enabled=?4 WHERE space=?1",
                    params![space.to_string(), sponsor.to_string(), machine.to_string(), tools_enabled],
                )
            }
        } else {
            db.execute(
                "DELETE FROM space_execution WHERE space=?1",
                [space.to_string()],
            )
        }
        .map_err(sql)?;
        if previous.is_some_and(|old| old.tools_enabled)
            && !st.space_execution(space)?.is_some_and(|current| {
                Some(current.machine) == machine
                    && current.tools_enabled
                    && previous.is_some_and(|old| {
                        old.machine == current.machine && old.sponsor == current.sponsor
                    })
            })
        {
            let affected: Vec<_> = st
                .leases
                .values()
                .filter(|lease| {
                    lease.row.space == space
                        && lease.row.shared_tools.is_some()
                        && lease.state != LeaseState::Ended
                })
                .map(|lease| lease.row.id)
                .collect();
            for lease in affected {
                st.end_with_files(
                    lease,
                    "shared tool access was withdrawn",
                    &self.settings,
                    true,
                );
            }
        }
        Ok(())
    }
}
