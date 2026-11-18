//! Per-item room admission, subscriber lifetimes and journal fan-out.
mod cache;
mod presence;
#[cfg(test)]
mod tests;
use calternal_content::{Content, RoomSnapshot};
use calternal_types::{AccountId, Error, SpaceId};
use std::{
    cell::Cell,
    collections::{BTreeSet, HashMap},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Notify, OwnedSemaphorePermit, Semaphore, broadcast, watch},
    time::Instant,
};
use uuid::Uuid;
use yrs::{
    ClientID, Doc, ReadTxn, StateVector, Transact, Update,
    sync::{
        Message, SyncMessage,
        awareness::{AwarenessUpdate, AwarenessUpdateEntry},
    },
    updates::{decoder::Decode, encoder::Encode},
};

const SHARDS: usize = 64;
const ROOMS_PER_SPACE: usize = 32;
const PARTICIPANTS: usize = 32;
const MEMORY: usize = 512 * 1024 * 1024;
const CACHE_RESERVATION: u32 = 4 * 1024 * 1024;
const FRAMES: usize = 64;
const IDLE: Duration = Duration::from_secs(60);
const SCAN_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    pub(super) space: SpaceId,
    pub(super) kind: String,
    pub(super) item: Uuid,
}

impl Key {
    pub(super) fn parse(value: &str) -> Option<Self> {
        let (space, tail) = value.split_once('/')?;
        let (kind, item) = tail.split_once('/')?;
        if kind.is_empty() || kind.len() > 64 {
            return None;
        }
        let space = SpaceId::parse(space).ok()?;
        let item = Uuid::parse_str(item).ok()?;
        if value != format!("{space}/{kind}/{item}") {
            return None;
        }
        Some(Self {
            space,
            kind: kind.into(),
            item,
        })
    }
}

#[derive(Clone)]
pub(crate) struct Hub(Arc<Inner>);

type SpaceRooms = HashMap<(String, Uuid), Arc<Room>>;

struct Inner {
    rooms: [Mutex<HashMap<SpaceId, SpaceRooms>>; SHARDS],
    memory: Arc<Semaphore>,
    activation_epochs: [AtomicU64; SHARDS],
    item_revision: OnceLock<Arc<AtomicU64>>,
}

impl Default for Hub {
    fn default() -> Self {
        Self(Arc::new(Inner {
            rooms: std::array::from_fn(|_| Mutex::new(HashMap::new())),
            memory: Arc::new(Semaphore::new(MEMORY)),
            activation_epochs: std::array::from_fn(|_| AtomicU64::new(0)),
            item_revision: OnceLock::new(),
        }))
    }
}

struct Cache {
    lineage: String,
    state: Vec<u8>,
    _memory: OwnedSemaphorePermit,
}

struct Participant {
    account: AccountId,
    termination: watch::Sender<Option<Termination>>,
    presence: Option<(ClientID, AwarenessUpdateEntry)>,
}

/// Distinguish a revoked grant from a room that can be reopened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Termination {
    Closed,
    AccessRevoked,
}

impl Termination {
    /// Map connection termination to its caller-safe reason without I/O.
    pub(super) fn error(self) -> Error {
        match self {
            Self::Closed => closed(),
            Self::AccessRevoked => super::revoked(super::hidden()),
        }
    }
}

pub(super) struct Room {
    key: Key,
    members: Mutex<HashMap<Uuid, Participant>>,
    activity: Notify,
    cache: tokio::sync::OnceCell<watch::Receiver<Option<Arc<Cache>>>>,
    frames: broadcast::Sender<Arc<Frame>>,
    memory: Arc<Semaphore>,
    _cache_permit: OwnedSemaphorePermit,
    durability: watch::Sender<Durability>,
    next_file_scan: Mutex<Instant>,
}

#[derive(Default)]
struct Durability {
    last: u64,
    pending: BTreeSet<u64>,
    failed: bool,
}

pub(super) struct Preview {
    room: Arc<Room>,
    member: Uuid,
    generation: Cell<Option<u64>>,
}

impl Preview {
    pub(super) fn publish(&self, update: &[u8]) -> Result<(), Error> {
        let participants = self.room.members.lock().unwrap();
        let participant = participants.get(&self.member).ok_or_else(closed)?;
        if let Some(end) = *participant.termination.borrow() {
            return Err(end.error());
        }
        self.room.durability.send_modify(|state| {
            state.last += 1;
            state.pending.insert(state.last);
            self.generation.set(Some(state.last));
        });
        self.room
            .send(Message::Sync(SyncMessage::Update(update.to_vec())).encode_v1())
    }

