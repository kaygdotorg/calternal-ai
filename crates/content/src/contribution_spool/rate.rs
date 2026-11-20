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

impl Bucket {
    /// Milliseconds until a frame of `bytes` can be charged, at least one.
    /// The refill is capped at one second of burst, so the answer never
    /// exceeds one second for a frame within the pinned frame bound.
    pub(super) fn wait_ms(self, capacity: Capacity, now: i64, bytes: i64) -> i64 {
        let elapsed = now.saturating_sub(self.ms).clamp(0, 1000);
        let have_bytes = (self.bytes + elapsed * capacity.bytes).min(capacity.bytes * 1000);
        let have_frames = (self.frames + elapsed * capacity.frames).min(capacity.frames * 1000);
        let ceil = |need: i64, rate: i64| (need.max(0) + rate - 1) / rate.max(1);
        ceil(bytes * 1000 - have_bytes, capacity.bytes)
            .max(ceil(1000 - have_frames, capacity.frames))
            .clamp(1, 1000)
    }
}
