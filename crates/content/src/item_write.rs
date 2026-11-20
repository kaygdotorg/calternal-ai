//! Broker-checked edits sharing the content writer and durable outbox.
use crate::{Content, Item, Request, Result, files, mutations::Destination, rooms::Body};
use calternal_fsio::{beneath::Folder, names};
use calternal_types::{AccountId, Error, ItemRef, RevisionRef, SpaceId};
use serde_json::Value;
use uuid::Uuid;

pub(crate) const SCHEMA: &str = "
    CREATE TABLE item_discovery_protection(
        item TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE
    ) WITHOUT ROWID, STRICT;";

pub(crate) const AUTHORSHIP_SCHEMA: &str = "
    CREATE TABLE checked_item_refs(
        item TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
        reference TEXT NOT NULL UNIQUE
    ) WITHOUT ROWID, STRICT;
    CREATE TABLE item_discovery_creators(
        item TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
        creator TEXT NOT NULL
    ) WITHOUT ROWID, STRICT;
    CREATE TABLE checked_item_authorship(
        revision TEXT PRIMARY KEY REFERENCES versions(revision) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED
    ) WITHOUT ROWID, STRICT;
    CREATE TRIGGER protect_unchecked_item_write AFTER INSERT ON versions
    WHEN EXISTS(SELECT 1 FROM item_discovery_creators WHERE item=NEW.item)
     AND NOT EXISTS(SELECT 1 FROM checked_item_authorship WHERE revision=NEW.revision)
    BEGIN
        INSERT OR IGNORE INTO item_discovery_protection VALUES(NEW.item);
    END;";

/// Host-checked provenance for a write to a kind with per-item discovery.
#[derive(Clone, Copy)]
pub enum DiscoveryWrite {
    /// The installed kind does not opt into item-specific discovery.
    Unmanaged,
    /// The body has unproven authorship or explicitly requires protection.
    Protect,
    /// The owning package writes under the request account's live author scope.
    Checked {
        /// Checked create opted into sole-creator discovery.
        allow_creator_only: bool,
        /// The writer explicitly requested protection on this write.
        protect: bool,
    },
}

impl DiscoveryWrite {
    pub(crate) fn stamp(self, tx: &rusqlite::Transaction<'_>, revision: &str) -> Result<()> {
        if matches!(self, Self::Checked { .. }) {
            tx.execute("INSERT INTO checked_item_authorship VALUES(?1)", [revision])
                .map_err(crate::internal)?;
        }
        Ok(())
    }

    pub(crate) fn record(
        self,
        tx: &rusqlite::Transaction<'_>,
        item: &Item,
        creator: AccountId,
        create: bool,
    ) -> Result<()> {
        let account = creator.to_string();
        let id = item.id.to_string();
        let protect = match self {
            Self::Unmanaged => return Ok(()),
            Self::Protect => true,
            Self::Checked {
                allow_creator_only,
                protect,
            } => {
                if create && allow_creator_only && !protect {
                    tx.execute(
                        "INSERT INTO item_discovery_creators VALUES(?1,?2)",
                        rusqlite::params![id, account],
                    )
                    .map_err(crate::internal)?;
                    return Ok(());
                }
                protect || !tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM item_discovery_creators WHERE item=?1 AND creator=?2)",
                    rusqlite::params![id, account], |row| row.get::<_, bool>(0),
                ).map_err(crate::internal)?
            }
        };
        if protect {
            tx.execute(
                "INSERT OR IGNORE INTO item_discovery_protection VALUES(?1)",
                [id],
            )
            .map_err(crate::internal)?;
        }
        Ok(())
    }
}

/// Child placement selected by the owning package. The writer resolves it
/// below the kind's registered root and reserves the final name in the head.
#[derive(Clone, Copy)]
pub enum CheckedItemPlacement<'a> {
    /// Place a new item below checked components under the registered root.
    Create {
        /// At most four child components, after the registered root.
        folders: &'a [String],
        /// Full readable leaf name, including any extension.
        name: &'a str,
        /// Host-issued opaque identity that cannot be minted from a locator.
        reference: &'a ItemRef,
    },
    /// Keep the current directory. A missing name keeps the existing leaf.
    KeepFolder {
        /// Requested full leaf for this edit.
        name: Option<&'a str>,
    },
}

