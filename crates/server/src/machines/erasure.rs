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
    /// Fence cached live machines and remove placements for a retired owner.
    /// Uses the existing registry lock and one FULL cleanup transaction after
    /// the lease-stop commits. No waits for remote acknowledgements.
    pub(crate) fn retire_owner(&self, owner: ErasedOwner) -> Result<(), Error> {
        let mut st = self.lock()?;
        let matches = |row: &db::LeaseRow| match owner {
            ErasedOwner::Account(id) => row.account == id,
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
        let pending;
        if let ErasedOwner::Account(id) = owner {
            let machines: Vec<_> = st
                .machines
                .values()
                .filter(|m| m.row.owner == (Owner::Account { id }))
                .map(|m| m.row.id)
                .collect();
            for machine in &machines {
                st.apply_revocation(*machine, &self.settings)?;
            }
            st.pool_grants.remove(&id);
            for m in st.machines.values_mut() {
                m.checks.remove(&id);
            }
            let tx = st.db.transaction().map_err(sql)?;
            for machine in &machines {
                tx.execute(
                    "DELETE FROM draining WHERE machine=?1",
                    [machine.to_string()],
                )
                .map_err(sql)?;
            }
            pending = super::erasure_queue::queue(&tx, owner, &machines)?;
            tx.execute(
                "DELETE FROM workspace_discards WHERE account=?1",
                [id.to_string()],
            )
            .map_err(sql)?;
            tx.execute("DELETE FROM leases WHERE account=?1", [id.to_string()])
                .map_err(sql)?;
            tx.commit().map_err(sql)?;
            for machine in machines {
                st.machines.remove(&machine);
            }
            st.execs.retain(|_, exec| exec.account != id);
        } else if let ErasedOwner::Space(id) = owner {
            let tx = st.db.transaction().map_err(sql)?;
            pending = super::erasure_queue::queue(&tx, owner, &[])?;
            tx.execute("DELETE FROM workspace_discards WHERE lease IN (SELECT id FROM leases WHERE space=?1)", [id.to_string()]).map_err(sql)?;
            tx.execute("DELETE FROM leases WHERE space=?1", [id.to_string()])
                .map_err(sql)?;
            tx.commit().map_err(sql)?;
            st.execs.retain(|_, exec| !leases.contains(&exec.lease));
        } else {
            unreachable!("all storage owner types handled");
        }
        st.pending_erasure_machines.extend(pending.iter().copied());
        for machine in pending {
            if let Some(live) = st.machines.get(&machine).and_then(|m| m.live.as_ref()) {
                live.notify.notify_one();
            }
        }
        for lease in leases {
            if let Some(placement) = st.leases.remove(&lease) {
                if st.threads.get(&placement.row.thread) == Some(&lease) {
                    st.threads.remove(&placement.row.thread);
                }
                st.workers.remove(&lease);
            }
        }
        // A revoked machine cannot send a teardown acknowledgement. Retain
        // stops on every still-enrolled machine until it confirms teardown.
        let orphaned: Vec<_> = st
            .worker_stops
            .iter()
            .filter_map(|(lease, (machine, _, _))| {
                (!st.machines.contains_key(machine)).then_some(*lease)
            })
            .collect();
        for lease in &orphaned {
            st.worker_stops.remove(lease);
        }
        if !orphaned.is_empty() {
            self.worker_changes.notify_waiters();
        }
        Ok(())
    }
}
