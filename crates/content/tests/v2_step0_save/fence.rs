//! Authority orderings around the recovery-visible SQLite commit.
use super::*;

// The adapter models the five host invalidation sources at a shared fence.
// Production routes are deliberately excluded from this instrumented adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Invalidation {
    Revocation,
    Cancellation,
    EpochLoss,
    EnrollmentRemoval,
    RunReplacement,
}

#[derive(Default)]
struct Fence {
    state: Mutex<(bool, Option<Invalidation>)>,
    committed: std::sync::Condvar,
}

impl Fence {
    fn admit(&self) -> calternal_content::Result<()> {
        let mut state = self.state.lock().unwrap();
        if state.1.is_some() {
            return Err(calternal_types::Error::forbidden("fixture fence"));
        }
        assert!(!state.0);
        state.0 = true;
        Ok(())
    }

    fn release(&self) {
        let mut state = self.state.lock().unwrap();
        assert!(state.0);
        state.0 = false;
        self.committed.notify_one();
    }

    fn invalidate(&self, cause: Invalidation, attempted: impl FnOnce(bool)) {
        let mut state = self.state.lock().unwrap();
        attempted(state.0);
        while state.0 {
            state = self.committed.wait(state).unwrap();
        }
        assert!(state.1.replace(cause).is_none());
    }

    fn live(&self) -> bool {
        self.state.lock().unwrap().1.is_none()
    }
}

type CommitHook = std::cell::RefCell<Option<Box<dyn FnOnce()>>>;
thread_local! { static AFTER_COMMIT: CommitHook = const { std::cell::RefCell::new(None) }; }

fn fence_hook(point: FaultPoint) {
    if point == FaultPoint::AfterCommit {
        AFTER_COMMIT.with(|hook| {
            if let Some(run) = hook.borrow_mut().take() {
                run();
            }
        });
    }
}

#[test]
#[ignore = "disposable perf fixture; see step 0 measurements"]
fn invalidate_on_both_sides_of_the_commit() {
    use std::sync::Arc;
    let body = body();
    for cause in [
        Invalidation::Revocation,
        Invalidation::Cancellation,
        Invalidation::EpochLoss,
        Invalidation::EnrollmentRemoval,
        Invalidation::RunReplacement,
    ] {
        for after in [false, true] {
            for _ in 0..SAFETY_RUNS {
                let root = tempfile::tempdir().unwrap();
                let space = SpaceId::new();
                let account = AccountId::new();
                let content = open(root.path(), fence_hook);
                setup(&content, space);
                let initial = put(&content, space, account, None, "initial", b"initial");
                let stage = Stage {
                    path: root.path().join("stage"),
                    receipt: Mutex::new(None),
                };
                assert!(stage.begin(*blake3::hash(&body).as_bytes()).unwrap());
                fill(&stage, &body);
                let bytes = stage.finish();
                let fence = Arc::new(Fence::default());
                let (release, signal) = std::sync::mpsc::sync_channel(0);
                let (attempted, observed_attempt) = std::sync::mpsc::sync_channel(0);
                let (ack, observed) = std::sync::mpsc::sync_channel(0);
                let invalidation = Arc::clone(&fence);
                let invalidator = std::thread::spawn(move || {
                    signal.recv().unwrap();
                    invalidation.invalidate(cause, |held| attempted.send(held).unwrap());
                    ack.send(()).unwrap();
                });
                if !after {
                    release.send(()).unwrap();
                    assert!(!observed_attempt.recv().unwrap());
                    observed.recv().unwrap();
                } else {
                    let fence = Arc::clone(&fence);
                    AFTER_COMMIT.with(|hook| {
                        hook.borrow_mut().replace(Box::new(move || {
                            fence.release();
                            observed.recv().unwrap();
                        }))
                    });
                }
                let result = content.write_shared_note(
                    space,
                    initial.id,
                    &initial.revision,
                    initial.metadata,
                    &bytes,
                    request(account, "save"),
                    |item| {
                        // The invalidation thread must actually reach the fence
                        // before SQLite commits. It waits until AfterCommit.
                        release.send(()).unwrap();
                        assert!(observed_attempt.recv().unwrap());
                        assert!(fence.live());
                        json!({"revision":item.revision})
                    },
                    |_| fence.admit(),
                );
                assert_eq!(result.is_ok(), after);
                invalidator.join().unwrap();
                assert!(!fence.live());
                let delivered = fence.live().then_some(result.as_ref().ok());
                assert!(delivered.is_none());
                assert_eq!(
                    content.journal(space).unwrap().len(),
                    if after { 2 } else { 1 }
                );
                let receipt = content.receipt(space, account, "save").unwrap();
                assert_eq!(receipt.is_some(), after);
                assert!(
                    content
                        .receipt(space, AccountId::new(), "save")
                        .unwrap()
                        .is_none()
                );
                if let Some(receipt) = receipt {
                    assert_eq!(
                        receipt.result["revision"],
                        content.live_revision(space, "notes", initial.id).unwrap()
                    );
                }
            }
            println!(
                "fixture_fence cause={cause:?} after_commit={after} n={SAFETY_RUNS} correct={SAFETY_RUNS} extra_events=0 deliveries=0"
            );
        }
    }
}
