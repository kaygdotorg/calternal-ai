//! Platform storage boundaries. Scope every instance to one server and account;
//! credentials never enter these records. No automatic local replica merging.
use crate::ClientError;
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::Mutex,
};

/// Async platform storage result. Implementations move blocking work off workers.
pub type StorageFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ClientError>> + Send + 'a>>;
/// Opaque journal bookmark persistence for one server/account namespace.
/// Cache application must finish before saving a bookmark. Never compare tokens.
pub trait BookmarkStorage: Send + Sync {
    /// Load one space's accepted bookmark. No token means a new subscription.
    fn load<'a>(&'a self, space: &'a str) -> StorageFuture<'a, Option<String>>;
    /// Atomically replace one bookmark; return failure without advancing memory.
    fn save<'a>(&'a self, space: &'a str, bookmark: &'a str) -> StorageFuture<'a, ()>;
}
/// Ephemeral bookmarks, bounded by the subscription's 16 distinct spaces.
#[derive(Default)]
pub struct MemoryBookmarks(Mutex<HashMap<String, String>>);
impl BookmarkStorage for MemoryBookmarks {
    fn load<'a>(&'a self, space: &'a str) -> StorageFuture<'a, Option<String>> {
        Box::pin(async move {
            Ok(self
                .0
                .lock()
                .map_err(|_| ClientError::Protocol("bookmark lock failed"))?
                .get(space)
                .cloned())
        })
    }
    fn save<'a>(&'a self, space: &'a str, bookmark: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            if bookmark.len() > 4000 || bookmark.bytes().any(|b| !b.is_ascii_graphic()) {
                return Err(ClientError::Protocol("invalid bookmark"));
            }
            let mut map = self
                .0
                .lock()
                .map_err(|_| ClientError::Protocol("bookmark lock failed"))?;
            if map.len() >= 16 && !map.contains_key(space) {
                return Err(ClientError::Protocol("bookmark capacity reached"));
            }
            map.insert(space.to_owned(), bookmark.to_owned());
            Ok(())
        })
    }
}
/// CLI bookmark files, in a caller-selected app cache scoped to server/account.
/// Stores one bounded `bookmark: value` file per space using inherited fsio atomic
/// writes. One instance exclusively owns its root, capped at 1024 files (4 MiB).
/// Callers clean up on sign-out/expiry. The per-instance async write mutex spans
/// fsync to serialize the capacity check and publication; it never blocks an
/// executor thread and never serializes different account cache instances.
pub struct FileBookmarks {
    root: PathBuf,
    writes: tokio::sync::Mutex<()>,
}
impl FileBookmarks {
    /// Bind a trusted cache root, without I/O. The caller owns namespace and retention.
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            writes: tokio::sync::Mutex::new(()),
        }
    }
    fn path(&self, space: &str) -> Result<PathBuf, ClientError> {
        if space == "account" {
            return Ok(self.root.join("account.md"));
        }
        let id = uuid::Uuid::parse_str(space)
            .map_err(|_| ClientError::Configuration("space UUID required"))?;
        Ok(self.root.join(format!("{id}.md")))
    }
}
impl BookmarkStorage for FileBookmarks {
    fn load<'a>(&'a self, space: &'a str) -> StorageFuture<'a, Option<String>> {
        Box::pin(async move {
            let path = self.path(space)?;
            tokio::task::spawn_blocking(move || {
                use std::io::Read;
                let file = match std::fs::File::open(path) {
                    Ok(file) => file,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(_) => return Err(ClientError::Protocol("cannot read bookmark")),
                };
                let mut value = String::new();
                file.take(4097)
                    .read_to_string(&mut value)
                    .map_err(|_| ClientError::Protocol("cannot read bookmark"))?;
                if value.len() > 4096 {
                    return Err(ClientError::Protocol("bookmark exceeds limit"));
                }
                let bookmark = value
                    .strip_prefix("bookmark: ")
                    .map(str::trim)
                    .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic()))
                    .ok_or(ClientError::Protocol(
                        "invalid bookmark cache; reload before following",
                    ))?;
                Ok(Some(bookmark.to_owned()))
            })
            .await
            .map_err(|_| ClientError::Protocol("bookmark task failed"))?
        })
    }
    fn save<'a>(&'a self, space: &'a str, bookmark: &'a str) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let path = self.path(space)?;
            if bookmark.len() > 4000 || bookmark.bytes().any(|b| !b.is_ascii_graphic()) {
                return Err(ClientError::Protocol("invalid bookmark"));
            }
            let bytes = format!("bookmark: {bookmark}\n");
            let _write = self.writes.lock().await;
            tokio::task::spawn_blocking(move || {
                let root = path.parent().expect("cache parent");
                let mut builder = std::fs::DirBuilder::new();
                builder.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder
                    .create(root)
                    .map_err(|_| ClientError::Protocol("cannot create bookmark cache"))?;
                if !path.exists()
                    && std::fs::read_dir(root)
                        .map_err(|_| ClientError::Protocol("cannot inspect bookmark cache"))?
                        .take(1024)
                        .count()
                        >= 1024
                {
                    return Err(ClientError::Protocol(
                        "bookmark cache is full; remove unused space bookmarks",
                    ));
                }
                #[cfg(unix)]
                let result = calternal_fsio::write_atomic_mode(&path, bytes.as_bytes(), 0o600);
                #[cfg(not(unix))]
                let result = calternal_fsio::write_atomic(&path, bytes.as_bytes());
                result.map_err(|_| ClientError::Protocol("cannot save bookmark"))
            })
            .await
            .map_err(|_| ClientError::Protocol("bookmark task failed"))?
        })
    }
}
/// Platform-owned persistent item cache. Values are opaque server results, never
/// authoritative state. Implementations define byte/age bounds and account cleanup.
pub trait PersistentCache: Send + Sync {
    /// Read one UUID-keyed item, with no authority implied by a hit.
    fn get<'a>(&'a self, item: &'a str) -> StorageFuture<'a, Option<Value>>;
    /// Store or invalidate one UUID. Must complete before the covering bookmark.
    fn put<'a>(&'a self, item: &'a str, value: Option<Value>) -> StorageFuture<'a, ()>;
}
/// Bounded memory cache with optional platform persistence; access requires &mut,
/// so no locks span I/O. Entries are capped at 1 MiB and evicted in insertion order.
pub struct ItemCache {
    values: HashMap<String, Value>,
    order: VecDeque<String>,
    capacity: usize,
    persistent: Option<Box<dyn PersistentCache>>,
}
impl ItemCache {
    /// Configure 1..1024 slots and optional storage; allocation is initially empty.
    pub fn new(
        capacity: usize,
        persistent: Option<Box<dyn PersistentCache>>,
    ) -> Result<Self, ClientError> {
        if !(1..=1024).contains(&capacity) {
            return Err(ClientError::Configuration(
                "cache capacity must be 1 to 1024",
            ));
        }
        Ok(Self {
            values: HashMap::new(),
            order: VecDeque::new(),
            capacity,
            persistent,
        })
    }
    /// Read memory or the platform cache. O(value bytes), at most one platform read.
    pub async fn get(&self, item: &str) -> Result<Option<Value>, ClientError> {
        if let Some(value) = self.values.get(item) {
            return Ok(Some(value.clone()));
        }
        match &self.persistent {
            Some(p) => p.get(item).await,
            None => Ok(None),
        }
    }
    /// Insert/invalidate an item UUID. O(value bytes), at most one platform write;
    /// no bookmark advances here. Fails before insertion for oversized values.
    pub async fn put(&mut self, item: &str, value: Option<Value>) -> Result<(), ClientError> {
        uuid::Uuid::parse_str(item)
            .map_err(|_| ClientError::Configuration("item UUID required"))?;
        if value
            .as_ref()
            .is_some_and(|v| serde_json::to_vec(v).map_or(true, |b| b.len() > 1024 * 1024))
        {
            return Err(ClientError::Protocol("cache item exceeds limit"));
        }
        if let Some(p) = &self.persistent {
            p.put(item, value.clone()).await?;
        }
        self.order.retain(|id| id != item);
        self.values.remove(item);
        if let Some(value) = value {
            if self.values.len() == self.capacity {
                if let Some(old) = self.order.pop_front() {
                    self.values.remove(&old);
                }
            }
            self.values.insert(item.to_owned(), value);
            self.order.push_back(item.to_owned());
        }
        Ok(())
    }
    /// Discard all memory values on reset or sign-out. Platform storage owns its
    /// own cleanup policy. O(cached bytes) deallocation, no I/O or errors.
    pub fn clear(&mut self) {
        self.values.clear();
        self.order.clear();
    }
}
