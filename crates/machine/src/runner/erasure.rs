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
    /// Hold the parking guard through deletion so idle release cannot publish
    /// another snapshot. A failed stop or removal stays queued on the server.
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
        let _parking = self.parking.lock().await;
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
        let success = stopped && self.backend.erase_space_runtime(space).await.is_ok();
        self.discards
            .lock()
            .expect("discards")
            .insert(request_id, success);
    }
}
