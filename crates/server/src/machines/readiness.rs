//! Caller-scoped placement and provider observations from the existing registry.
use super::{LeaseState, Machine, MachineService, Settings, State, db};
use calternal_core::{
    MachineExecution, MachineReadiness, PersonalMachineReadiness, ProviderCheck, ProviderReadiness,
    ReadinessResult,
};
use calternal_machine::protocol::Owner;
use calternal_types::{AccountId, Error, SpaceId};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use tokio::sync::watch;
use uuid::Uuid;

const CHECK_TTL: Duration = Duration::from_secs(300);
const MAX_CHECKS_PER_MACHINE: usize = 128;

/// Account-qualified provider status; the check never returns credentials.
pub(crate) struct Observation {
    pub(super) at: Instant,
    pub(super) value: ProviderCheck,
}

/// Account-keyed in-memory index and invalidation listeners for the registry.
/// Pool machines have a private index key and their identities stay server-side.
#[derive(Default)]
pub(super) struct Readiness {
    by_owner: HashMap<Option<AccountId>, HashSet<Uuid>>,
    watchers: HashMap<AccountId, Weak<watch::Sender<u64>>>,
}

/// Keeps one account's watch alive for the lifetime of its live subscription.
pub(crate) struct Listener {
    _sender: Arc<watch::Sender<u64>>,
    /// Only revisions affecting this account are delivered.
    pub(crate) changes: watch::Receiver<u64>,
}

impl Readiness {
    pub(super) fn insert(&mut self, machine: &Machine) {
        self.by_owner
            .entry(db::owner_account(machine.row.owner))
            .or_default()
            .insert(machine.row.id);
    }

    fn ids(&self, owner: Option<AccountId>) -> impl Iterator<Item = Uuid> + '_ {
        self.by_owner.get(&owner).into_iter().flatten().copied()
    }

    pub(super) fn change(&self, account: AccountId) {
        if let Some(sender) = self.watchers.get(&account).and_then(Weak::upgrade) {
            sender.send_modify(|revision| *revision = revision.wrapping_add(1));
        }
    }

    fn subscribe(&mut self, account: AccountId) -> Listener {
        let sender = self
            .watchers
            .get(&account)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let (sender, _) = watch::channel(0);
                let sender = Arc::new(sender);
                self.watchers.insert(account, Arc::downgrade(&sender));
                sender
            });
        Listener {
            changes: sender.subscribe(),
            _sender: sender,
        }
    }

    pub(super) fn prune(&mut self) {
        self.watchers.retain(|_, sender| sender.strong_count() > 0);
    }
}

impl State {
    pub(super) fn readiness_changed(&self, owner: Owner) {
        match owner {
            Owner::Account { id } => self.readiness.change(id),
            Owner::Pool => {
                for account in self
                    .readiness
                    .watchers
                    .keys()
                    .filter(|a| self.pool_grants.contains(a))
                {
                    self.readiness.change(*account);
                }
            }
        }
    }

    fn eligible(&self, account: AccountId, pin: Option<Uuid>) -> impl Iterator<Item = &Machine> {
        let owners = [Some(account), None];
        let indexed = owners
            .into_iter()
            .filter(move |owner| {
                pin.is_none() && (owner.is_some() || self.pool_grants.contains(&account))
            })
            .flat_map(|owner| self.readiness.ids(owner));
        pin.into_iter()
            .chain(indexed)
            .filter_map(|id| self.machines.get(&id))
            .filter(move |m| self.may_use(m, account) && !m.row.revoked)
    }
}

fn last_seen(machine: &Machine) -> Option<i64> {
    machine.last_seen_ms
}

pub(super) fn connected(machine: &Machine, settings: &Settings) -> bool {
    !machine.row.revoked
        && machine.live.as_ref().is_some_and(|live| {
            live.polls > 0
                || machine.last_seen.elapsed()
                    < (settings.poll_hold + Duration::from_secs(5)).min(settings.lease_ttl)
        })
}

