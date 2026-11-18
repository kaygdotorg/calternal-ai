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
}
