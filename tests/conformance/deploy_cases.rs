use super::*;

#[tokio::test]
async fn deployment_bind_preserves_host_and_origin_checks() {
    let f = fixture_with_listener().await;
    f.req("GET", "/api/v1/auth/status", None, Value::Null, None)
        .await
        .ok();

    let denied_host = f
        .client
        .get(format!("{}/api/v1/auth/status", f.base))
        .header("host", "foreign.example")
        .header("x-forwarded-host", "localhost:18080")
        .send()
        .await
        .unwrap();
    assert_eq!(denied_host.status(), 403);

    let denied_origin = f
        .client
        .post(format!("{}/api/v1/auth/setup/start", f.base))
        .header("host", "localhost:18080")
        .header("origin", "https://foreign.example")
        .header("x-forwarded-host", "localhost:18080")
        .header("x-forwarded-proto", "http")
        .json(&json!({"token":"invalid","name":"Foreign"}))
        .send()
        .await
        .unwrap();
    assert_eq!(denied_origin.status(), 403);
}

async fn fixture_with_listener() -> Fixture {
    let mut f = Fixture::empty();
    f.bind_ip = Some("127.0.0.2");
    f.start().await;
    assert!(f.base.starts_with("http://127.0.0.2:"));
    f
}
