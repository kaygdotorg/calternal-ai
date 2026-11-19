//! Apply an acknowledged server erasure under the machine role's sandbox root.
use super::*;

impl Runner {
    pub(super) async fn erase_owner_data(
        &self,
        request_id: Uuid,
        account_id: Uuid,
        thread_id: Option<Uuid>,
    ) {
        if self
            .discards
            .lock()
            .expect("discards")
            .contains_key(&request_id)
        {
            return;
        }
        // Serialize with idle parking. An authenticated sync applies
        // envelopes in order, so new assignments cannot arrive here.
        let _parking = self.parking.lock().await;
        let selected: Vec<_> = self
            .slots
            .lock()
            .expect("slots")
            .iter()
            .filter(|(_, slot)| {
                slot.spec.account_id == account_id
                    && thread_id.is_none_or(|thread| slot.spec.thread_id == thread)
            })
            .map(|(lease, slot)| (*lease, slot.clone()))
            .collect();
        let mut stopped = true;
        for (lease, slot) in selected {
            if Self::close(&slot, slot.spec.worker.is_none()).await {
                self.slots.lock().expect("slots").remove(&lease);
            } else {
                stopped = false;
            }
        }
        let success = stopped && self.backend.erase(account_id, thread_id).await.is_ok();
        self.discards
            .lock()
            .expect("discards")
            .insert(request_id, success);
    }

    /// Stop the matching small VM before attempting physical erasure.
    /// Join the slot's terminal action before erasing under the space gate.
    /// A failed stop or removal stays queued on the server.
    pub(super) async fn erase_space_runtime(
        &self,
        request_id: Uuid,
        space: calternal_types::SpaceId,
    ) {
        if self
            .discards
            .lock()
            .expect("discards")
            .contains_key(&request_id)
        {
            return;
        }
        let mut stopped = true;
        for slot in self
            .all()
            .into_iter()
            .filter(|slot| slot.spec.space_id == Some(space) && slot.is_small_vm())
        {
            if Self::close(&slot, false).await {
                self.slots.lock().expect("slots").remove(&slot.lease_id);
            } else {
                stopped = false;
            }
        }
        let success = stopped && self.erase_space_files(space).await.is_ok();
        self.discards
            .lock()
            .expect("discards")
            .insert(request_id, success);
    }
    async fn erase_space_files(
        &self,
        space: calternal_types::SpaceId,
    ) -> crate::sandbox::SandboxResult<()> {
        let result = self.backend.erase_space_runtime(space).await;
        #[cfg(target_os = "linux")]
        if result.is_err()
            && let Some(root) = &self.space_runtime_root
        {
            return crate::sandbox::microvm::erase_unconfigured(root, space).await;
        }
        result
    }
}
