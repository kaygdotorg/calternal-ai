//! Remote content fetch timing is chosen by the host, before queue admission.

use calternal_core::ContentFetchPreference;

#[derive(Clone, Copy)]
pub(super) enum SourceAge {
    WithinWindow,
    Older,
    Junk,
}

#[derive(Clone, Copy)]
pub(super) enum Trigger {
    Ingest,
    Open,
    ExplicitLoad,
}

pub(super) fn may_enqueue(
    preference: ContentFetchPreference,
    source_age: SourceAge,
    trigger: Trigger,
) -> bool {
    use ContentFetchPreference::{Always, Ask};
    use SourceAge::{Older, WithinWindow};
    use Trigger::{ExplicitLoad, Ingest, Open};
    matches!(
        (preference, source_age, trigger),
        (Always, WithinWindow, Ingest | ExplicitLoad)
            | (Always, Older, Open | ExplicitLoad)
            | (Ask, WithinWindow | Older, ExplicitLoad)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ContentFetchPreference::{Always, Ask, Never};
    use SourceAge::{Junk, Older, WithinWindow};
    use Trigger::{ExplicitLoad, Ingest, Open};

    #[test]
    fn only_eligible_ingest_backfill_or_explicit_choices_cause_egress() {
        assert!(may_enqueue(Always, WithinWindow, Ingest));
        assert!(!may_enqueue(Always, WithinWindow, Open));
        assert!(!may_enqueue(Always, Older, Ingest));
        assert!(may_enqueue(Always, Older, Open));
        for source in [WithinWindow, Older] {
            assert!(!may_enqueue(Ask, source, Ingest));
            assert!(!may_enqueue(Ask, source, Open));
            assert!(may_enqueue(Ask, source, ExplicitLoad));
            assert!(!may_enqueue(Never, source, ExplicitLoad));
        }
        for trigger in [Ingest, Open, ExplicitLoad] {
            assert!(!may_enqueue(Always, Junk, trigger));
        }
    }
}