    pub(super) fn finish(&self, durable: bool) {
        if let Some(generation) = self.generation.take() {
            self.room.durability.send_modify(|state| {
                state.pending.remove(&generation);
                state.failed |= !durable;
            });
        }
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.finish(false);
    }
}

pub(super) struct Frame {
    pub(super) bytes: Vec<u8>,
    _permit: OwnedSemaphorePermit,
}

pub(super) struct Lease {
    room: Arc<Room>,
    hub: Weak<Inner>,
    pub(super) id: Uuid,
}

impl Drop for Lease {
    fn drop(&mut self) {
        let removed = self.room.members.lock().unwrap().remove(&self.id);
        if let Some((client, entry)) = removed.and_then(|member| member.presence) {
            let update = AwarenessUpdate {
                clients: HashMap::from([(
                    client,
                    AwarenessUpdateEntry {
                        clock: entry.clock.saturating_add(1),
                        json: "null".into(),
                    },
                )]),
            };
            let _ = self.room.send(Message::Awareness(update).encode_v1());
        }
        self.room.activity.notify_one();
        if self.room.cache.get().is_none()
            && let Some(hub) = self.hub.upgrade()
        {
            hub.evict_empty(&self.room);
        }
    }
}

pub(super) struct Joined {
    pub(super) lease: Lease,
    pub(super) lineage: String,
    pub(super) termination: watch::Receiver<Option<Termination>>,
    pub(super) frames: broadcast::Receiver<Arc<Frame>>,
    pub(super) initial: Vec<Vec<u8>>,
    _initial_memory: OwnedSemaphorePermit,
}

pub(crate) struct ActivationEdit<'a>(&'a AtomicU64);

impl Drop for ActivationEdit<'_> {
    fn drop(&mut self) {
        // Low bits count overlapping updates to a stripe. The high bits
        // change at each completion, including rollback, invalidating readers.
        self.0.fetch_add((1 << 16) - 1, Ordering::AcqRel);
    }
}

