//! A preview follows the current membership of its fenced thread lease.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};

async fn preview_request(machine: &mut requests_cases::Machine, f: &Fixture) -> Value {
    tokio::time::timeout(Duration::from_secs(6), async {
        loop {
            if let Some(command) = machine
                .sync(f, json!([]))
                .await
                .into_iter()
                .find(|data| data["type"] == "preview")
            {
                return command;
            }
        }
    })
    .await
    .expect("server sent a preview request")
}

fn page() -> Value {
    json!([
        {"seq":1, "kind":"data", "stream":"stdout", "data": B64.encode(
            b"{\"status\":200,\"content_type\":\"text/html\"}\n<p>Private dev preview</p>"
        )},
        {"seq":2, "kind":"exit", "code":0, "signal":null, "error":null}
    ])
}

#[tokio::test]
async fn machine_preview_refuses_foreign_spaces_and_revoked_viewers() {
    let f = Fixture::new().await;
    let mut machine = requests_cases::Machine::enroll(&f).await;
    let space = &f.spaces[2]; // Account B is a Reader, account 3 has no membership.
    let url = "/api/v1/machines/runs";
    let lease = f
        .req(
            "POST",
            url,
            Some(&f.cookies[1]),
            json!({
                "space_id": space, "thread_id": uuid::Uuid::new_v4()
            }),
            Some(ORIGIN),
        )
        .await;
    lease.ok();
    let lease = lease.body["lease_id"].as_str().unwrap();
    assert!(
        machine
            .sync(&f, json!([]))
            .await
            .iter()
            .any(|e| e["type"] == "assign")
    );
    machine.sync(&f, json!([])).await; // report that the assigned sandbox is running
    let publication = format!("{url}/{lease}/previews");

    let refused = f
        .req(
            "POST",
            &publication,
            Some(&f.cookies[2]),
            json!({"port":3000}),
            Some(ORIGIN),
        )
        .await;
    assert_eq!(refused.status, 404);
    let made = f
        .req(
            "POST",
            &publication,
            Some(&f.cookies[1]),
            json!({"port":3000}),
            Some(ORIGIN),
        )
        .await;
    made.ok();
    let path = made.body["path"].as_str().unwrap();

    for (cookie, status) in [(Some(f.cookies[3].as_str()), 404), (None, 401)] {
        let r = f.req("GET", path, cookie, Value::Null, None).await;
        assert_eq!(r.status, status);
    }
    let unknown = format!("/api/v1/machine-previews/{}/", uuid::Uuid::new_v4());
    let real = f
        .req("GET", path, Some(&f.cookies[3]), Value::Null, None)
        .await;
    let missing = f
        .req("GET", &unknown, Some(&f.cookies[3]), Value::Null, None)
        .await;
    assert_eq!(real.signature(), missing.signature());

    let fetch = || {
        f.client
            .get(format!("{}{path}hello?q=3", f.base))
            .header("host", "localhost:18080")
            .header("cookie", &f.cookies[2])
            .header("x-private-client-header", "must stay on the server")
            .send()
    };
    let (result, ()) = tokio::join!(fetch(), async {
        let request = preview_request(&mut machine, &f).await;
        assert_eq!(request["lease_id"], lease);
        assert_eq!(
            request["request"],
            json!({"port":3000, "path":"/hello?q=3"})
        );
        assert_eq!(
            machine
                .try_output(&f, &request, page())
                .await
                .unwrap()
                .status,
            409
        );
        machine.preview_output(&f, &request, page()).await.ok();
    });
    let result = result.unwrap();
    assert_eq!(result.status(), 200);
    assert_eq!(result.headers()["x-frame-options"], "SAMEORIGIN");
    assert_eq!(result.headers()["cache-control"], "no-store");
    assert!(
        result.headers()["content-security-policy"]
            .to_str()
            .unwrap()
            .contains("sandbox allow-scripts")
    );
    assert_eq!(result.text().await.unwrap(), "<p>Private dev preview</p>");

    let (result, ()) = tokio::join!(fetch(), async {
        let request = preview_request(&mut machine, &f).await;
        f.op(
            1,
            "spaces.remove_member",
            json!({"space_id":space,"account_id":f.ids[2]}),
        )
        .await
        .ok();
        machine.preview_output(&f, &request, page()).await.ok();
    });
    assert_eq!(result.unwrap().status(), 404);
    let stop = format!("{url}/{lease}/stop");
    f.req("POST", &stop, Some(&f.cookies[1]), json!({}), Some(ORIGIN))
        .await
        .ok();
    assert_eq!(
        f.req("GET", path, Some(&f.cookies[1]), Value::Null, None)
            .await
            .status,
        404
    );
}
