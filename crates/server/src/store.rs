//! Notes adapter over generic, physically per-space content storage.
//!
//! There is no installation-wide state database: plugin enablement is a control
//! value in each space's own `content.db`. `system/content-index.db` holds
//! identifier-only item routes. The content crate owns each UUID space's
//! `content.db`, inherited `index.db` journal/receipts and SSD-tree files.
//! [`Store::open`] validates and locks the three roots through `calternal-roots`
//! before it creates or opens anything, and holds those locks for its lifetime.
mod plugin_items;
pub(crate) use plugin_items::PluginItemWrite;

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
    live_rooms: crate::live_rooms::Hub,
    /// The appdata root (account-input databases live under it).
    appdata: std::path::PathBuf,
    /// The three roots this store validated and locked.
    roots: calternal_roots::Roots,
    secrets: std::sync::Arc<calternal_secrets::LocalStore>,
    installation: Uuid,
    /// Account appearance service and bounded installation font cache.
    pub(crate) appearance: std::sync::Arc<crate::api::appearance::Appearance>,
    machines: std::sync::OnceLock<std::sync::Weak<crate::machines::MachineService>>,
    /// Bounded viewer-scoped references for sealed plugin content.
    pub(crate) plugin_frames: crate::plugins::network::frames::Frames,
    /// Process-lifetime key for authenticated, stateless list cursors.
    pub(crate) cursor_key: ring::hmac::Key,
    // Dropped last: every database and file handle above closes first.
    _roots: std::sync::Arc<calternal_roots::RootLocks>,
}
/// Shared account-qualified durable result, including its associated item UUID.
/// The HTTP replay boundary checks current authority and permanent-deletion state.
pub use calternal_content::Receipt;
/// Persisted plugin enablement and asynchronous-result invalidation generation.
#[derive(Debug, Clone, Copy)]
pub struct PluginRow {
    /// Whether the plugin is enabled in its owning space.
    pub enabled: bool,
    /// Monotonic generation checked before asynchronous delivery.
    pub generation: u32,
    /// Package authority revision approved by the space when it set this state.
    pub approval: u32,
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
        calternal_content::FaultPoint::BeforeRoomCommit => FaultPoint::BeforeRoomCommit,
        calternal_content::FaultPoint::AfterCommit => FaultPoint::AfterCommit,
        calternal_content::FaultPoint::AfterJournalCommit => FaultPoint::AfterJournalCommit,
        calternal_content::FaultPoint::BeforePlacement => FaultPoint::BeforePlacement,
        calternal_content::FaultPoint::AfterPlacement => FaultPoint::AfterPlacement,
    });
}
/// The control key holding a plugin's enablement in a space.
/// Encode a plugin identity as its existing space control key.
pub(crate) fn plugin_key(plugin_id: &str) -> String {
    format!("plugin/{plugin_id}")
}
/// Decodes a stored enablement value; anything unreadable is a storage error.
pub(crate) fn plugin_row(value: serde_json::Value) -> Result<PluginRow, Error> {
    // One control field carries both counters. The low bits identify the space
    // toggle and the high bits identify the approved package authority.
    let stamp = value["generation"]
        .as_u64()
        .ok_or_else(|| Error::internal("corrupt plugin state"))?;
    Ok(PluginRow {
        enabled: value["enabled"]
            .as_bool()
            .ok_or_else(|| Error::internal("corrupt plugin state"))?,
        generation: stamp as u32,
        approval: (stamp >> 32) as u32,
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
    /// The pinned installation UUID the root markers carry. Cost: a copy.
    pub fn installation(&self) -> Uuid {
        self.installation
    }
    /// The appdata root this store locked. Cost: a borrow.
    pub fn appdata(&self) -> &std::path::Path {
        &self.appdata
    }
    /// Borrow the locked roots for resumable owner removal. Cost: no I/O.
    pub(crate) fn roots(&self) -> &calternal_roots::Roots {
        &self.roots
    }

    /// Attach placement needed for owner cleanup before erasure replay.
    /// The weak handle leaves machine ownership with the host runtime.
    pub fn bind_machines(&self, machines: &std::sync::Arc<crate::machines::MachineService>) {
        let _ = self.machines.set(std::sync::Arc::downgrade(machines));
    }

    pub(crate) fn machines(&self) -> Option<std::sync::Arc<crate::machines::MachineService>> {
        self.machines.get().and_then(std::sync::Weak::upgrade)
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
    /// and schema fsync. Per-space publication and materialization recovery
    /// run on first use inside the existing bounded content cache. Call only on
    /// a blocking thread, including during async server startup.
    pub fn open(
        roots: &calternal_roots::Roots,
        installation: Uuid,
        create: bool,
        receipt_retention: Duration,
    ) -> Result<Self, Error> {
        let locks = std::sync::Arc::new(
            calternal_roots::prepare(roots, installation, create)
                .map_err(|e| Error::unavailable(format!("storage roots refused: {e}")))?,
        );
        crate::upgrades::check_start(&roots.appdata, installation)
            .map_err(|error| Error::unavailable(error.to_string()))?;
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
        content.retain_root_guard(locks.clone());
        // Owner keys are opened on access. The host attaches recovery and validates
        // protected fingerprints before admission through secrets::configure_keys.
        let secrets = calternal_secrets::LocalStore::open(&root)
            .map_err(|e| Error::unavailable(e.to_string()))?
            .with_owner_lifecycle(content.lifecycle());
        let cursor_key =
            ring::hmac::Key::generate(ring::hmac::HMAC_SHA256, &ring::rand::SystemRandom::new())
                .map_err(|_| Error::unavailable("cursor entropy unavailable"))?;
        let appearance = std::sync::Arc::new(crate::api::appearance::Appearance::open(
            &root,
            locks.clone(),
        )?);
        Ok(Self {
            cursor_key,
            plugin_frames: Default::default(),
            appearance,
            content: std::sync::Arc::new(content),
            live_rooms: crate::live_rooms::Hub::default(),
            appdata: root.clone(),
            roots: roots.clone(),
            secrets: std::sync::Arc::new(secrets),
            installation,
            machines: std::sync::OnceLock::new(),
            _roots: locks,
        })
    }
    pub(crate) fn live_rooms(&self) -> &crate::live_rooms::Hub {
        &self.live_rooms
    }

    /// Evict all of an account's live sessions on an item after its last
    /// effective read grant is revoked. Call after the grant commits.
    /// Cost: bounded registry and participant lookups, no storage or awaits.
    pub fn evict_live_room_account(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        account: AccountId,
    ) {
        self.live_rooms
            .evict_item_account(space, kind, item, account);
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
        self.content.receipt(space, account, request_id)
    }
    /// Creates a globally UUIDv7-identified Notes item with a human filename and
    /// frontmatter identity. Cost: generic write allocations/locks, FULL content and
    /// journal and outbox-retirement commits. File/directory durability and
    /// replayable placement bookkeeping follow on the existing lifecycle worker.
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        actor: impl Into<calternal_types::Actor>,
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
        let actor = actor.into();
        let request = Request {
            account: actor.account,
            via: actor.via.as_ref(),
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
    /// without its previous body. Cost: one metadata read plus one shared write
    /// header, and two checked file reads (admission and deferred placement).
    /// Three FULL SSD commits precede acknowledgment; file/directory syncs follow
    /// within one second. A linked rename reads its bounded indexed source set
    /// and completes guarded placement synchronously.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &self,
        actor: impl Into<calternal_types::Actor>,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        id: &str,
        base_revision: &str,
        title: Option<&str>,
        markdown: &str,
        allow_link_rewrites: bool,
        shared_authority: Option<&calternal_auth::spaces::Spaces>,
    ) -> Result<Note, Error> {
        let id = item_id(id)?;
        let current = self.content.live_metadata(space_id, "notes", id)?;
        let old_title = current["title"].as_str().unwrap_or("");
        let actor = actor.into();
        let request = Request {
            account: actor.account,
            via: actor.via.as_ref(),
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
        if title != old_title && !allow_link_rewrites {
            return Err(Error::forbidden(
                "renaming a shared note requires space edit access",
            ));
        }
        let updated = if let Some(authority) = shared_authority {
            self.content.write_shared_note(
                space_id,
                id,
                base_revision,
                crate::notes::metadata(title, &markdown)?,
                markdown.as_bytes(),
                request,
                |i| serde_json::json!({"note":note(i)}),
                |collections| {
                    if authority
                        .can_edit_shared_item(actor.account, space_id, id, collections)
                        .map_err(crate::auth::auth_error)?
                    {
                        Ok(())
                    } else {
                        Err(Error::not_found("item not found"))
                    }
                },
            )?
        } else if title == old_title {
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
    /// Provisions a plugin in one space: registers each declared kind's placement
    /// and format, then creates (or returns) each standard collection with its
    /// stable ID. Idempotent; a changed placement for an existing kind is refused.
    /// Cost: one indexed read per kind (plus one FULL insert when new) and, per
    /// absent collection, one durable content write.
    pub fn provision_plugin(
        &self,
        space_id: SpaceId,
        actor: impl Into<calternal_types::Actor>,
        kinds: &[(
            String,
            calternal_content::Placement,
            calternal_content::FileFormat,
            bool,
        )],
        collections: &[(String, String, String)],
    ) -> Result<(), Error> {
        let space = space_id;
        let actor = actor.into();
        for (kind, placement, format, live_editing) in kinds {
            self.content
                .register_kind(space, kind, placement.clone(), format.clone())?;
            if *live_editing {
                self.content.enable_live_editing(space, kind)?;
            }
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
                .ensure_standard_collection(space, actor.clone(), kind, key, title)?;
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
        actor: impl Into<calternal_types::Actor>,
        request_id: &str,
        operation: &str,
        fingerprint: &str,
        space_id: SpaceId,
        plugin_id: &str,
        default: PluginRow,
        enabled: bool,
        result: impl FnOnce(&PluginRow) -> serde_json::Value,
    ) -> Result<serde_json::Value, Error> {
        let actor = actor.into();
        let request = Request {
            account: actor.account,
            via: actor.via.as_ref(),
            id: request_id,
            operation,
            fingerprint,
        };
        let _room_activation_edit = self.live_rooms.begin_activation_edit(space_id);
        self.content
            .set_control(space_id, request, &plugin_key(plugin_id), |current| {
                let current = current.map(plugin_row).transpose()?.unwrap_or(default);
                let next = if current.enabled == enabled && current.approval == default.approval {
                    current
                } else {
                    PluginRow {
                        enabled,
                        generation: current
                            .generation
                            .checked_add(u32::from(current.enabled != enabled))
                            .ok_or_else(|| {
                                Error::internal("plugin activation generation exhausted")
                            })?,
                        approval: default.approval,
                    }
                };
                let response = result(&next);
                Ok((
                    serde_json::json!({"enabled": next.enabled, "generation":
                        (u64::from(next.approval) << 32) | u64::from(next.generation)}),
                    response,
                ))
            })
    }
}

#[path = "store_live.rs"]
mod live_follow;
