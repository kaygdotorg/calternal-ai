//! Short-lived viewer-scoped references to sanitized content.

use calternal_types::{AccountId, Owner, SpaceId};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

use super::sanitize::CleanContent;
use crate::plugins::Session;

const SHARDS: usize = 16;
const PER_SHARD: usize = 4;
pub(super) const LIFETIME: Duration = Duration::from_secs(300);

/// The stored revision behind a sealed frame.
#[derive(Clone)]
pub(super) struct Source {
    pub(super) account: AccountId,
    pub(super) session: Session,
    pub(super) space: SpaceId,
    pub(super) plugin: String,
    pub(super) package_revision: u32,
    pub(super) activation_generation: u32,
    pub(super) kind: String,
    pub(super) item: Uuid,
    pub(super) revision: String,
    pub(super) age: super::eligibility::SourceAge,
}

pub(super) struct Frame {
    pub(super) source: Source,
    pub(super) content: CleanContent,
    /// An explicit viewer action has authorized Ask-mode media for this frame.
    pub(super) load_approved: std::sync::atomic::AtomicBool,
    expires: Instant,
}

/// Bounded in-memory index. One short shard lock per lookup or insert, with
/// no I/O. A cloned entry is only a reference and never authorizes delivery.
#[derive(Default)]
pub(crate) struct Frames([Mutex<VecDeque<(Uuid, Arc<Frame>)>>; SHARDS]);

impl Frames {
    fn bucket(&self, id: Uuid) -> &Mutex<VecDeque<(Uuid, Arc<Frame>)>> {
        &self.0[usize::from(id.as_bytes()[15]) % SHARDS]
    }

    pub(super) fn insert(&self, id: Uuid, source: Source, content: CleanContent) {
        let mut entries = self
            .bucket(id)
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        entries.retain(|(key, frame)| *key != id && frame.expires > now);
        if entries.len() == PER_SHARD {
            entries.pop_front();
        }
        entries.push_back((
            id,
            Arc::new(Frame {
                source,
                content,
                load_approved: Default::default(),
                expires: now + LIFETIME,
            }),
        ));
    }

    pub(super) fn get(&self, id: Uuid) -> Option<Arc<Frame>> {
        let entries = self
            .bucket(id)
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let now = Instant::now();
        entries
            .iter()
            .find(|(key, frame)| *key == id && frame.expires > now)
            .map(|(_, frame)| frame.clone())
    }

    /// Release prepared references after the owner lifecycle has been closed
    /// and drained. At most 64 entries are examined.
    pub(crate) fn erase(&self, owner: Owner) {
        for shard in &self.0 {
            shard
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .retain(|(_, frame)| match owner {
                    Owner::Account(id) => frame.source.account != id,
                    Owner::Space(id) => frame.source.space != id,
                });
        }
    }
}
