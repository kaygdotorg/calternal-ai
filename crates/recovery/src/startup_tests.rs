//! Startup cache reuse must retain replacement detection before authority admission.
use super::*;

#[test]
fn startup_reconciliation_rechecks_changed_snapshot_and_segment() {
    for snapshot in [false, true] {
        let f = Fixture::new();
        f.baseline();
        let coordinator = f.coordinator();
        let path = if snapshot {
            f.root.join("snapshot.json")
        } else {
            log_path(&f)
        };
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[0] ^= 1;
        fsio::write_atomic_mode(&path, &bytes, 0o600).unwrap();
        assert!(
            coordinator.reconcile(&f.connection()).is_err(),
            "changed startup proof must not use cached authority"
        );
    }
}
