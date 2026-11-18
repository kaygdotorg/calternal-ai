//! Per-item room admission, subscriber lifetimes and journal fan-out.
use calternal_content::{Content, RoomSnapshot};
use calternal_types::{AccountId, Error, SpaceId};
use std::{
    cell::Cell,
    collections::{BTreeSet, HashMap},
    sync::{Arc, Mutex, Weak},
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

const ROOMS_PER_SPACE: usize = 32;
const PARTICIPANTS: usize = 32;
const MEMORY: usize = 512 * 1024 * 1024;
const CACHE_RESERVATION: u32 = 24 * 1024 * 1024;
const FRAMES: usize = 64;
const IDLE: Duration = Duration::from_secs(60);

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
    rooms: Mutex<HashMap<SpaceId, SpaceRooms>>,
    memory: Arc<Semaphore>,
}

impl Default for Hub {
    fn default() -> Self {
        Self(Arc::new(Inner {
            rooms: Mutex::new(HashMap::new()),
            memory: Arc::new(Semaphore::new(MEMORY)),
        }))
    }
}

struct Cache {
    lineage: String,
    state: Vec<u8>,
}

struct Participant {
    account: AccountId,
    kicked: watch::Sender<bool>,
    presence: Option<(ClientID, AwarenessUpdateEntry)>,
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
        if participants
            .get(&self.member)
            .is_none_or(|member| *member.kicked.borrow())
        {
            return Err(Error::forbidden("room access was revoked"));
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
    }
}

pub(super) struct Joined {
    pub(super) lease: Lease,
    pub(super) lineage: String,
    pub(super) kicked: watch::Receiver<bool>,
    pub(super) frames: broadcast::Receiver<Arc<Frame>>,
    pub(super) initial: Vec<Vec<u8>>,
}

impl Hub {
    pub(super) fn memory(&self) -> &Arc<Semaphore> {
        &self.0.memory
    }

    fn room(&self, key: &Key, account: AccountId) -> Result<(Lease, watch::Receiver<bool>), Error> {
        let busy = || Error::unavailable("live room capacity is busy; retry shortly");
        let mut rooms = self.0.rooms.lock().unwrap();
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
        let (kick, kicked) = watch::channel(false);
        members.insert(
            id,
            Participant {
                account,
                kicked: kick,
                presence: None,
            },
        );
        drop(members);
        room.activity.notify_one();
        Ok((Lease { room, id }, kicked))
    }