impl Hub {
    pub(crate) fn begin_activation_edit(&self, space: SpaceId) -> ActivationEdit<'_> {
        let epoch = &self.0.activation_epochs[shard(space)];
        epoch.fetch_add(1, Ordering::AcqRel);
        ActivationEdit(epoch)
    }

    pub(super) fn retain_item_revision(&self, revision: Arc<AtomicU64>) -> Result<(), Error> {
        let retained = self.0.item_revision.get_or_init(|| revision.clone());
        if Arc::ptr_eq(retained, &revision) {
            Ok(())
        } else {
            Err(Error::unavailable(
                "room item authority revision changed identity",
            ))
        }
    }

    pub(super) fn item_authority_epoch(&self) -> Option<u64> {
        self.0
            .item_revision
            .get()
            .map(|revision| revision.load(Ordering::Acquire))
    }

    pub(super) fn activation_epoch(&self, space: SpaceId) -> u64 {
        self.0.activation_epochs[shard(space)].load(Ordering::Acquire)
    }

    pub(super) fn memory(&self) -> &Arc<Semaphore> {
        &self.0.memory
    }

    fn room(
        &self,
        key: &Key,
        account: AccountId,
    ) -> Result<(Lease, watch::Receiver<Option<Termination>>), Error> {
        let busy = || Error::unavailable("live room capacity is busy; retry shortly");
        let mut rooms = self.0.spaces(key.space);
        let key_in_space = (key.kind.clone(), key.item);
        let existing = rooms
            .get(&key.space)
            .and_then(|space| space.get(&key_in_space))
            .cloned();
        let room = if let Some(existing) = existing {
            existing
        } else {
            if rooms
                .get(&key.space)
                .is_some_and(|space| space.len() >= ROOMS_PER_SPACE)
            {
                return Err(busy());
            }
            let permit = self
                .0
                .memory
                .clone()
                .try_acquire_many_owned(CACHE_RESERVATION)
                .map_err(|_| busy())?;
            let room = Arc::new(Room {
                key: key.clone(),
                members: Mutex::new(HashMap::new()),
                activity: Notify::new(),
                cache: tokio::sync::OnceCell::new(),
                frames: broadcast::channel(FRAMES).0,
                memory: self.0.memory.clone(),
                _cache_permit: permit,
                durability: watch::channel(Durability::default()).0,
                next_file_scan: Mutex::new(Instant::now() + SCAN_INTERVAL),
            });
            rooms
                .entry(key.space)
                .or_default()
                .insert(key_in_space, room.clone());
            room
        };
        let mut members = room.members.lock().unwrap();
        if members.len() >= PARTICIPANTS {
            return Err(busy());
        }
        let id = Uuid::new_v4();
        let (end, termination) = watch::channel(None);
        members.insert(
            id,
            Participant {
                account,
                termination: end,
                presence: None,
            },
        );
        drop(members);
        room.activity.notify_one();
        Ok((
            Lease {
                room,
                hub: Arc::downgrade(&self.0),
                id,
            },
            termination,
        ))
    }

    pub(super) async fn join(
        &self,
        content: Arc<Content>,
        key: Key,
        account: AccountId,
        lineage: Option<&str>,
    ) -> Result<Joined, Error> {
        let (lease, termination) = self.room(&key, account)?;
        let room = lease.room.clone();
        let frames = room.frames.subscribe();
        let started = room
            .cache
            .get_or_try_init(|| async {
                let opener = content.clone();
                let (snapshot, journal) = disk(move || {
                    let journal = opener.live_journal(key.space)?;
                    let receiver = journal.subscribe();
                    let snapshot = opener.room_snapshot(key.space, &key.kind, key.item)?;
                    Ok((snapshot, (journal, receiver)))
                })
                .await?;
                let cache = Cache::new(snapshot, &room.memory)?;
                let (cache, initial) = watch::channel(Some(Arc::new(cache)));
                tokio::spawn(
                    room.clone()
                        .follow(Arc::downgrade(&self.0), content, cache, journal),
                );
                Ok::<_, Error>(initial)
            })
            .await;
        let cache = match started {
            Ok(cache) => cache.borrow().clone().ok_or_else(closed)?,
            Err(error) => {
                drop(lease);
                self.0.evict_empty(&room);
                return Err(error);
            }
        };
        if lineage.is_some_and(|lineage| lineage != cache.lineage) {
            return Err(Error::conflict("room lineage changed")
                .with_details(serde_json::json!({"reason":"lineage_changed"})));
        }
        let initial_memory = room
            .memory
            .clone()
            .try_acquire_many_owned(
                cache.state.len().saturating_mul(2).saturating_add(16_384) as u32
            )
            .map_err(|_| Error::unavailable("live room initial state capacity is busy"))?;
        let initial = vec![
            Message::Sync(SyncMessage::SyncStep1(state_vector(&cache.state)?)).encode_v1(),
            Message::Sync(SyncMessage::SyncStep2(cache.state.clone())).encode_v1(),
            lease.presence_snapshot(),
        ];
        Ok(Joined {
            lease,
            lineage: cache.lineage.clone(),
            termination,
            frames,
            initial,
            _initial_memory: initial_memory,
        })
    }

    pub(crate) fn evict_item_account(
        &self,
        space: SpaceId,
        kind: &str,
        item: Uuid,
        account: AccountId,
    ) {
        let room = self
            .0
            .spaces(space)
            .get(&space)
            .and_then(|s| s.get(&(kind.to_owned(), item)))
            .cloned();
        if let Some(room) = room {
            for member in room.members.lock().unwrap().values() {
                if member.account == account {
                    member
                        .termination
                        .send_replace(Some(Termination::AccessRevoked));
                }
            }
        }
    }
}

fn shard(space: SpaceId) -> usize {
    usize::from(space.as_uuid().as_bytes()[15]) % SHARDS
}

impl Inner {
    fn spaces(&self, space: SpaceId) -> MutexGuard<'_, HashMap<SpaceId, SpaceRooms>> {
        self.rooms[shard(space)].lock().unwrap()
    }

    fn evict_empty(&self, room: &Room) -> bool {
        let mut rooms = self.spaces(room.key.space);
        if !room.members.lock().unwrap().is_empty() {
            return false;
        }
        if let Some(space) = rooms.get_mut(&room.key.space) {
            let key = (room.key.kind.clone(), room.key.item);
            if space
                .get(&key)
                .is_some_and(|r| std::ptr::eq(r.as_ref(), room))
            {
                space.remove(&key);
            }
            if space.is_empty() {
                rooms.remove(&room.key.space);
            }
        }
        true
    }
}

