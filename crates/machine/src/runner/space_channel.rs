//! Attach the supervisor endpoint to a worker assignment from machine sync.
use super::*;
use crate::protocol::SpaceChannelAssignment;
use calternal_plugin_host::space_channel::SpaceChannel;

impl Runner {
    pub(super) async fn open_space_channel(self: Arc<Self>, binding: SpaceChannelAssignment) {
        let Some(slot) = self.slot(binding.lease_id).filter(|slot| {
            !slot.removed.load(Ordering::Acquire)
                && slot.fence.load(Ordering::Acquire) == binding.fence
                && slot
                    .spec
                    .worker
                    .as_ref()
                    .is_some_and(|worker| worker.space_id == binding.space)
        }) else {
            return;
        };
        self.supervise_channel(&binding, &slot).await;
        // This reservation belongs to a v2 placement and cannot keep its sandbox
        // after the authenticated channel lease ends.
        if slot.fence.load(Ordering::Acquire) == binding.fence {
            Self::close(&slot, false).await;
        }
    }

    async fn supervise_channel(&self, binding: &SpaceChannelAssignment, slot: &Slot) {
        let Ok((io, mut session)) = self.link.upgrade_space(binding).await else {
            return;
        };
        let Ok(channel) = SpaceChannel::runtime(io, binding.space, binding.assignment).await else {
            return;
        };
        if slot.removed.load(Ordering::Acquire)
            || slot.fence.load(Ordering::Acquire) != binding.fence
        {
            return;
        }
        let mut life = channel.lifetime();
        tokio::select! {
            _ = session.changed() => channel.fence(),
            _ = life.wait_for(|live| !*live) => {}
        }
    }
}
