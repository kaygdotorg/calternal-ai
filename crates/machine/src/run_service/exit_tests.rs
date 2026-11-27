use super::*;

#[tokio::test]
async fn exit_waiter_owns_its_receiver_independently_of_the_log() {
    let (_log, events) = mpsc::channel(1);
    let (signal, exit) = watch::channel(None);
    let handle = RunHandle {
        id: Uuid::now_v7(),
        events,
        exit,
    };
    let waiter = handle.wait_exit();
    drop(handle);
    let expected = ExitInfo {
        code: Some(0),
        ..ExitInfo::default()
    };
    signal.send_replace(Some(expected.clone()));
    assert_eq!(waiter.await, expected);
}

#[test]
fn system_resolver_admission_is_independent_for_each_run() {
    struct Authorizer;
    impl RunAuthorizer for Authorizer {
        fn may_read<'a>(
            &'a self,
            _: &'a Actor,
            _: SpaceId,
            _: RunScope,
            _: &'a [Uuid],
        ) -> BoxFuture<'a, bool> {
            Box::pin(async { false })
        }
    }

    let runner = RunService::new(
        Arc::new(PodmanBackend::new("unused", "unused", "unused")),
        "unused",
        Arc::new(Authorizer),
    );
    let first = runner.resolver_for_run();
    let second = runner.resolver_for_run();
    assert!(!Arc::ptr_eq(&first, &second));
}
