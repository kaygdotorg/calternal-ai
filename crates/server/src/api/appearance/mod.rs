//! Signed-in per-account appearance and installation-owned font downloads.
mod catalogue;
mod db;
mod face;
mod http;
mod prefs;
mod remote;

use super::{
    Directory, Identity, Shared, assets,
    cursors::CursorScope,
    dispatch::{decode, to_value},
};
use calternal_core::*;
use calternal_types::{AccountId, Error};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::Path,
    sync::{Arc, Mutex as SyncMutex},
};
use tokio::sync::{Mutex, Semaphore, watch};

pub(crate) use db::SCHEMA;
pub(super) use http::routes;

type Fill = watch::Receiver<Option<Result<(), Error>>>;

pub(crate) struct Appearance {
    storage: db::Storage,
    client: Option<reqwest::Client>,
    catalogue: catalogue::Catalogue,
    installs: Mutex<Installs>,
    fills: Arc<Semaphore>,
    uploads: Arc<Semaphore>,
    publication: SyncMutex<()>,
    // Detached installs must retain the installation root locks through publication.
    _root_guard: Arc<calternal_roots::RootLocks>,
}

struct Installs {
    pending: HashMap<String, Fill>,
    budget: assets::Budget<AccountId>,
}

impl Appearance {
    /// Open private storage, recover interrupted cache publication and load the
    /// operator switch. Called on the server's blocking startup path.
    pub(crate) fn open(
        path: &Path,
        root_guard: Arc<calternal_roots::RootLocks>,
    ) -> Result<Self, Error> {
        let enabled = match std::env::var("CALTERNAL_FONT_DOWNLOADS") {
            Ok(value) if value.eq_ignore_ascii_case("off") => false,
            Ok(value) if value.eq_ignore_ascii_case("on") => true,
            Err(std::env::VarError::NotPresent) => true,
            _ => {
                return Err(Error::invalid_input(
                    "CALTERNAL_FONT_DOWNLOADS is on or off",
                ));
            }
        };
        Ok(Self {
            storage: db::Storage::open(path)?,
            client: enabled.then(remote::client).transpose()?,
            catalogue: catalogue::Catalogue::default(),
            installs: Mutex::new(Installs {
                pending: HashMap::new(),
                budget: assets::Budget::new(),
            }),
            fills: Arc::new(Semaphore::new(2)),
            uploads: Arc::new(Semaphore::new(2)),
            publication: SyncMutex::new(()),
            _root_guard: root_guard,
        })
    }

    /// Remove all account rows after the protected owner fence has drained.
    /// One secure-delete transaction, replayable if erasure was interrupted.
    pub(crate) fn erase(&self, id: AccountId) -> Result<(), Error> {
        self.storage.write(|c| {
            let id = id.to_string();
            c.execute("DELETE FROM uploads WHERE account=?1", [&id])
                .map_err(db_error)?;
            c.execute("DELETE FROM appearance WHERE account=?1", [&id])
                .map_err(db_error)?;
            Ok(())
        })
    }

    fn status_result(&self, state: FontInstallState) -> FontStatusResult {
        FontStatusResult {
            state,
            downloads_enabled: self.client.is_some(),
        }
    }

    async fn local(self: &Arc<Self>, account: AccountId, id: String) -> Result<bool, Error> {
        let service = self.clone();
        run(move || {
            let c = service.storage.connect(false)?;
            Ok(db::family(&c, &account.to_string(), &id)?.is_some())
        })
        .await
    }

    async fn status(
        self: &Arc<Self>,
        account: AccountId,
        id: String,
    ) -> Result<FontStatusResult, Error> {
        validate_id(&id)?;
        let pending = self.installs.lock().await.pending.contains_key(&id);
        let state = if self.local(account, id).await? {
            FontInstallState::Ready
        } else if pending {
            FontInstallState::Installing
        } else {
            FontInstallState::Missing
        };
        Ok(self.status_result(state))
    }

