//! Remove account or space placement after its owner-local steps have drained.
use super::*;
use calternal_types::Owner as ErasedOwner;

/// A witnessed deletion can remove machine authority before this local store
/// is cleaned. Reuse the boot inventory and commit only if a flag is orphaned.
pub(super) fn clear_orphan_drains(
    db: &mut rusqlite::Connection,
    machines: &std::collections::HashMap<Uuid, state::Machine>,
    draining: std::collections::HashSet<Uuid>,
) -> Result<(), Error> {
    let stale: Vec<_> = draining
        .into_iter()
        .filter(|id| !machines.contains_key(id))
        .collect();
    if stale.is_empty() {
        return Ok(());
    }
    let tx = db.transaction().map_err(sql)?;
    for id in stale {
        db::set_draining(&tx, id, false).map_err(sql)?;
    }
    tx.commit().map_err(sql)
}

impl MachineService {
    /// Fence cached live machines and space carriers before storage erasure.
    /// Replace erased stopping leases with
    /// machine-only reservations. Holds the registry lock across the cleanup
    /// transaction so teardown cannot race lease deletion. No remote waits.
    pub(crate) fn retire_owner(&self, owner: ErasedOwner) -> Result<(), Error> {
        let mut st = self.lock()?;
        let st = &mut *st;
        let matches = |row: &db::LeaseRow| match owner {
            ErasedOwner::Account(id) => row.account == id || row.shared_sponsor == Some(id),
            ErasedOwner::Space(id) => row.space == id,
        };
        let leases: Vec<_> = st
            .leases
            .values()
            .filter(|l| matches(&l.row))
            .map(|l| l.row.id)
            .collect();
        for lease in &leases {
            let keep_files = !st.leases[lease].row.worker;
            st.end_with_files(*lease, "owner erased", &self.settings, keep_files);
        }
        let mut machines = Vec::new();
        if let ErasedOwner::Account(id) = owner {
            machines.extend(
                st.machines
                    .values()
                    .filter_map(|m| (m.row.owner == (Owner::Account { id })).then_some(m.row.id)),
            );
            for machine in &machines {
                st.apply_revocation(*machine, &self.settings)?;
            }
            st.pool_grants.remove(&id);
            for m in st.machines.values_mut() {
                m.checks.remove(&id);
            }
        }
        let tx = st.db.transaction().map_err(sql)?;
        for machine in &machines {
            db::set_draining(&tx, *machine, false).map_err(sql)?;
        }
        let pending = super::erasure_queue::queue(&tx, owner, &machines)?;
        for lease in &leases {
            if let Some(stop) = st.worker_stops.get(lease) {
                db::put_erased_worker_stop(&tx, *lease, stop.machine).map_err(sql)?;
            }
        }
        match owner {
            ErasedOwner::Account(id) => {
                tx.execute(
                    "DELETE FROM workspace_discards WHERE account=?1",
                    [id.to_string()],
                )
                .map_err(sql)?;
                tx.execute(
                    "DELETE FROM space_execution WHERE sponsor=?1",
                    [id.to_string()],
                )
                .map_err(sql)?;
                tx.execute(
                    "DELETE FROM leases WHERE account=?1 OR shared_sponsor=?1",
                    [id.to_string()],
                )
                .map_err(sql)?;
            }
            ErasedOwner::Space(id) => {
                tx.execute("DELETE FROM workspace_discards WHERE lease IN (SELECT id FROM leases WHERE space=?1)", [id.to_string()]).map_err(sql)?;
                tx.execute(
                    "DELETE FROM space_execution WHERE space=?1",
                    [id.to_string()],
                )
                .map_err(sql)?;
                tx.execute("DELETE FROM leases WHERE space=?1", [id.to_string()])
                    .map_err(sql)?;
            }
        }
        tx.commit().map_err(sql)?;
        for machine in machines {
            st.machines.remove(&machine);
        }
        st.execs.retain(|_, exec| match owner {
            ErasedOwner::Account(id) => exec.account != id,
            ErasedOwner::Space(_) => !leases.contains(&exec.lease),
        });
        st.pending_erasure_machines.extend(pending.iter().copied());
        for machine in pending {
            if let Some(live) = st.machines.get(&machine).and_then(|m| m.live.as_ref()) {
                live.notify.notify_one();
            }
        }
        for lease in leases {
            // Remote plugin workers run on the operator pool. The machine
            // reservation stays held, while the account loses its association.
            if let Some(account) = st
                .worker_stops
                .get_mut(&lease)
                .and_then(|stop| stop.account.take())
            {
                st.release_pool_slot(account);
                st.readiness.change(account);
            }
            if let Some(placement) = st.leases.remove(&lease) {
                if st.threads.get(&placement.row.thread) == Some(&lease) {
                    st.threads.remove(&placement.row.thread);
                }
                st.workers.remove(&lease);
            }
        }
        Ok(())
    }
}
