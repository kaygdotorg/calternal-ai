/// One second of burst, stored as thousandths of a byte or frame to preserve
/// fractional refills between commits. The clock cannot refill on reversal.
#[derive(Clone, Copy)]
pub(super) struct Bucket {
    pub(super) ms: i64,
    pub(super) bytes: i64,
    pub(super) frames: i64,
}

#[derive(Clone, Copy)]
pub(super) struct Capacity {
    bytes: i64,
    frames: i64,
}

impl Capacity {
    pub(super) fn new(bytes: i64, frames: i64) -> Self {
        Self { bytes, frames }
    }
}

impl Bucket {
    pub(super) fn charge(self, capacity: Capacity, now: i64, bytes: i64) -> Option<Self> {
        let elapsed = now.saturating_sub(self.ms).clamp(0, 1000);
        let bytes =
            (self.bytes + elapsed * capacity.bytes).min(capacity.bytes * 1000) - bytes * 1000;
        let frames = (self.frames + elapsed * capacity.frames).min(capacity.frames * 1000) - 1000;
        (bytes >= 0 && frames >= 0).then_some(Self {
            ms: self.ms.max(now),
            bytes,
            frames,
        })
    }
}