    async fn install(
        self: &Arc<Self>,
        account: AccountId,
        id: String,
    ) -> Result<FontStatusResult, Error> {
        validate_id(&id)?;
        if id.starts_with("upload-") {
            return Err(missing());
        }
        if self.local(account, id.clone()).await? {
            self.storage.touch(&id);
            return Ok(self.status_result(FontInstallState::Ready));
        }
        let mut installs = self.installs.lock().await;
        let mut result = if let Some(existing) = installs.pending.get(&id) {
            existing.clone()
        } else {
            let client = self
                .client
                .clone()
                .ok_or_else(|| Error::forbidden("outside font downloads are disabled"))?;
            let permit = self
                .fills
                .clone()
                .try_acquire_owned()
                .map_err(|_| Error::rate_limited("font installer is busy"))?;
            let (tx, rx) = watch::channel(None);
            installs.pending.insert(id.clone(), rx.clone());
            let owner = self.clone();
            tokio::spawn(async move {
                let result = owner.fill(&client, account, &id).await;
                // Wake waiters before releasing the in-flight slot, with all
                // publication already completed on the durable writer.
                tx.send_replace(Some(result));
                let mut installs = owner.installs.lock().await;
                installs.pending.remove(&id);
                drop(permit);
            });
            rx
        };
        drop(installs);
        result
            .wait_for(Option::is_some)
            .await
            .map_err(|_| Error::internal("font install task stopped"))?
            .as_ref()
            .cloned()
            .expect("observable result")?;
        Ok(self.status_result(FontInstallState::Ready))
    }

    async fn fill(
        self: &Arc<Self>,
        client: &reqwest::Client,
        account: AccountId,
        id: &str,
    ) -> Result<(), Error> {
        // An install may have published between an initial miss and joining the
        // pending map. Recheck before charging the rolling budget.
        if self.local(account, id.to_owned()).await? {
            self.storage.touch(id);
            return Ok(());
        }
        if !self.installs.lock().await.budget.admit(account, 48, 12) {
            return Err(Error::rate_limited("font install rate limit reached"));
        }
        let data = tokio::time::timeout(std::time::Duration::from_secs(300), async {
            let entry = self.catalogue.entry(client, id).await?;
            remote::install(client, entry).await
        })
        .await
        .map_err(|_| Error::unavailable("font download timed out"))??;
        let owner = self.clone();
        run(move || {
            // Publication serializes only cache fills. Account writes and hot
            // reads use independent SQLite connections and never take this lock.
            let _publish = owner.publication.lock().unwrap_or_else(|p| p.into_inner());
            owner.storage.publish(data)
        })
        .await
    }

    fn faces(&self, account: &str, input: FontFacesInput) -> Result<FontFacesResult, Error> {
        let c = self.storage.connect(false)?;
        let preview = input.id.is_some();
        let ids = if let Some(id) = input.id {
            validate_id(&id)?;
            BTreeSet::from([id])
        } else {
            let appearance = prefs::get(&c, account)?;
            prefs::ROLES
                .into_iter()
                .filter_map(|role| prefs::role(&appearance.fonts, role).id.clone())
                .collect()
        };
        let mut css = String::new();
        let mut families = BTreeMap::new();
        for id in ids {
            if let Some(faces) = db::faces(&c, account, &id)? {
                if !id.starts_with("upload-") {
                    self.storage.touch(&id);
                }
                let alias = format!("calternal-{id}");
                for face in faces {
                    face.append_css(&mut css, &id, &alias);
                }
                families.insert(id, alias);
            } else if preview {
                return Err(missing());
            }
        }
        Ok(FontFacesResult { css, families })
    }
}

