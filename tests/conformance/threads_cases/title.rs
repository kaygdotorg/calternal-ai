//! HTTP thread and machine isolation for the background title adapter.
use super::*;
use crate::requests_cases::Machine;

const TURN: &str = "eyJ2ZXJzaW9uIjoxLCJzZXEiOjEsImV2ZW50Ijp7InR5cGUiOiJ0ZXh0X2RlbHRhIiwidGV4dCI6IkFzc2lzdGFudCBmaXh0dXJlIn19CnsidmVyc2lvbiI6MSwic2VxIjoyLCJldmVudCI6eyJ0eXBlIjoiZG9uZSIsIm91dGNvbWUiOiJkb25lIn19Cg==";
const TITLE: &str = "eyJ0aXRsZSI6IlRocmVlLVBvdCBCYWxjb255IEdhcmRlbiJ9Cg==";
const MANAGED: &str = "eyJyZWFzb24iOiJtYW5hZ2VkX3BvbGljeSJ9Cg==";

async fn next_exec(f: &Fixture, machine: &mut Machine) -> Value {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(Instant::now() < deadline, "adapter exec was not delivered");
        for frame in machine.sync(f, json!([])).await {
            if frame["type"] == "exec" {
                return frame;
            }
        }
    }
}

async fn reply(f: &Fixture, machine: &Machine, exec: &Value, frame: &str) {
    machine
        .output(
            f,
            exec,
            json!([
                {"kind":"data","seq":1,"stream":"stdout","data":frame},
                {"kind":"exit","seq":2,"code":0}
            ]),
        )
        .await;
}

async fn read(f: &Fixture, id: &Value) -> Value {
    let got = f.op(1, "threads.get", json!({"id":id})).await;
    got.ok();
    got.body["data"]["thread"].clone()
}

async fn run_three(f: &Fixture, machine: &mut Machine, id: &Value) {
    for n in 1..=3 {
        f.op(
            1,
            "threads.send",
            json!({"id":id,"message":format!("Balcony {n}")}),
        )
        .await
        .ok();
        let exec = next_exec(f, machine).await;
        assert_ne!(exec["argv"][1], "--title", "title ran before the last turn");
        reply(f, machine, &exec, TURN).await;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let got = read(f, id).await;
            if got["turn_state"] == "idle" {
                break;
            }
            assert!(Instant::now() < deadline, "{got}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
}

#[tokio::test]
async fn title_runs_privately_and_respects_the_latest_manual_title() {
    let f = Fixture::new().await;
    let space = &f.spaces[0];
    let mut machine = Machine::enroll(&f).await;
    for mode in ["automatic", "manual", "managed"] {
        let created = f.op(1, "threads.create", json!({"space_id":space})).await;
        created.ok();
        let id = &created.body["data"]["thread"]["id"];
        run_three(&f, &mut machine, id).await;
        let exec = next_exec(&f, &mut machine).await;
        assert_eq!(exec["argv"][1], "--title");
        assert_eq!(exec["env"], json!([]));
        let prompt = exec["stdin"].as_str().unwrap();
        assert_eq!(prompt.lines().count(), 5);
        assert!(prompt.contains("Balcony 3"));
        assert_eq!(f.op(2, "threads.get", json!({"id":id})).await.status, 404);
        if mode == "manual" {
            f.op(1, "threads.rename", json!({"id":id,"title":"New thread"}))
                .await
                .ok();
        }
        reply(
            &f,
            &machine,
            &exec,
            if mode == "managed" { MANAGED } else { TITLE },
        )
        .await;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let frames = machine.sync(&f, json!([])).await;
            if frames
                .iter()
                .any(|f| f["type"] == "stop" && f["lease_id"] == exec["lease_id"])
            {
                break;
            }
            assert!(Instant::now() < deadline, "title lease survived");
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let got = read(&f, id).await;
            if got["metadata"]["title_source"] == mode || mode == "managed" {
                assert_eq!(
                    got["metadata"]["title"],
                    if mode == "automatic" {
                        "Three-Pot Balcony Garden"
                    } else {
                        "New thread"
                    }
                );
                if mode == "automatic" {
                    assert_eq!(got["metadata"]["title_source"], "automatic");
                }
                break;
            }
            assert!(Instant::now() < deadline, "{got}");
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        if mode == "managed" {
            let readiness = f.op(1, "agent.readiness", json!({"space_id":space})).await;
            readiness.ok();
            assert_eq!(
                readiness.body["data"]["machines"][0]["providers"]["calternal-claude-code"]["title_blocked"],
                "managed_policy"
            );
        }
    }
}
