//! Stored collection records and idempotent standard-collection provisioning.
//! Space collections use ordinary content items. Account collections live only in
//! a protected account database, never inside a shareable space's content file.
use crate::types::{internal, missing};
use crate::{
    AccountId, Collection, CollectionOwner, Content, Error, Request, Result, SpaceId, db,
    journal::lock, now_rfc3339, sha256,
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stored collection's stable identity and versioned membership definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCollection {
    /// Globally fresh UUIDv7, independent from the owner identity.
    pub id: Uuid,
    /// Ownership, display metadata and explicit membership.
    pub collection: Collection,
    /// Opaque revision required for account collection updates.
    pub revision: String,
}
impl Content {
    /// Creates or returns one manifest-declared standard collection when enabling
    /// its kind's plugin. Repeated calls retain its ID and explicit membership.
    /// Plugin lifecycle wiring and the `collections` kind placement registration
    /// belong to the host; creation grants no access.
    /// Cost: indexed metadata lookup under cache/space locks; absent collections
    /// pay one [`Self::write`] including journal, file and directory durability.
    pub fn ensure_standard_collection(
        &self,
        space: SpaceId,
        account: AccountId,
        kind: &str,
        key: &str,
        title: &str,
    ) -> Result<StoredCollection> {
        if key.is_empty() || key.len() > 64 {
            return Err(Error::invalid_input("invalid standard collection key"));
        }
        super::files::validate_kind(kind)?;
        let existing = || {
            self.with_space(space,|s|{
            s.conn.query_row("SELECT id FROM items WHERE kind='collections' AND json_extract(metadata,'$.item_kind')=?1 AND json_extract(metadata,'$.standard_key')=?2",params![kind,key],|r|r.get::<_,String>(0)).optional().map_err(internal)?.map(|id|{
                let item=s.get(Uuid::parse_str(&id).map_err(internal)?)?;
                Ok(StoredCollection {id:item.id,revision:item.revision,collection:serde_json::from_value(item.metadata).map_err(internal)?})
            }).transpose()
        })
        };
        if let Some(collection) = existing()? {
            return Ok(collection);
        }
        let definition = Collection {
            owner: CollectionOwner::Space(space),
            title: title.into(),
            item_kind: kind.into(),
            standard_key: Some(key.into()),
            members: vec![],
        };
        let metadata = serde_json::to_value(&definition).map_err(internal)?;
        let bytes = serde_json::to_vec_pretty(&definition).map_err(internal)?;
        let request = format!("standard/{space}/{kind}/{key}");
        let fingerprint = sha256(&bytes);
        let write = self.write(
            space,
            None,
            "collections",
            None,
            metadata,
            &bytes,
            Request {
                account,
                id: &request,
                operation: "collections.create",
                fingerprint: &fingerprint,
            },
            |i| serde_json::json!({"id":i.id}),
        );
        match write {
            Ok(item) => Ok(StoredCollection {
                id: item.id,
                revision: item.revision,
                collection: definition,
            }),
            Err(e) => existing()?.ok_or(e),
        }
    }
    /// Reads one space collection after the host authorizes that space. Member IDs
    /// are references: the caller must check current access before loading them.
    /// Cost: [`Self::get`] plus bounded metadata deserialization; no extra fsync.
    pub fn collection(&self, space: SpaceId, id: Uuid) -> Result<StoredCollection> {
        let item = self.get(space, id)?;
        if item.kind != "collections" {
            return Err(missing());
        }
        Ok(StoredCollection {
            id,
            revision: item.revision,
            collection: serde_json::from_value(item.metadata).map_err(internal)?,
        })
    }
    /// Replaces explicit membership as one ordinary versioned space-item mutation.
    /// The host authorizes collection editing; membership never grants item access.
    /// Cost: [`Self::get`], bounded JSON allocation and [`Self::write`] durability.
    pub fn set_collection_members(
        &self,
        space: SpaceId,
        id: Uuid,
        base: &str,
        members: Vec<Uuid>,
        request: Request<'_>,
    ) -> Result<StoredCollection> {
        if members.len() > 10_000 {
            return Err(Error::invalid_input("collection membership is too large"));
        }
        let mut collection = self.collection(space, id)?.collection;
        collection.members = members;
        let bytes = serde_json::to_vec_pretty(&collection).map_err(internal)?;
        let item = self.write(
            space,
            Some(id),
            "collections",
            Some(base),
            serde_json::to_value(&collection).map_err(internal)?,
            &bytes,
            request,
            |i| serde_json::json!({"id":i.id,"revision":i.revision}),
        )?;
        Ok(StoredCollection {
            id,
            revision: item.revision,
            collection,
        })
    }
    /// Creates a collection in the selected protected account profile. Cross-space
    /// members remain references requiring their own authority checks. This record
    /// is never placed in a space database and cannot be read through space APIs.
    /// Cost: opens that account's SQLite file, serializes bounded metadata, then
    /// FULL content/catalog transactions; no space scan, journal or file placement.
    /// Account-profile recovery and public resolver transport are host concerns.
    pub fn create_account_collection(
        &self,
        account: AccountId,
        title: &str,
        item_kind: &str,
        members: Vec<Uuid>,
    ) -> Result<StoredCollection> {
        if members.len() > 10_000 || title.len() > 4096 {
            return Err(Error::invalid_input("collection is too large"));
        }
        super::files::validate_kind(item_kind)?;
        let id = Uuid::now_v7();
        let collection = Collection {
            owner: CollectionOwner::Account(account),
            title: title.into(),
            item_kind: item_kind.into(),
            standard_key: None,
            members,
        };
        let revision = format!("v1.{}", Uuid::now_v7().simple());
        let conn = self.account_collection_db(account)?;
        conn.execute(
            "INSERT INTO collections VALUES(?1,?2,?3,?4)",
            params![
                id.to_string(),
                serde_json::to_string(&collection).map_err(internal)?,
                revision,
                now_rfc3339()
            ],
        )
        .map_err(internal)?;
        lock(&self.index)
            .execute(
                "INSERT INTO account_items VALUES(?1,?2,'collections')",
                params![id.to_string(), account.to_string()],
            )
            .map_err(internal)?;
        Ok(StoredCollection {
            id,
            collection,
            revision,
        })
    }
    /// Reads a collection only from the authenticated owner's protected profile.
    /// Cost: one account-local database open, indexed lookup and JSON allocation;
    /// no other account/space database reads or normal fsync.
    pub fn account_collection(&self, account: AccountId, id: Uuid) -> Result<StoredCollection> {
        let conn = self.account_collection_db(account)?;
        conn.query_row(
            "SELECT definition,revision FROM collections WHERE id=?1",
            [id.to_string()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(internal)?
        .map(|(json, revision)| {
            Ok(StoredCollection {
                id,
                revision,
                collection: serde_json::from_str(&json).map_err(internal)?,
            })
        })
        .transpose()?
        .ok_or_else(missing)
    }
    /// Replaces account-owned collection membership with optimistic concurrency.
    /// Only the host-authenticated account selects this profile. Cost: one database
    /// open, indexed conditional update, bounded JSON allocation and FULL SQL fsync.
    pub fn set_account_collection_members(
        &self,
        account: AccountId,
        id: Uuid,
        base: &str,
        members: Vec<Uuid>,
    ) -> Result<StoredCollection> {
        if members.len() > 10_000 {
            return Err(Error::invalid_input("collection membership is too large"));
        }
        let mut stored = self.account_collection(account, id)?;
        stored.collection.members = members;
        stored.revision = Uuid::now_v7().to_string();
        let conn = self.account_collection_db(account)?;
        let changed=conn.execute("UPDATE collections SET definition=?1,revision=?2,updated_at=?3 WHERE id=?4 AND revision=?5",params![serde_json::to_string(&stored.collection).map_err(internal)?,stored.revision,now_rfc3339(),id.to_string(),base]).map_err(internal)?;
        if changed != 1 {
            return Err(Error::new(
                super::ErrorCode::Conflict,
                "collection has a newer revision",
            ));
        }
        Ok(stored)
    }
    fn account_collection_db(&self, account: AccountId) -> Result<rusqlite::Connection> {
        let conn = db::database(
            &self
                .roots
                .appdata
                .join("accounts")
                .join(account.to_string())
                .join("content.db"),
        )?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS binding(account TEXT PRIMARY KEY) STRICT;CREATE TABLE IF NOT EXISTS collections(id TEXT PRIMARY KEY,definition TEXT NOT NULL,revision TEXT NOT NULL,updated_at TEXT NOT NULL) STRICT;").map_err(internal)?;
        conn.execute(
            "INSERT INTO binding SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM binding)",
            [account.to_string()],
        )
        .map_err(internal)?;
        let binding: String = conn
            .query_row("SELECT account FROM binding", [], |r| r.get(0))
            .map_err(internal)?;
        if binding != account.to_string() {
            return Err(internal("wrong account binding"));
        }
        Ok(conn)
    }
}
