//! Notes adapter over generic, physically per-space content storage.
//!
//! There is no installation-wide state database: plugin enablement is a control
//! value in each space's own `content.db`. `system/content-index.db` holds
//! identifier-only item routes. The content crate owns each UUID space's
//! `content.db`, inherited `index.db` journal/receipts and SSD-tree files.
//! [`Store::open`] validates and locks the three roots through `calternal-roots`
//! before it creates or opens anything, and holds those locks for its lifetime.
use crate::fault::{self, FaultPoint};
use calternal_content::{Content, Item, Request};
use calternal_core::{Note, NoteSummary, NoteVersion};
use calternal_types::Error;
use calternal_types::{AccountId, SpaceId};
use std::time::Duration;
use uuid::Uuid;

/// Host adapter over the generic per-space content service, shared by every request
/// thread without an outer lock: its methods take `&self`.
///
/// Locks: content, plugin enablement and receipts take only the target space's
/// lock (see [`calternal_content`]). The one installation-wide writer left is the
/// catalog's, taken once when a new item is first routed.
pub struct Store {
    /// Shared with live streams and account inputs; never behind a store-wide lock.
    content: std::sync::Arc<Content>,
    /// The appdata root (account-input databases live under it).
    appdata: std::path::PathBuf,
    /// The three roots this store validated and locked.
    roots: calternal_roots::Roots,
    secrets: std::sync::Arc<calternal_secrets::LocalStore>,
    installation: Uuid,
    // Dropped last: every database and file handle above closes first.
    _roots: calternal_roots::RootLocks,
}
/// Replayable account-qualified result; note responses live in their space journal.
pub struct Receipt {
    /// Operation that committed the response.
    pub operation: String,
    /// Canonical operation/input digest.
    pub fingerprint: String,
    /// Previously committed response, private to its acting account.
    pub result: serde_json::Value,
    /// Item whose file placement must finish before replay.
    pub note_id: Option<String>,
}
/// Persisted plugin enablement and asynchronous-result invalidation generation.
#[derive(Debug, Clone)]
pub struct PluginRow {
    /// Whether the plugin is enabled in its owning space.
    pub enabled: bool,
    /// Monotonic generation checked before asynchronous delivery.
    pub generation: u32,
}
/// Current UTC commit timestamp. Cost: clock read and string allocation; no I/O or locks.
pub fn now_rfc3339() -> String {
    calternal_content::now_rfc3339()
}
/// SHA-256 request/byte digest. Cost: O(bytes) CPU and one hex allocation; no locks or I/O.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(ring::digest::digest(&ring::digest::SHA256, bytes))
}
fn io_err(context: &'static str, e: std::io::Error) -> Error {
    tracing::error!(error=%e,context,"io error");
    Error::internal("storage io error")
}
fn content_fault(point: calternal_content::FaultPoint) {
    fault::hit(match point {
        calternal_content::FaultPoint::AfterCommit => FaultPoint::AfterCommit,
        calternal_content::FaultPoint::BeforePlacement => FaultPoint::BeforePlacement,
        calternal_content::FaultPoint::AfterPlacement => FaultPoint::AfterPlacement,
    });
}
/// The control key holding a plugin's enablement in a space.
fn plugin_key(plugin_id: &str) -> String {
    format!("plugin/{plugin_id}")
}
/// Decodes a stored enablement value; anything unreadable is a storage error.
fn plugin_row(value: serde_json::Value) -> Result<PluginRow, Error> {
    Ok(PluginRow {
        enabled: value["enabled"]
            .as_bool()
            .ok_or_else(|| Error::internal("corrupt plugin state"))?,
        generation: value["generation"]
            .as_u64()
            .and_then(|g| u32::try_from(g).ok())
            .ok_or_else(|| Error::internal("corrupt plugin state"))?,
    })
}
/// Parse a note identifier, mapping malformed IDs to the ordinary absence error.
pub(crate) fn item_id(id: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(id).map_err(|_| Error::not_found("item not found"))
}
/// Build a wire snapshot from one current content revision, without file I/O.
pub(crate) fn note(item: &Item) -> Note {
    Note {
        id: item.id.to_string(),
        space_id: item.space.to_string(),
        title: item.metadata["title"].as_str().unwrap_or("").into(),
        markdown: String::from_utf8_lossy(&item.bytes).into_owned(),
        revision: item.revision.clone(),
        updated_at: item.updated_at.clone(),
    }
}
/// Map a selected content head into its wire summary without loading note bytes.
pub(crate) fn note_summary(i: calternal_content::Summary) -> NoteSummary {
    NoteSummary {
        id: i.id.to_string(),
        space_id: i.space.to_string(),
        title: i.metadata["title"].as_str().unwrap_or("").into(),
        revision: i.revision,
        updated_at: i.updated_at,
    }
}
impl Store {
    /// Clone the validated userdata root for bounded instruction reads without holding
    /// a content lock. The caller must authorize the UUID-selected space.
    /// Cost: one path clone; no file I/O or SQLite.
    pub fn instruction_root(&self) -> std::path::PathBuf {
        self.content.userdata_root()
    }