    pub(super) async fn join(
        &self,
        content: Arc<Content>,
        key: Key,
        account: AccountId,
        lineage: Option<&str>,
    ) -> Result<Joined, Error> {
        let (lease, kicked) = self.room(&key, account)?;
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
                let (cache, initial) = watch::channel(Some(Arc::new(Cache::from(snapshot))));
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
        let initial = vec![
            Message::Sync(SyncMessage::SyncStep1(state_vector(&cache.state)?)).encode_v1(),
            Message::Sync(SyncMessage::SyncStep2(cache.state.clone())).encode_v1(),
            lease.presence_snapshot(),
        ];
        Ok(Joined {
            lease,
            lineage: cache.lineage.clone(),
            kicked,
            frames,
            initial,
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
            .rooms
            .lock()
            .unwrap()
            .get(&space)
            .and_then(|s| s.get(&(kind.to_owned(), item)))
            .cloned();
        if let Some(room) = room {
            for member in room.members.lock().unwrap().values() {
                if member.account == account {
                    member.kicked.send_replace(true);
                }
            }
        }
    }
}

impl Inner {
    fn evict_empty(&self, room: &Room) -> bool {
        let mut rooms = self.rooms.lock().unwrap();
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

    pub(super) fn presence_snapshot(&self) -> Vec<u8> {
        let clients = self
            .room
            .members
            .lock()
            .unwrap()
            .values()
            .filter_map(|member| member.presence.clone())
            .collect();
        Message::Awareness(AwarenessUpdate { clients }).encode_v1()
    }

    pub(super) fn awareness(&self, update: AwarenessUpdate) -> Result<(), Error> {
        let invalid = || Error::invalid_input("invalid room awareness");
        if update.clients.len() != 1 {
            return Err(invalid());
        }
        let (client, entry) = update.clients.into_iter().next().ok_or_else(invalid)?;
        if client.get() > 9_007_199_254_740_991
            || entry.clock == u32::MAX
            || entry.json.len() > 2048
        {
            return Err(invalid());
        }
        let value: serde_json::Value = serde_json::from_str(&entry.json).map_err(|_| invalid())?;
        if !value.is_null() && !value.is_object() {
            return Err(invalid());
        }
        let mut members = self.room.members.lock().unwrap();
        if members.iter().any(|(id, member)| {
            *id != self.id
                && member
                    .presence
                    .as_ref()
                    .is_some_and(|(existing, _)| *existing == client)
        }) {
            return Err(invalid());
        }
        let participant = members.get_mut(&self.id).ok_or_else(closed)?;
        if let Some((existing, previous)) = &participant.presence {
            if *existing != client {
                return Err(invalid());
            }
            if previous.clock >= entry.clock {
                return Ok(());
            }
        }
        let json = if value.is_null() {
            "null".into()
        } else {
            // The presence identity comes from the authenticated host session.
            serde_json::json!({"account_id":participant.account,"cursor":value.get("cursor")})
                .to_string()
                .into()
        };
        let entry = AwarenessUpdateEntry {
            clock: entry.clock,
            json,
        };
        participant.presence = Some((client, entry.clone()));
        drop(members);
        self.room.send(
            Message::Awareness(AwarenessUpdate {
                clients: HashMap::from([(client, entry)]),
            })
            .encode_v1(),
        )
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

    async fn follow(
        self: Arc<Self>,
        hub: Weak<Inner>,
        content: Arc<Content>,
        cache: watch::Sender<Option<Arc<Cache>>>,
        (_journal, mut events): (
            calternal_store::SpaceStore,
            broadcast::Receiver<Arc<calternal_store::JournalEntry>>,
        ),
    ) {
        let mut idle = Instant::now() + IDLE;
        let mut durability = self.durability.subscribe();
        loop {
            if durability.borrow_and_update().failed {
                break;
            }
            let vacant = self.members.lock().unwrap().is_empty();
            tokio::select! {
                _ = durability.changed() => {}
                _ = self.activity.notified() => { idle = Instant::now() + IDLE; }
                _ = tokio::time::sleep_until(idle), if vacant => {
                    if hub.upgrade().is_none_or(|hub| hub.evict_empty(&self)) { break; }
                    idle = Instant::now() + IDLE;
                }
                entry = events.recv() => {
                    match entry {
                        Ok(entry) if entry.item != self.key.item.to_string() => continue,
                        Err(broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                    let content = content.clone();
                    let key = self.key.clone();
                    let Ok(snapshot) = disk(move || content.room_snapshot(key.space, &key.kind, key.item)).await else { break };
                    let old = cache.borrow().clone().expect("active cache");
                    if old.lineage != snapshot.lineage { break; }
                    if old.state == snapshot.update { continue; }
                    let Ok(update) = update_since(&snapshot.update, &old.state) else { break };
                    cache.send_replace(Some(Arc::new(Cache::from(snapshot))));
                    if self.send(update).is_err() { break; }
                }
            }
        }
        cache.send_replace(None);
        for member in self.members.lock().unwrap().values() {
            member.kicked.send_replace(true);
        }
        if let Some(hub) = hub.upgrade() {
            hub.remove_closed(&self);
        }
    }
}

impl Inner {
    fn remove_closed(&self, room: &Room) {
        let mut rooms = self.rooms.lock().unwrap();
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

impl From<RoomSnapshot> for Cache {
    fn from(snapshot: RoomSnapshot) -> Self {
        Self {
            lineage: snapshot.lineage,
            state: snapshot.update,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn last_grant_eviction_fences_preview_for_just_the_affected_account() {
        let hub = Hub::default();
        let alice = AccountId::new();
        let bob = AccountId::new();
        let key = Key {
            space: SpaceId::new(),
            kind: "documents".into(),
            item: Uuid::new_v4(),
        };
        let (alice, mut cut) = {
            let (lease, kicked) = hub.room(&key, alice).unwrap();
            hub.evict_item_account(key.space, &key.kind, key.item, alice);
            (lease, kicked)
        };
        let (_bob, other) = hub.room(&key, bob).unwrap();
        cut.changed().await.unwrap();
        assert!(*cut.borrow());
        assert!(!*other.borrow());
        assert!(alice.preview().publish(&[0, 0]).is_err());
    }
}