impl Lease {
    pub(super) fn file_scan_due(&self) -> bool {
        *self.room.next_file_scan.lock().unwrap() <= Instant::now()
    }

    pub(super) fn claim_file_scan(&self) -> bool {
        let mut next = self.room.next_file_scan.lock().unwrap();
        let now = Instant::now();
        if *next > now {
            return false;
        }
        *next = now + SCAN_INTERVAL;
        true
    }

    pub(super) fn preview(&self) -> Preview {
        Preview {
            room: self.room.clone(),
            member: self.id,
            generation: Cell::new(None),
        }
    }

    pub(super) async fn wait_durable(&self) -> Result<(), Error> {
        let mut watch = self.room.durability.subscribe();
        let target = watch.borrow().last;
        loop {
            {
                let progress = watch.borrow_and_update();
                if progress.failed {
                    return Err(closed());
                }
                if progress
                    .pending
                    .first()
                    .is_none_or(|generation| *generation > target)
                {
                    return Ok(());
                }
            }
            watch.changed().await.map_err(|_| closed())?;
        }
    }

    pub(super) fn state(&self, since: Option<&StateVector>) -> Result<Vec<u8>, Error> {
        let cache = self
            .room
            .cache
            .get()
            .ok_or_else(closed)?
            .borrow()
            .clone()
            .ok_or_else(closed)?;
        let doc = load(&cache.state)?;
        let txn = doc.transact();
        Ok(match since {
            Some(since) => {
                Message::Sync(SyncMessage::SyncStep2(txn.encode_diff_v1(since))).encode_v1()
            }
            None => Message::Sync(SyncMessage::SyncStep1(txn.state_vector())).encode_v1(),
        })
    }
}

impl Room {
    fn send(&self, bytes: Vec<u8>) -> Result<(), Error> {
        let permit = self
            .memory
            .clone()
            .try_acquire_many_owned(bytes.len() as u32)
            .map_err(|_| Error::unavailable("live room output capacity is busy"))?;
        let _ = self.frames.send(Arc::new(Frame {
            bytes,
            _permit: permit,
        }));
        Ok(())
    }
}

impl Inner {
    fn remove_closed(&self, room: &Room) {
        let mut rooms = self.spaces(room.key.space);
        if let Some(space) = rooms.get_mut(&room.key.space) {
            let key = (room.key.kind.clone(), room.key.item);
            if space
                .get(&key)
                .is_some_and(|r| std::ptr::eq(r.as_ref(), room))
            {
                space.remove(&key);
            }
            if space.is_empty() {
                rooms.remove(&room.key.space);
            }
        }
    }
}

impl Cache {
    fn new(snapshot: RoomSnapshot, memory: &Arc<Semaphore>) -> Result<Self, Error> {
        // Budget both the saved frame and temporary decoder working space.
        let bytes = snapshot.update.len().saturating_mul(8) as u32;
        let permit = memory
            .clone()
            .try_acquire_many_owned(bytes)
            .map_err(|_| Error::unavailable("live room state capacity is busy"))?;
        Ok(Self {
            lineage: snapshot.lineage,
            state: snapshot.update,
            _memory: permit,
        })
    }
}

fn update_since(state: &[u8], old: &[u8]) -> Result<Vec<u8>, Error> {
    let vector = state_vector(old)?;
    let doc = load(state)?;
    let update = doc.transact().encode_diff_v1(&vector);
    Ok(Message::Sync(SyncMessage::Update(update)).encode_v1())
}

fn state_vector(state: &[u8]) -> Result<StateVector, Error> {
    Ok(load(state)?.transact().state_vector())
}

fn load(state: &[u8]) -> Result<Doc, Error> {
    let doc = Doc::new();
    let update = Update::decode_v1(state).map_err(|_| closed())?;
    doc.transact_mut()
        .apply_update(update)
        .map_err(|_| closed())?;
    Ok(doc)
}

pub(super) fn closed() -> Error {
    Error::unavailable("live room closed; reconnect to retry")
}

pub(super) async fn disk<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, Error> + Send + 'static,
) -> Result<T, Error> {
    crate::blocking::spawn(operation)
        .await
        .map_err(|_| closed())?
}
