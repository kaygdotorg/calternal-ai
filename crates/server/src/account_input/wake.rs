//! Account-scoped wake hints using the journal feed's subscribe-before-replay pattern.
//! Hints carry no data; durable bookmark pages remain the authority on lag/restart.
use calternal_types::AccountId;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};
use tokio::sync::broadcast;

type Senders = Arc<Mutex<HashMap<AccountId, Weak<broadcast::Sender<()>>>>>;

/// Only live account feeds retain senders. The map lock spans no I/O or await.
#[derive(Clone, Default)]
pub(super) struct Wakes(Senders);

/// One active feed retains its account sender and releases the registry entry
/// when the last listener drops; at most one empty hint is buffered.
pub(super) struct Listener {
    account: AccountId,
    map: Senders,
    sender: Arc<broadcast::Sender<()>>,
    /// Coalesced hints carry no content; lag triggers durable bookmark replay.
    pub(super) receiver: broadcast::Receiver<()>,
}
impl Wakes {
    /// Subscribe before reading the durable head/page, closing the empty-read race.
    /// One coalesced hint suffices because followers replay all retained events.
    pub(super) fn listen(&self, account: AccountId) -> Listener {
        let mut map = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let sender = map
            .get(&account)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let sender = Arc::new(broadcast::channel(1).0);
                map.insert(account, Arc::downgrade(&sender));
                sender
            });
        Listener {
            account,
            map: self.0.clone(),
            receiver: sender.subscribe(),
            sender,
        }
    }
    /// Notify only after a successful commit. No listener means no allocation.
    pub(super) fn committed(&self, account: AccountId) {
        let sender = {
            let map = self.0.lock().unwrap_or_else(|p| p.into_inner());
            map.get(&account).and_then(Weak::upgrade)
        };
        if let Some(sender) = sender {
            let _ = sender.send(());
        }
    }
}
impl Drop for Listener {
    fn drop(&mut self) {
        let mut map = self.map.lock().unwrap_or_else(|p| p.into_inner());
        if Arc::strong_count(&self.sender) == 1 {
            map.remove(&self.account);
        }
    }
}
