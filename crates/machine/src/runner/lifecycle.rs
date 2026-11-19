//! Machine lease teardown and idle parking.
use super::*;

impl Runner {
    /// Release idle turn containers after a lease TTL, keeping each logical
    /// lease and its files. Its next exec starts a new container on this machine.
    /// Scans at most the assigned slots; only a parked container incurs engine I/O.
    pub async fn park_idle(&self, idle: Duration) {
        if self.backend.name() == "direct" {
            return;
        }
        // Slow engine cleanup can outlast a poll. Keep at most one idle sweep
        // in progress while the connection continues to renew its leases.
        let Ok(_parking) = self.parking.try_lock() else {
            return;
        };
        for slot in self.all() {
            if slot.spec.worker.is_some() || slot.closed.get().is_some() {
                continue;
            }
            let Ok(mut cell) = slot.sandbox.try_write() else {
                continue;
            };
            let idle_for = slot.last_use.lock().expect("sandbox last use").elapsed();
            if idle_for >= idle
                && let Some(Ok(sb)) = cell.get()
            {
                match sb.release().await {
                    Ok(()) => {
                        cell.take();
                    }
                    Err(e) => {
                        tracing::warn!(lease = %slot.lease_id, error = %e, "idle sandbox removal failed")
                    }
                }
            }
        }
    }

    /// Close machine-owned sandboxes on orderly exit, revocation or lease expiry.
    /// Provider work and homes remain; worker work is transient. On expiry,
    /// closed slots stay as tombstones until the server reconciles them.
    /// Joins in-flight starts. Returns false if any teardown needs a retry.
    pub async fn release_all(&self) -> bool {
        let mut closed = true;
        for slot in self.all() {
            closed &= Self::close(&slot, slot.spec.worker.is_none()).await;
        }
        closed
    }
}
