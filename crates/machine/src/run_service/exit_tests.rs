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