impl MachineService {
    /// Snapshot the caller's indexed machine candidates and cached checks, with
    /// no SQL, filesystem I/O, or machine round trips. One registry lock.
    /// An optional saved-thread pin applies only to this account's own lease.
    pub(crate) fn readiness(
        &self,
        account: AccountId,
        space: SpaceId,
        thread: Option<Uuid>,
        saved_pin: Option<Uuid>,
        providers: &[(&str, bool)],
    ) -> Result<ReadinessResult, Error> {
        let st = self.lock()?;
        let pin = thread
            .and_then(|id| st.threads.get(&id))
            .and_then(|id| st.leases.get(id))
            .filter(|lease| lease.row.account == account && lease.row.space == space)
            .and_then(|lease| lease.row.pinned_machine)
            .or(saved_pin);
        let mut machines = Vec::new();
        let mut pool = Vec::new();
        for machine in st.eligible(account, pin) {
            let online = connected(machine, &self.settings);
            let available = online && st.free(machine) != 0;
            let checks = machine.checks.get(&account);
            let providers = providers
                .iter()
                .map(|(provider, enabled)| {
                    let check = checks
                        .and_then(|checks| checks.get(*provider))
                        .filter(|check| check.at.elapsed() < CHECK_TTL)
                        .map(|check| check.value.clone());
                    let ready = available
                        && *enabled
                        && check
                            .as_ref()
                            .is_some_and(|check| check.installed && check.signed_in);
                    (
                        (*provider).to_owned(),
                        ProviderReadiness {
                            enabled: *enabled,
                            check,
                            ready,
                        },
                    )
                })
                .collect();
            let direct = machine
                .live
                .as_ref()
                .map_or(machine.row.direct, |live| live.direct);
            let readiness = MachineReadiness {
                enrolled: true,
                connected: online,
                available,
                providers,
                execution: if direct {
                    MachineExecution::Direct
                } else {
                    MachineExecution::Sandboxed
                },
            };
            if machine.row.owner == Owner::Pool {
                pool.push(readiness);
            } else {
                machines.push(PersonalMachineReadiness {
                    machine_id: machine.row.id.to_string(),
                    name: machine.row.name.clone(),
                    last_seen_ms: last_seen(machine),
                    readiness,
                });
            }
        }
        machines.sort_by(|a, b| a.machine_id.cmp(&b.machine_id));
        // Anonymous options sort on reported attributes, never by host ID.
        pool.sort_unstable();
        Ok(ReadinessResult {
            machines,
            pool,
            check_ttl_ms: CHECK_TTL.as_secs() * 1000,
        })
    }

    /// Refuse foreign, revoked and unknown machine IDs alike. Unnamed pool
    /// probes choose an entitled online machine with capacity. One registry read.
    pub(crate) fn check_target(
        &self,
        account: AccountId,
        id: Option<Uuid>,
        pool: bool,
    ) -> Result<Uuid, Error> {
        let st = self.lock()?;
        st.eligible(account, id)
            .filter(|m| (m.row.owner == Owner::Pool) == pool)
            .filter(|_| !pool || id.is_none())
            .filter(|m| !pool || connected(m, &self.settings) && st.free(m) > 0)
            .min_by_key(|m| m.row.id)
            .map(|m| m.row.id)
            .ok_or_else(|| Error::not_found("machine not found"))
    }

