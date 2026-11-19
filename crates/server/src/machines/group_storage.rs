//! Durable operator allocations shared by the content writers of a group.
use calternal_content::{Content, StorageLimits};
use calternal_types::{Error, GroupId, SpaceId};
use rusqlite::params;

use super::{MachineService, sql};

impl MachineService {
    /// Restore the durable storage rules before accepting content mutations.
    /// Reads the machine policy tables once, then releases their database lock.
    pub fn restore_group_storage(&self, content: &Content) -> Result<(), Error> {
        let state = self.lock()?;
        let memberships = {
            let mut result = Vec::new();
            let mut statement = state
                .db
                .prepare("SELECT space,group_id FROM space_resource_groups")
                .map_err(sql)?;
            let mut rows = statement.query([]).map_err(sql)?;
            while let Some(row) = rows.next().map_err(sql)? {
                result.push((
                    SpaceId::parse(&row.get::<_, String>(0).map_err(sql)?)?,
                    GroupId::parse(&row.get::<_, String>(1).map_err(sql)?)?,
                ));
            }
            result
        };
        let limits = {
            let mut result = Vec::new();
            let mut statement = state
                .db
                .prepare(
                    "SELECT group_id,text_bytes,binary_bytes,total_bytes FROM group_storage_limits",
                )
                .map_err(sql)?;
            let mut rows = statement.query([]).map_err(sql)?;
            while let Some(row) = rows.next().map_err(sql)? {
                let bytes = |column| {
                    row.get::<_, Option<i64>>(column)
                        .map_err(sql)?
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| Error::internal("invalid stored storage ceiling"))
                };
                result.push((
                    GroupId::parse(&row.get::<_, String>(0).map_err(sql)?)?,
                    StorageLimits {
                        text_bytes: bytes(1)?,
                        binary_bytes: bytes(2)?,
                        total_bytes: bytes(3)?,
                    },
                ));
            }
            result
        };
        drop(state);
        content.restore_group_storage(memberships, limits);
        Ok(())
    }

    pub(super) fn set_group_storage(
        &self,
        group: GroupId,
        limits: StorageLimits,
    ) -> Result<(), Error> {
        let bytes = |value: Option<u64>| {
            value
                .map(i64::try_from)
                .transpose()
                .map_err(|_| Error::invalid_input("storage byte ceiling is too large"))
        };
        let text = bytes(limits.text_bytes)?;
        let binary = bytes(limits.binary_bytes)?;
        let total = bytes(limits.total_bytes)?;
        let state = self.lock()?;
        if limits == StorageLimits::default() {
            state
                .db
                .execute(
                    "DELETE FROM group_storage_limits WHERE group_id=?1",
                    [group.to_string()],
                )
                .map_err(sql)?;
        } else {
            state
                .db
                .execute(
                    "INSERT INTO group_storage_limits VALUES(?1,?2,?3,?4)
                 ON CONFLICT(group_id) DO UPDATE SET text_bytes=excluded.text_bytes,
                 binary_bytes=excluded.binary_bytes,total_bytes=excluded.total_bytes",
                    params![group.to_string(), text, binary, total],
                )
                .map_err(sql)?;
        }
        Ok(())
    }
}
