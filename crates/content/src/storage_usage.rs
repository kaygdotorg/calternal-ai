//! Committed current-body byte totals maintained on the same transactions as revisions.

use crate::{Content, Result, SpaceId, types::internal};
use serde::{Deserialize, Serialize};

/// Logical bytes stored by the current heads in one space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageUsage {
    /// Body bytes of current Markdown heads, including recoverable Trash.
    pub text_bytes: u64,
    /// Body bytes of current opaque heads, including recoverable Trash.
    pub binary_bytes: u64,
}

/// Explicit byte ceilings for the committed current heads of one space.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorageLimits {
    /// Maximum logical text bytes, or no configured ceiling.
    pub text_bytes: Option<u64>,
    /// Maximum logical binary bytes, or no configured ceiling.
    pub binary_bytes: Option<u64>,
}

impl Content {
    /// Read committed body bytes without scanning files or loading item bodies.
    /// The caller must authorize the space. One indexed read under the space lock
    /// on a warm open plus one stat. A cold open can run content recovery.
    /// An empty space reports zero without creating a database.
    pub fn storage_usage(&self, space: SpaceId) -> Result<StorageUsage> {
        let _step = self.lifecycle.step(space.as_uuid())?;
        if !crate::db::space_path(&self.roots.appdata, space)
            .try_exists()
            .map_err(internal)?
        {
            return Ok(StorageUsage::default());
        }
        self.with_space(space, |space| read(&space.conn))
    }

    /// Change a space's byte ceilings without evicting existing data. The
    /// operator must have authorized this space before calling. One FULL update
    /// under the existing space lock, plus ordinary costs on a cold open.
    pub fn set_storage_limits(&self, space: SpaceId, limits: StorageLimits) -> Result<()> {
        fn to_sql(bytes: Option<u64>) -> Result<Option<i64>> {
            bytes
                .map(|bytes| {
                    i64::try_from(bytes).map_err(|_| {
                        crate::Error::invalid_input("storage byte ceiling is too large")
                    })
                })
                .transpose()
        }
        let text = to_sql(limits.text_bytes)?;
        let binary = to_sql(limits.binary_bytes)?;
        self.with_space(space, |space| {
            space
                .conn
                .execute(
                    "UPDATE storage_usage SET text_limit=?1,binary_limit=?2 WHERE singleton=1",
                    rusqlite::params![text, binary],
                )
                .map_err(internal)?;
            Ok(())
        })
    }
}

pub(crate) fn read(conn: &rusqlite::Connection) -> Result<StorageUsage> {
    let (text, binary): (i64, i64) = conn
        .query_row(
            "SELECT text_bytes,binary_bytes FROM storage_usage WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(internal)?;
    Ok(StorageUsage {
        text_bytes: u64::try_from(text).map_err(internal)?,
        binary_bytes: u64::try_from(binary).map_err(internal)?,
    })
}

pub(crate) fn write_error(error: rusqlite::Error) -> crate::Error {
    let kind = match &error {
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message == "text storage byte limit" =>
        {
            "text"
        }
        rusqlite::Error::SqliteFailure(_, Some(message))
            if message == "binary storage byte limit" =>
        {
            "binary"
        }
        _ => return internal(error),
    };
    crate::Error::conflict(format!("{kind} storage byte limit reached"))
        .with_details(serde_json::json!({"kind":kind}))
}

pub(crate) const MIGRATION: &str = include_str!("storage_usage.sql");