    /// The pinned installation UUID the root markers carry. Cost: a copy.
    pub fn installation(&self) -> Uuid {
        self.installation
    }
    /// The appdata root this store locked. Cost: a borrow.
    pub fn appdata(&self) -> &std::path::Path {
        &self.appdata
    }
    /// Opens the installation's protected security log in userdata-ssd
    /// (`.calternal/recovery`), bound to the pinned installation ID. `create`
    /// provisions its directory at first setup; otherwise a missing log is refused,
    /// since every authority change must reach it before acknowledgement.
    /// The caller attaches it to `auth.db` before any authority change.
    ///
    /// # Errors
    /// Unavailable when the directory is missing (without `create`), unusable,
    /// damaged or bound to another installation.
    ///
    /// Cost: directory checks and the log's open (reads, at most a provisioning fsync).
    pub fn open_recovery(&self, create: bool) -> Result<calternal_recovery::Coordinator, Error> {
        let dir = self.roots.userdata_ssd.join(".calternal/recovery");
        if create {
            calternal_fsio::private::directory(&dir)
                .map_err(|e| Error::unavailable(format!("recovery log directory: {e}")))?;
        } else if !dir.is_dir() {
            return Err(Error::unavailable(
                "the recovery log in userdata-ssd is missing or not a directory",
            ));
        }
        calternal_recovery::Coordinator::open(&dir, self.installation)
            .map_err(|e| Error::unavailable(format!("recovery log: {e}")))
    }
    /// Shared secret store under this appdata root. Cost: one reference-count
    /// increment.
    pub fn secrets(&self) -> std::sync::Arc<calternal_secrets::LocalStore> {
        self.secrets.clone()
    }
    /// Validates and locks the installation's three roots, then opens its stores.
    ///
    /// `installation` is the ID pinned in server configuration, outside every root
    /// (ADR 0062): each root's marker must carry it with that root's role. `create`
    /// allows first setup of empty or missing roots, writing the markers; without
    /// it, missing, swapped, mismatched, foreign or occupied roots are refused
    /// before anything is created (see [`calternal_roots::prepare`]).
    /// `receipt_retention` bounds request receipts in every space (see
    /// [`calternal_content::DEFAULT_RECEIPT_RETENTION`]). The content journal witness lives in
    /// `<appdata>/secrets`; secret encryption keys live beside each owner’s secrets.db.
    ///
    /// # Errors
    /// Unavailable for a refused or locked root, or a store that cannot open.
    ///
    /// Cost: the roots guard (marker reads, three locks), the catalog database open
    /// and schema fsync, then startup-only pending publication and
    /// materialization recovery per space.
    pub fn open(
        roots: &calternal_roots::Roots,
        installation: Uuid,
        create: bool,
        receipt_retention: Duration,
    ) -> Result<Self, Error> {
        let locks = calternal_roots::prepare(roots, installation, create)
            .map_err(|e| Error::unavailable(format!("storage roots refused: {e}")))?;
        let root = roots.appdata.clone();
        calternal_fsio::private::directory(&root).map_err(|e| io_err("create data dir", e))?;
        let secrets = root.join("secrets");
        let content = Content::open(
            roots.clone(),
            secrets,
            Duration::from_secs(600),
            receipt_retention,
            content_fault,
        )?;
        // Owner keys are opened on access. The host attaches recovery and validates
        // protected fingerprints before admission through secrets::configure_keys.
        let secrets = calternal_secrets::LocalStore::open(&root)
            .map_err(|e| Error::unavailable(e.to_string()))?;
        Ok(Self {
            content: std::sync::Arc::new(content),
            appdata: root.clone(),
            roots: roots.clone(),
            secrets: std::sync::Arc::new(secrets),
            installation,
            _roots: locks,
        })
    }
    /// Returns the generic item service for trusted host integrations such as
    /// plugin kind/standard-collection provisioning. Every caller must enforce
    /// current authority before opening a selected account or space.
    /// Cost: borrowed reference only; no allocation, locks, SQLite or filesystem I/O.
    pub fn content(&self) -> &Content {
        &self.content
    }
    /// Resolves identifier-only catalog routing for any stored kind. No content is read
    /// and no authority is granted. A malformed ID is simply absent.
    /// Cost: catalog mutex and one indexed read; no membership scan or fsync.
    pub fn locate(&self, kind: &str, id: &str) -> Result<Option<SpaceId>, Error> {
        let Ok(id) = Uuid::parse_str(id) else {
            return Ok(None);
        };
        self.content.locate(kind, id)
    }
    /// Lists bounded Notes metadata in one authorized space. Cost: generic cache/space
    /// locks and O(log N + page) indexed read, allocating at most 1,000 rows; no file I/O.
    pub fn list(&self, space_id: SpaceId, limit: u32) -> Result<Vec<NoteSummary>, Error> {
        self.content
            .list(space_id, "notes", limit)
            .map(|rows| rows.into_iter().map(note_summary).collect())
    }
    /// Gets one Notes item from its selected physical database. A foreign ID is absent.
    /// Cost: generic cache/space locks, indexed head join and body allocation; no fsync.
    pub fn get(&self, space_id: SpaceId, id: &str) -> Result<Note, Error> {
        let item = self.content.get_live(space_id, item_id(id)?)?;
        if item.kind != "notes" {
            return Err(Error::not_found("item not found"));
        }
        Ok(note(&item))
    }
    /// Reads newest immutable versions using an item-bound opaque cursor. Cost:
    /// cache/space locks, bounded indexed reads and at most 1,001 body allocations;
    /// no other-space data, filesystem scans or fsyncs.
    pub fn history(
        &self,
        space_id: SpaceId,
        id: &str,
        limit: u32,
        cursor: Option<&str>,
    ) -> Result<(Vec<NoteVersion>, Option<String>), Error> {
        let (versions, next) =
            self.content
                .history_live(space_id, "notes", item_id(id)?, limit, cursor)?;
        Ok((
            versions
                .into_iter()
                .map(|i| NoteVersion {
                    title: i.metadata["title"].as_str().unwrap_or("").into(),
                    markdown: String::from_utf8_lossy(&i.bytes).into_owned(),
                    revision: i.revision,
                    updated_at: i.updated_at,
                })
                .collect(),
            next,
        ))
    }
    /// Reads `account`'s receipt for `request_id` in `space`, after the caller has
    /// checked current grants there. Request IDs are scoped to (account, space), so
    /// another account or space never sees it. The space's pending intents finish
    /// publication first. Cost: the space's lock and one journal receipt read;
    /// journal fsyncs only during pending replay.
    pub fn receipt(
        &self,
        account: AccountId,
        space: SpaceId,
        request_id: &str,
    ) -> Result<Option<Receipt>, Error> {
        self.content.receipt(space, account, request_id).map(|r| {
            r.map(|r| Receipt {
                operation: r.operation,
                fingerprint: r.fingerprint,
                result: r.result,
                note_id: r.item.map(|id| id.to_string()),
            })
        })
    }
    /// Creates a globally UUIDv7-identified Notes item with a human filename and
    /// frontmatter identity. Cost: generic write allocations/locks, FULL content and
    /// journal commits, fsio file+directory fsync, placement/outbox SQL commits.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        account: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        title: &str,
        markdown: &str,
    ) -> Result<Note, Error> {
        self.content.register_kind(
            space_id,
            "notes",
            calternal_content::Placement::Visible {
                folder: "Notes".into(),
            },
            calternal_content::FileFormat::Markdown,
        )?;
        let request = Request {
            account: account,
            id: request_id,
            operation,
            fingerprint,
        };
        let title = crate::notes::heading_title(markdown).unwrap_or(title);
        calternal_core::validate_title(title)?;
        let item = self.content.write(
            space_id,
            None,
            "notes",
            None,
            crate::notes::metadata(title, markdown)?,
            markdown.as_bytes(),
            request,
            |i| serde_json::json!({"note":note(i)}),
        )?;
        Ok(note(&item))
    }
    /// Updates only the current base revision, preserving external edits/deletes.
    /// Title changes stage the path and link edits in the same content transaction.
    /// Warm content-only saves project tags and links once and load head metadata
    /// without its previous body. Cost: two file hashes, one guarded placement and
    /// the shared revision and receipt commits. A linked rename also reads its
    /// bounded indexed source set.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        account: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        id: &str,
        base_revision: &str,
        title: Option<&str>,
        markdown: &str,
    ) -> Result<Note, Error> {
        let id = item_id(id)?;
        let current = self.content.live_metadata(space_id, "notes", id)?;
        let old_title = current["title"].as_str().unwrap_or("");
        let request = Request {
            account: account,
            id: request_id,
            operation,
            fingerprint,
        };
        let (title, markdown) = match (title, crate::notes::heading_title(markdown)) {
            (Some(title), Some(heading)) if title != heading => (
                title,
                std::borrow::Cow::Owned(crate::notes::rename_heading(markdown, title)),
            ),
            (title, heading) => (
                title.or(heading).unwrap_or(old_title),
                std::borrow::Cow::Borrowed(markdown),
            ),
        };
        calternal_core::validate_title(title)?;
        calternal_core::validate_markdown(&markdown)?;
        let updated = if title == old_title {
            self.content.write(
                space_id,
                Some(id),
                "notes",
                Some(base_revision),
                crate::notes::metadata(title, &markdown)?,
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i)}),
            )?
        } else {
            self.content.write_with_links(
                space_id,
                id,
                "notes",
                base_revision,
                serde_json::json!({"title":title}),
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i)}),
                |item, old, new| crate::notes::retarget(item, old, new, old_title, title, id),
            )?
        };
        Ok(note(&updated))
    }
    /// Finishes checked placement before retry replay. Cost: catalog lookup and
    /// generic materialization locks/reads, with file+directory fsync only if pending.
    pub fn materialize(&self, id: &str) -> Result<(), Error> {
        let space = self
            .locate("notes", id)?
            .ok_or_else(|| Error::not_found("item not found"))?;
        self.content.materialize(space, item_id(id)?)
    }
    /// Records the result of a mutating plugin operation under the caller's request ID,
    /// so a retry replays it through [`Self::receipt`] instead of running the plugin
    /// again. The receipt lives in the space's journal like every other write receipt.
    /// Cost: one per-space journal append with FULL durability.
    pub fn record_plugin_receipt(
        &self,
        account: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        result: serde_json::Value,
    ) -> Result<(), Error> {
        let request = Request {
            account: account,
            id: request_id,
            operation,
            fingerprint,
        };
        self.content.record_receipt(space_id, request, result)
    }
    /// Reads one item of `kind` for a plugin broker. The global route must place it in
    /// `space_id`; a foreign, absent or wrong-kind ID is the same not-found.
    /// Cost: catalog route lookup plus one indexed item read and body allocation.
    pub fn read_item(&self, space_id: SpaceId, kind: &str, id: &str) -> Result<Item, Error> {
        let missing = || Error::not_found("item not found");
        let (space, id) = (space_id, item_id(id)?);
        if self.content.locate(kind, id)? != Some(space) {
            return Err(missing());
        }
        let item = if kind == "notes" {
            self.content.get_live(space, id)?
        } else {
            self.content.get(space, id)?
        };
        if item.kind != kind {
            return Err(missing());
        }
        Ok(item)
    }
    /// Creates (no `id`) or updates (against `base`) one plugin-kind item with a
    /// per-account receipt, so a replayed receipt returns the original item.
    /// The kind must already be registered in the space (plugin enable does that).
    /// Cost: one generic content write: journal and FULL SQL commits, file and
    /// directory fsync.
    #[allow(clippy::too_many_arguments)]
    pub fn write_item(
        &self,
        account: AccountId,
        request_id: &str,
        fingerprint: &str,
        space_id: SpaceId,
        kind: &str,
        id: Option<Uuid>,
        base: Option<&str>,
        metadata: serde_json::Value,
        text: &str,
    ) -> Result<Item, Error> {
        if let Some(id) = id {
            self.read_item(space_id, kind, &id.to_string())?;
        }
        let request = Request {
            account: account,
            id: request_id,
            operation: "plugin.write_item",
            fingerprint,
        };
        self.content.write(
            space_id,
            id,
            kind,
            base,
            metadata,
            text.as_bytes(),
            request,
            |i| serde_json::json!({"id": i.id, "revision": i.revision}),
        )
    }
    /// Provisions a plugin in one space: registers each declared kind's placement
    /// and format, then creates (or returns) each standard collection with its
    /// stable ID. Idempotent; a changed placement for an existing kind is refused.
    /// Cost: one indexed read per kind (plus one FULL insert when new) and, per
    /// absent collection, one durable content write.
    pub fn provision_plugin(
        &self,
        space_id: SpaceId,
        account: AccountId,
        kinds: &[(
            String,
            calternal_content::Placement,
            calternal_content::FileFormat,
        )],
        collections: &[(String, String, String)],
    ) -> Result<(), Error> {
        let space = space_id;
        for (kind, placement, format) in kinds {
            self.content
                .register_kind(space, kind, placement.clone(), format.clone())?;
        }
        if collections.is_empty() {
            return Ok(());
        }
        // Collections are host-owned items with one fixed hidden placement.
        self.content.register_kind(
            space,
            "collections",
            calternal_content::Placement::Hidden {
                plugin_id: "collections".into(),
            },
            calternal_content::FileFormat::Opaque {
                extension: "json".into(),
            },
        )?;
        for (kind, key, title) in collections {
            self.content
                .ensure_standard_collection(space, account, kind, key, title)?;
        }
        Ok(())
    }
    /// Read one space's plugin state without inheriting another space's enablement.
    /// Cost: the space's lock and one indexed read in its `content.db`; no other
    /// space is touched and nothing is written.
    pub fn plugin_row(
        &self,
        space_id: SpaceId,
        plugin_id: &str,
    ) -> Result<Option<PluginRow>, Error> {
        self.content
            .control(space_id, &plugin_key(plugin_id))?
            .map(plugin_row)
            .transpose()
    }

    /// Set enabled (bump generation on change) and record the request's receipt,
    /// as one admission in the space (see [`Content::set_control`]), returning the
    /// response `result` builds from the new row. A retried request replays its
    /// stored response, so a retried old enable can never undo a newer disable.
    /// Cost: the space's lock (this space only), one receipt read, one FULL
    /// content transaction and one FULL journal commit. No user file access.
    #[allow(clippy::too_many_arguments)]
    pub fn set_plugin_enabled(
        &self,
        account: AccountId,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        plugin_id: &str,
        default: PluginRow,
        enabled: bool,
        result: impl FnOnce(&PluginRow) -> serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let request = Request {
            account,
            id: request_id,
            operation,
            fingerprint,
        };
        self.content
            .set_control(space_id, request, &plugin_key(plugin_id), |current| {
                let current = current.map(plugin_row).transpose()?.unwrap_or(default);
                let next = if current.enabled == enabled {
                    current
                } else {
                    PluginRow {
                        enabled,
                        generation: current.generation + 1,
                    }
                };
                let response = result(&next);
                Ok((
                    serde_json::json!({"enabled": next.enabled, "generation": next.generation}),
                    response,
                ))
            })
    }
}

#[path = "store_live.rs"]
mod live_follow;