/// Byte change under the expected revision. An append never merges stale work.
#[derive(Clone, Copy)]
pub enum CheckedItemBody<'a> {
    /// Keep the existing body of a live item, including its room checkpoint.
    Keep,
    /// Replace the entire body with checked bytes.
    Replace(&'a [u8]),
    /// Append at most 64 KiB to the exact expected version of a live item.
    Append(&'a [u8]),
}

pub(crate) struct CheckedWrite<'a> {
    pub(crate) placement: CheckedItemPlacement<'a>,
    pub(crate) body: CheckedItemBody<'a>,
    pub(crate) discovery: DiscoveryWrite,
    pub(crate) validate: &'a dyn Fn(&Value) -> Result<()>,
}

impl Content {
    /// Resolve an opaque ref within the checked space and kind, without
    /// exposing a portable locator as an operand. Wrong kind, foreign,
    /// purged and absent identities share one refusal. One indexed query.
    pub fn resolve_item_ref(
        &self,
        space: SpaceId,
        kind: &str,
        reference: &ItemRef,
    ) -> Result<Uuid> {
        use rusqlite::OptionalExtension;
        self.with_space(space, |s| {
            s.conn
                .prepare_cached(
                    "SELECT i.id FROM checked_item_refs r JOIN items i ON i.id=r.item
                 WHERE r.reference=?1 AND i.kind=?2 AND i.mat_state NOT IN ('purging','purged')
                 AND substr(i.path,1,7) <> '.trash/'",
                )
                .map_err(crate::internal)?
                .query_row(rusqlite::params![reference.0, kind], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(crate::internal)?
                .ok_or_else(crate::missing)?
                .parse()
                .map_err(crate::internal)
        })
    }

    /// Return the independently minted reference for an already checked item.
    /// One indexed read and no body or placement access.
    pub fn checked_item_ref(&self, space: SpaceId, item: Uuid) -> Result<ItemRef> {
        use rusqlite::OptionalExtension;
        self.with_space(space, |s| {
            s.conn
                .prepare_cached("SELECT reference FROM checked_item_refs WHERE item=?1")
                .map_err(crate::internal)?
                .query_row([item.to_string()], |row| row.get(0))
                .optional()
                .map_err(crate::internal)?
                .map(ItemRef)
                .ok_or_else(crate::missing)
        })
    }

    /// Check the creator-only eligibility of a live item in the selected space.
    /// The caller already checked the installed kind opts into item policy and
    /// must separately enforce current attempt and reader authority. Unknown,
    /// protected, foreign and purged items all return false. Call off the async
    /// executor. Cost: indexed head, creator and protection lookups, zero flush.
    pub fn creator_discoverable(
        &self,
        space: SpaceId,
        id: Uuid,
        kind: &str,
        actor: AccountId,
    ) -> Result<bool> {
        self.with_space(space, |s| {
            s.conn
                .prepare_cached(
                    "SELECT EXISTS(
                    SELECT 1 FROM items i JOIN item_discovery_creators a ON a.item=i.id
                    WHERE i.id=?1 AND i.kind=?2 AND a.creator=?3
                    AND i.mat_state NOT IN ('purging','purged')
                    AND substr(i.path, 1, 7) <> '.trash/'
                    AND NOT EXISTS(SELECT 1 FROM item_discovery_protection p WHERE p.item=i.id)
                )",
                )
                .map_err(crate::internal)?
                .query_row(
                    rusqlite::params![id.to_string(), kind, actor.to_string()],
                    |row| row.get(0),
                )
                .map_err(crate::internal)
        })
    }

    /// Apply a checked create, edit or append. Metadata is the full object on
    /// create and an object of updated fields on edit. The broker authorizes
    /// the owning kind, validates and fingerprints the request, and rechecks
    /// authority off the async executor. The guard runs immediately before the
    /// FULL writer transaction is changed and must never reenter content.
    /// The durable receipt and discovery protection share the head's commit
    /// through the existing outbox. Exact receipt replay is handled by the
    /// authorized caller. Cost: ordinary content commit/publication, checked
    /// name allocation when requested and at most one current body read.
    #[allow(clippy::too_many_arguments)]
    pub fn write_checked_item(
        &self,
        space: SpaceId,
        kind: &str,
        item: Option<Uuid>,
        base: Option<&RevisionRef>,
        placement: CheckedItemPlacement<'_>,
        metadata: Value,
        body: CheckedItemBody<'_>,
        discovery: DiscoveryWrite,
        request: Request<'_>,
        guard: impl FnOnce() -> Result<()>,
        validate: impl Fn(&Value) -> Result<()>,
        receipt: impl FnOnce(&Item) -> Value,
    ) -> Result<Item> {
        let create = matches!(placement, CheckedItemPlacement::Create { .. });
        if create != item.is_none()
            || create != base.is_none()
            || (create && !matches!(body, CheckedItemBody::Replace(_)))
            || !metadata.is_object()
        {
            return Err(Error::invalid_input("invalid checked content edit"));
        }
        let bytes = match body {
            CheckedItemBody::Replace(bytes) => bytes,
            CheckedItemBody::Append(bytes) if bytes.len() > 64 * 1024 => {
                return Err(Error::invalid_input("append exceeds 64 KiB"));
            }
            CheckedItemBody::Keep | CheckedItemBody::Append(_) => &[],
        };
        self.write_placed(
            space,
            item,
            kind,
            base.map(|r| r.0.as_str()),
            metadata,
            Body::Replace(bytes),
            request,
            receipt,
            Destination::Current,
            None,
            None,
            None,
            guard,
            |_| Ok(()),
            Some(CheckedWrite {
                placement,
                body,
                discovery,
                validate: &validate,
            }),
        )
    }
}

