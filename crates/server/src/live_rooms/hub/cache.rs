//! Room cache refresh, idle retirement and participant shutdown.
use super::*;

impl Room {
    pub(super) async fn follow(
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
                    let Ok(next) = Cache::new(snapshot, &self.memory) else { break };
                    let Ok(update) = update_since(&next.state, &old.state) else { break };
                    cache.send_replace(Some(Arc::new(next)));
                    if self.send(update).is_err() { break; }
                }
            }
        }
        cache.send_replace(None);
        self.terminate_closed();
        if let Some(hub) = hub.upgrade() {
            hub.remove_closed(&self);
        }
    }

    fn terminate_closed(&self) {
        for member in self.members.lock().unwrap().values() {
            member.termination.send_if_modified(|end| {
                if end.is_some() {
                    return false;
                }
                *end = Some(Termination::Closed);
                true
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_shutdown_preserves_explicit_revocation_and_keeps_other_closures_retriable() {
        let hub = Hub::default();
        let key = Key {
            space: SpaceId::new(),
            kind: "documents".into(),
            item: Uuid::new_v4(),
        };
        let removed = AccountId::new();
        let (_removed, revoked) = hub.room(&key, removed).unwrap();
        let (active, closed) = hub.room(&key, AccountId::new()).unwrap();
        hub.evict_item_account(key.space, &key.kind, key.item, removed);
        active.room.terminate_closed();
        assert_eq!(*revoked.borrow(), Some(Termination::AccessRevoked));
        assert_eq!(*closed.borrow(), Some(Termination::Closed));
        assert!(!crate::live_rooms::access_revoked(
            &Termination::Closed.error()
        ));
    }
}
