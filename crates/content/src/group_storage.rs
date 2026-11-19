//! One write admission limit shared by the spaces bound to a group.
use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::{Arc, Mutex},
};

use calternal_types::GroupId;
use rusqlite::{Connection, OpenFlags};

use crate::{Content, Error, Result, SpaceId, StorageLimits, StorageUsage, lock, types::internal};

pub(crate) type PoolRef = Arc<Mutex<Pool>>;

#[derive(Default)]
pub(crate) struct Groups(Mutex<Registry>);

#[derive(Default)]
struct Registry {
    groups: HashMap<GroupId, PoolRef>,
    spaces: HashMap<SpaceId, GroupId>,
}

#[derive(Default)]
pub(crate) struct Pool {
    limits: StorageLimits,
    members: BTreeSet<SpaceId>,
}

impl Groups {
    fn for_group(&self, id: GroupId) -> PoolRef {
        lock(&self.0).groups.entry(id).or_default().clone()
    }

    pub(crate) fn for_space(&self, space: SpaceId) -> Option<PoolRef> {
        let registry = lock(&self.0);
        registry
            .spaces
            .get(&space)
            .and_then(|id| registry.groups.get(id))
            .cloned()
    }

    fn assign(&self, space: SpaceId, group: Option<GroupId>) -> Option<PoolRef> {
        let mut registry = lock(&self.0);
        let previous = match group {
            Some(id) => registry.spaces.insert(space, id),
            None => registry.spaces.remove(&space),
        };
        previous
            .filter(|id| Some(*id) != group)
            .and_then(|id| registry.groups.get(&id))
            .cloned()
    }

    pub(crate) fn retire(&self, space: SpaceId) {
        if let Some(old) = self.assign(space, None) {
            lock(&old).members.remove(&space);
        }
    }
}

impl Content {
    /// Restore group bindings and ceilings before the writer starts accepting requests.
    /// The caller provides the durable installation inventory. This performs no I/O.
    pub fn restore_group_storage(
        &self,
        memberships: impl IntoIterator<Item = (SpaceId, GroupId)>,
        limits: impl IntoIterator<Item = (GroupId, StorageLimits)>,
    ) {
        for (id, limits) in limits {
            lock(&self.storage_groups.for_group(id)).limits = limits;
        }
        for (space, id) in memberships {
            lock(&self.storage_groups.for_group(id))
                .members
                .insert(space);
            self.storage_groups.assign(space, Some(id));
        }
        let slots: Vec<_> = lock(&self.spaces).values().cloned().collect();
        for slot in slots {
            if let Some(space) = lock(&slot.space).as_mut() {
                space.group_storage = self.storage_groups.for_space(space.id);
            }
        }
    }

    /// Read the committed bytes of all spaces bound to an authorized group.
    /// The caller authorizes the group. Costs one snapshot per member under
    /// the group lock. An empty group has no stored bytes.
    pub fn group_storage_usage(&self, group: GroupId) -> Result<StorageUsage> {
        let pool = lock(&self.storage_groups.0).groups.get(&group).cloned();
        let Some(pool) = pool else {
            return Ok(StorageUsage::default());
        };
        let (text, binary) = lock(&pool).totals(&self.roots.appdata, None)?;
        Ok(StorageUsage {
            text_bytes: u64::try_from(text).map_err(internal)?,
            binary_bytes: u64::try_from(binary).map_err(internal)?,
        })
    }

    /// Persist and apply a group's combined storage ceilings. Admission for
    /// this group waits for the change under its own mutex. The caller authorizes
    /// the group and supplies its durable commit. No other group waits on that I/O.
    pub fn set_group_storage_limits(
        &self,
        group: GroupId,
        limits: StorageLimits,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let pool = self.storage_groups.for_group(group);
        let mut pool = lock(&pool);
        persist()?;
        pool.limits = limits;
        Ok(())
    }

    /// Serialize a group assignment against body commits in this space. The
    /// authorized caller supplies the durable binding commit. Changes can put
    /// existing data over a group's ceiling, at which point further growth waits.
    pub fn bind_group_storage(
        &self,
        space: SpaceId,
        group: Option<GroupId>,
        persist: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        self.with_space(space, |body| {
            if let Some(id) = group {
                let next = self.storage_groups.for_group(id);
                let mut next = lock(&next);
                persist()?;
                next.members.insert(space);
            } else {
                persist()?;
            }
            if let Some(previous) = self.storage_groups.assign(space, group) {
                lock(&previous).members.remove(&space);
            }
            body.group_storage = self.storage_groups.for_space(space);
            Ok(())
        })
    }
}

impl Pool {
    /// Check growth under the group's mutex immediately before the caller's
    /// commit. The caller holds the mutex through the local SQLite commit.
    /// Read committed body counters through sibling WAL snapshots. A cold
    /// pre-counter space contributes its canonical heads through the same query
    /// as the backfill. Errors fail closed and the local transaction rolls back.
    pub(crate) fn admit(
        &self,
        appdata: &Path,
        space: SpaceId,
        before: StorageUsage,
        after: StorageUsage,
    ) -> Result<()> {
        let growing_text = after.text_bytes > before.text_bytes && self.limits.text_bytes.is_some();
        let growing_binary =
            after.binary_bytes > before.binary_bytes && self.limits.binary_bytes.is_some();
        let total =
            |usage: StorageUsage| u128::from(usage.text_bytes) + u128::from(usage.binary_bytes);
        let growing_total = total(after) > total(before) && self.limits.total_bytes.is_some();
        if !(growing_text || growing_binary || growing_total) {
            return Ok(());
        }
        let (sibling_text, sibling_binary) = self.totals(appdata, Some(space))?;
        let text = sibling_text + u128::from(after.text_bytes);
        let binary = sibling_binary + u128::from(after.binary_bytes);
        for (growing, used, limit, kind) in [
            (growing_text, text, self.limits.text_bytes, "text"),
            (growing_binary, binary, self.limits.binary_bytes, "binary"),
            (
                growing_total,
                text + binary,
                self.limits.total_bytes,
                "total",
            ),
        ] {
            if growing && limit.is_some_and(|max| used > u128::from(max)) {
                return Err(
                    Error::conflict(format!("group {kind} storage byte limit reached"))
                        .with_details(serde_json::json!({"scope":"group","kind":kind})),
                );
            }
        }
        Ok(())
    }

    fn totals(&self, appdata: &Path, skip: Option<SpaceId>) -> Result<(u128, u128)> {
        let mut text = 0;
        let mut binary = 0;
        for member in self.members.iter().copied().filter(|id| Some(*id) != skip) {
            let path = crate::db::space_path(appdata, member);
            match path.symlink_metadata() {
                Ok(meta) if meta.is_file() => {}
                Ok(_) => {
                    return Err(Error::unavailable(
                        "group storage needs a regular space database",
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(internal(e)),
            }
            let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(internal)?;
            let usage = crate::storage_usage::read_sibling(&db)?;
            text += u128::from(usage.text_bytes);
            binary += u128::from(usage.binary_bytes);
        }
        Ok((text, binary))
    }
}