fn invalid_path() -> Error {
    Error::invalid_input("invalid readable path component")
}

fn path_conflict() -> Error {
    Error::conflict("readable path is occupied")
        .with_details(serde_json::json!({ "reason": "path_conflict" }))
}

fn component(input: &str) -> Result<String> {
    let name = names::nfc(input);
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let device = matches!(
        stem.as_str(),
        "con" | "prn" | "aux" | "nul" | "conin$" | "conout$"
    ) || ["com", "lpt"].iter().any(|prefix| {
        stem.strip_prefix(prefix).is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    });
    if name.is_empty()
        || name.len() > 120
        || name.trim() != name
        || name.starts_with('.')
        || name.ends_with('.')
        || device
        || name.contains(['/', '\\', ':', '<', '>', '"', '|', '?', '*'])
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
    {
        return Err(invalid_path());
    }
    Ok(name)
}

impl crate::db::Space {
    pub(crate) fn checked_path(
        &mut self,
        kind: &str,
        request: CheckedItemPlacement<'_>,
        current: Option<&str>,
    ) -> Result<String> {
        let (folder, name) = match request {
            CheckedItemPlacement::Create { folders, name, .. } => {
                if folders.len() > 4 {
                    return Err(invalid_path());
                }
                let name = component(name)?;
                let (placement, _) = self.registration(kind)?;
                let mut folder = files::folder(&placement, kind);
                for part in folders {
                    folder.push('/');
                    folder.push_str(&component(part)?);
                }
                (folder, name)
            }
            CheckedItemPlacement::KeepFolder { name: None } => {
                return current.map(str::to_owned).ok_or_else(invalid_path);
            }
            CheckedItemPlacement::KeepFolder { name: Some(name) } => {
                let (folder, _) = current
                    .and_then(|path| path.rsplit_once('/'))
                    .ok_or_else(invalid_path)?;
                (folder.to_owned(), component(name)?)
            }
        };
        let root = Folder::open(&self.files).map_err(crate::internal)?;
        root.open_sub(&folder, true).map_err(|_| path_conflict())?;
        if !self.names.contains_key(&folder) && self.names.len() >= crate::db::KIND_CACHE {
            self.names.clear();
        }
        let names = self.names.entry(folder.clone()).or_default();
        names
            .refresh(&self.files.join(&folder))
            .map_err(crate::internal)?;
        let (stem, extension) = name
            .rsplit_once('.')
            .map_or((name.as_str(), ""), |(stem, _)| (stem, &name[stem.len()..]));
        let current_folded = current.map(calternal_fsio::names::fold);
        let mut reserved = self
            .conn
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM items WHERE folded_path=?1 AND path!=COALESCE(?2,''))",
            )
            .map_err(crate::internal)?;
        for number in 1..=32 {
            let leaf = if number == 1 {
                name.clone()
            } else {
                format!("{stem} {number}{extension}")
            };
            let path = format!("{folder}/{leaf}");
            files::ensure_item_path(&path)?;
            let folded = calternal_fsio::names::fold(&path);
            if (current_folded.as_ref() == Some(&folded)
                || !names.taken(&calternal_fsio::names::fold(&leaf)))
                && !reserved
                    .query_row(rusqlite::params![folded, current], |row| {
                        row.get::<_, bool>(0)
                    })
                    .map_err(crate::internal)?
            {
                return Ok(path);
            }
        }
        Err(path_conflict())
    }
}