pub(super) async fn operation<D: Directory>(
    state: Shared<D>,
    identity: Identity,
    op: &str,
    mut input: JsonObject,
) -> Result<Value, Error> {
    let service = state.store.appearance.clone();
    let id = identity.id;
    let cursor = if matches!(op, FONTS_CATALOGUE | FONTS_LIST) {
        Some(CursorScope::open(
            &state.store.cursor_key,
            id,
            op,
            &mut input,
        )?)
    } else {
        None
    };
    let result = match op {
        FONTS_CATALOGUE => to_value(
            service
                .catalogue
                .page(service.client.as_ref(), decode(input)?)
                .await?,
        )?,
        FONTS_STATUS => to_value(service.status(id, decode::<FontIdInput>(input)?.id).await?)?,
        FONTS_INSTALL => to_value(
            service
                .install(id, decode::<FontIdInput>(input)?.id)
                .await?,
        )?,
        _ => {
            let op = op.to_owned();
            run(move || {
                let _step = state.store.content().lifecycle().step(id.as_uuid())?;
                local_op(&service, &id.to_string(), &op, input)
            })
            .await?
        }
    };
    let mut result = result;
    if let Some(cursor) = cursor
        && let Some(next) = result.get_mut("next_cursor")
    {
        cursor.seal(next);
    }
    Ok(result)
}

fn local_op(
    service: &Appearance,
    account: &str,
    op: &str,
    input: JsonObject,
) -> Result<Value, Error> {
    let db = &service.storage;
    match op {
        APPEARANCE_GET => {
            let _: AppearanceGetInput = decode(input)?;
            to_value(prefs::get(&db.connect(false)?, account)?)
        }
        APPEARANCE_PATCH => {
            let patch = decode(input)?;
            to_value(db.write(|c| {
                let mut prefs = prefs::get(c, account)?;
                prefs::merge(&mut prefs, patch)?;
                prefs::save(c, account, &prefs)?;
                Ok(prefs)
            })?)
        }
        FONTS_FACES => to_value(service.faces(account, decode(input)?)?),
        FONTS_LIST => {
            let input: FontListInput = decode(input)?;
            let after = input.cursor.as_deref().unwrap_or("");
            if !after.is_empty() {
                validate_id(after)?;
            }
            to_value(db::list(
                &db.connect(false)?,
                account,
                after,
                page_size(input.limit)?,
            )?)
        }
        FONTS_REMOVE => {
            let input: FontIdInput = decode(input)?;
            validate_id(&input.id)?;
            to_value(db.write(|c| {
                let removed = c
                    .execute(
                        "DELETE FROM uploads WHERE account=?1 AND id=?2",
                        rusqlite::params![account, input.id],
                    )
                    .map_err(db_error)?;
                if removed == 0 {
                    return Err(missing());
                }
                let mut prefs = prefs::get(c, account)?;
                prefs::reset_family(&mut prefs, &input.id);
                prefs::save(c, account, &prefs)?;
                Ok(FontRemoveResult { appearance: prefs })
            })?)
        }
        _ => unreachable!("appearance operation"),
    }
}

fn page_size(limit: Option<u16>) -> Result<usize, Error> {
    let limit = limit.unwrap_or(30);
    if !(1..=100).contains(&limit) {
        return Err(Error::invalid_input("font page limit must be 1–100"));
    }
    Ok(limit.into())
}

fn validate_digest(digest: &str) -> Result<(), Error> {
    if hex_id(digest, 64) {
        Ok(())
    } else {
        Err(missing())
    }
}

fn hex_id(digest: &str, size: usize) -> bool {
    digest.len() == size
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_id(id: &str) -> Result<(), Error> {
    match id.split_once('-') {
        Some(("google" | "fontsource" | "fontshare", key)) if hex_id(key, 64) => Ok(()),
        Some(("upload", key)) if hex_id(key, 32) => Ok(()),
        _ => Err(missing()),
    }
}

fn missing() -> Error {
    Error::not_found("font family or face not found")
}

fn db_error(error: impl std::fmt::Display) -> Error {
    tracing::warn!(%error,"appearance storage failed");
    Error::internal("appearance storage could not be accessed")
}

async fn run<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(f)
        .await
        .map_err(|_| Error::internal("appearance storage task stopped"))?
}