    /// Cache a bounded Check event only while the same account still holds the
    /// current machine lease. One registry lock, zero flushes.
    pub(crate) fn record_check(
        &self,
        account: AccountId,
        lease: Uuid,
        machine: Uuid,
        provider: String,
        value: ProviderCheck,
    ) -> Result<(), Error> {
        let mut st = self.lock()?;
        if !st.leases.get(&lease).is_some_and(|l| {
            l.row.account == account
                && l.row.machine == Some(machine)
                && matches!(l.state, LeaseState::Assigned | LeaseState::Running)
        }) {
            return Ok(());
        }
        if st
            .machines
            .get(&machine)
            .is_some_and(|m| m.row.owner == Owner::Pool)
            && !st.pool_grants.contains(&account)
        {
            return Ok(());
        }
        let Some(m) = st.machines.get_mut(&machine) else {
            return Ok(());
        };
        let pool = m.row.owner == Owner::Pool;
        if m.checks.values().map(HashMap::len).sum::<usize>() >= MAX_CHECKS_PER_MACHINE
            && !m
                .checks
                .get(&account)
                .is_some_and(|checks| checks.contains_key(&provider))
            && let Some((owner, key)) = m
                .checks
                .iter()
                .flat_map(|(id, checks)| {
                    checks.iter().map(move |(key, entry)| (*id, key, entry.at))
                })
                .min_by_key(|(_, _, at)| *at)
                .map(|(id, key, _)| (id, key.clone()))
        {
            if let Some(checks) = m.checks.get_mut(&owner) {
                checks.remove(&key);
                if checks.is_empty() {
                    m.checks.remove(&owner);
                }
            }
            if !pool || st.pool_grants.contains(&owner) {
                st.readiness.change(owner);
            }
        }
        let m = st.machines.get_mut(&machine).expect("checked machine");
        let old = m.checks.entry(account).or_default().insert(
            provider,
            Observation {
                at: Instant::now(),
                value: value.clone(),
            },
        );
        if old.is_none_or(|check| check.value != value || check.at.elapsed() >= CHECK_TTL) {
            st.readiness.change(account);
        }
        Ok(())
    }

    /// Open a coalescing watch for the verified account. No machine data is
    /// carried in this signal. Watch entries are reclaimed by the machine sweep.
    pub(crate) fn subscribe_readiness(&self, account: AccountId) -> Result<Listener, Error> {
        Ok(self.lock()?.readiness.subscribe(account))
    }

    /// Explain a placement wait without disclosing machine identities. Indexed
    /// owner/candidate reads, no SQL or I/O.
    pub(crate) fn placement_reason(
        &self,
        account: AccountId,
        pin: Option<Uuid>,
        position: Option<usize>,
    ) -> Result<Value, Error> {
        let st = self.lock()?;
        let mut enrolled = false;
        let mut online = false;
        let mut last = None;
        for machine in st.eligible(account, pin) {
            enrolled = true;
            online |= connected(machine, &self.settings);
            if machine.row.owner != Owner::Pool {
                last = last.max(last_seen(machine));
            }
        }
        let code = if !enrolled {
            "no_machine"
        } else if online {
            "waiting_for_capacity"
        } else {
            "machine_offline"
        };
        let since = if online { None } else { last };
        Ok(json!({"code":code,"queue_position":position,"since_ms":since}))
    }

    /// Last contact of the machine when the lease is interrupted. One keyed
    /// registry read. Pool hosts' timestamps remain private to the operator.
    pub(crate) fn interrupted_at(
        &self,
        account: AccountId,
        lease: Uuid,
    ) -> Result<Option<i64>, Error> {
        let st = self.lock()?;
        Ok(st
            .leases
            .get(&lease)
            .filter(|l| l.row.account == account && l.state == LeaseState::Interrupted)
            .and_then(|l| l.row.last_machine)
            .and_then(|id| st.machines.get(&id))
            .filter(|m| m.row.owner == (Owner::Account { id: account }))
            .and_then(last_seen))
    }
}

pub(super) fn expire(m: &mut Machine) -> impl Iterator<Item = AccountId> + '_ {
    m.checks.iter_mut().filter_map(|(account, checks)| {
        let count = checks.len();
        checks.retain(|_, check| check.at.elapsed() < CHECK_TTL);
        (count != checks.len()).then_some(*account)
    })
}
