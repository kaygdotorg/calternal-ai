//! Real thread sends queue from machine telemetry and resume after release.
use super::*;
use crate::requests_cases::Machine;

#[tokio::test]
async fn memory_admission_queues_the_next_thread_until_the_machine_reclaims_a_lease() {
    let f = Fixture::new().await;
    let mut machine = Machine::enroll(&f).await;
    let send = || async {
        let created = f
            .op(
                1,
                "threads.create",
                json!({"space_id":f.spaces[0],"title":"Memory fixture"}),
            )
            .await;
        created.ok();
        let id = created.body["data"]["thread"]["id"].clone();
        f.op(
            1,
            "threads.send",
            json!({"id":id,"message":"Hold this turn"}),
        )
        .await
        .ok();
        id
    };

    let first = send().await;
    let mut admitted = Vec::new();
    // Five admitted turns exercise removal of the server's four-run ceiling.
    for index in 0..5 {
        if index != 0 {
            send().await;
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let frames = machine
                .try_sync_with_memory(&f, json!([]), 10, Some(10 << 30))
                .await
                .unwrap();
            if let Some(exec) = frames.into_iter().find(|frame| frame["type"] == "exec") {
                admitted.push(exec["lease_id"].clone());
                break;
            }
            assert!(Instant::now() < deadline, "turn {index} was not admitted");
        }
    }

    let queued = send().await;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let got = f.op(1, "threads.get", json!({"id":queued})).await;
        got.ok();
        let reason = &got.body["data"]["thread"]["turn_reason"];
        if reason["code"] == "waiting_for_memory" {
            assert_eq!(reason["queue_position"], 1);
            assert!(reason["message"].as_str().unwrap().contains("memory"));
            break;
        }
        assert!(Instant::now() < deadline, "{reason}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    f.op(1, "threads.stop", json!({"id":first})).await.ok();
    let frames = machine
        .try_sync_with_memory(&f, json!([]), 10, Some(10 << 30))
        .await
        .unwrap();
    assert!(
        frames
            .iter()
            .any(|frame| frame["type"] == "stop" && frame["lease_id"] == admitted[0])
    );
    assert!(!frames.iter().any(|frame| frame["type"] == "assign"));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let frames = machine
            .try_sync_with_memory(&f, json!([]), 10, Some(10 << 30))
            .await
            .unwrap();
        if let Some(assign) = frames.iter().find(|frame| frame["type"] == "assign") {
            assert_eq!(assign["spec"]["thread_id"], queued);
            assert!(!frames.iter().any(|frame| frame["type"] == "stop"));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "waiting turn was not admitted after release"
        );
    }
}
