//! The process-local room cache fence. It never represents authority itself.

use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};

// Store the active-writer count in bits 1..=20. Bit zero marks any in-flight
// change. The upper bits distinguish every acquisition and retirement.
const WRITER_MASK: u64 = (1 << 20) - 1;
const SHIFT: u32 = 21;
const POISONED: u64 = u64::MAX;

fn revision() -> &'static Arc<AtomicU64> {
    static REVISION: OnceLock<Arc<AtomicU64>> = OnceLock::new();
    REVISION.get_or_init(|| Arc::new(AtomicU64::new(0)))
}

/// Return the one shared epoch for item access in this process. Load with
/// Acquire. An odd value is dirty. Cache a fresh authorization only if the
/// same even value is observed before and after the check. No storage reads.
pub fn item_access_revision_handle() -> Arc<AtomicU64> {
    revision().clone()
}

/// Keep the epoch dirty from before an authority mutation through commit or
/// rollback and publication. Overlapping changes leave it dirty until the
/// last guard drops. Acquisition and retirement use lock-free atomics.
pub struct ItemAccessChange(&'static AtomicU64);

/// Fence an in-process change that may affect a room's item access. The caller
/// holds the guard until the outcome, including any protected publication.
pub fn begin_item_access_change() -> ItemAccessChange {
    let value = revision();
    advance(value, true);
    ItemAccessChange(value)
}

impl Drop for ItemAccessChange {
    fn drop(&mut self) {
        advance(self.0, false);
    }
}

fn advance(value: &AtomicU64, begin: bool) {
    let _ = value.fetch_update(Ordering::AcqRel, Ordering::Acquire, |previous| {
        if previous == POISONED {
            return None;
        }
        let writers = (previous >> 1) & WRITER_MASK;
        let generation = previous >> SHIFT;
        // Saturation fails closed. A room will never reuse such an epoch.
        if generation == (u64::MAX >> SHIFT)
            || (begin && writers == WRITER_MASK)
            || (!begin && writers == 0)
        {
            return Some(POISONED);
        }
        let writers = if begin { writers + 1 } else { writers - 1 };
        Some(((generation + 1) << SHIFT) | (writers << 1) | u64::from(writers != 0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_changes_cannot_publish_a_clean_epoch_early() {
        let epoch = AtomicU64::new(0);
        advance(&epoch, true);
        assert_eq!(epoch.load(Ordering::Acquire) & 1, 1);
        advance(&epoch, true);
        advance(&epoch, false);
        assert_eq!(epoch.load(Ordering::Acquire) & 1, 1);
        advance(&epoch, false);
        let clean = epoch.load(Ordering::Acquire);
        assert_ne!(clean, 0);
        assert_eq!(clean & 1, 0);
        advance(&epoch, true);
        advance(&epoch, false);
        assert_ne!(epoch.load(Ordering::Acquire), clean);
    }
}
