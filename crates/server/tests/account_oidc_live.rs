//! Real host and local HTTPS provider exercise the protected OIDC ceremony.
#[path = "account_common/mod.rs"]
mod common;
#[path = "account_live_common/mod.rs"]
mod live;
#[path = "machines_common/mod.rs"]
mod machines;
#[path = "account_network_common/mod.rs"]
mod network;

use live::Host;
use network::Service;
use reqwest::{Client, StatusCode, Url, header};
use serde_json::json;
use std::time::Duration;

struct Flow {
    callback: Url,
    cookie: String,
    id: String,
}

async fn begin(
    host: &Host,
    client: &Client,
    purpose: &str,
    session: Option<&str>,
    scenario: &str,
) -> Flow {
    let started = host
        .post(
            "oidc/start",
            session,
            json!({
                "provider_id":"local", "purpose":purpose,
            }),
        )
        .await;
    assert_eq!(started.status, 200, "{}", started.body);
    let id = started.body["flow_id"].as_str().unwrap().to_owned();
    let mut address = Url::parse(started.body["redirect_url"].as_str().unwrap()).unwrap();
    address.query_pairs_mut().append_pair("test_case", scenario);
    let answer = client.get(address).send().await.unwrap();
    assert_eq!(answer.status(), 302);
    let callback = Url::parse(answer.headers()[header::LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(callback.path(), "/api/v1/auth/oidc/callback");
    Flow {
        callback,
        cookie: started.session(),
        id,
    }
}

async fn callback(host: &Host, address: Url, cookie: &str) -> StatusCode {
    host.client
        .get(address)
        .header(header::COOKIE, cookie)
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .unwrap()
        .status()
}

async fn finish(host: &Host, flow: Flow, session: Option<&str>) -> common::Res {
    assert_eq!(
        callback(host, flow.callback.clone(), &flow.cookie).await,
        303
    );
    assert_eq!(callback(host, flow.callback, &flow.cookie).await, 401);
    let cookie = session.map_or(flow.cookie.clone(), |s| format!("{s}; {}", flow.cookie));
    host.post("oidc/finish", Some(&cookie), json!({"flow_id":flow.id}))
        .await
}

async fn configure(host: &Host, cookie: &str, preset: &str, id: &str, issuer: &str) {
    let saved = host
        .post(
            "oidc/admin/providers",
            Some(cookie),
            json!({
                "provider":{"id":id,"preset":preset,"label":"Test provider","issuer":issuer,
                            "client_id":"fixture-client","scopes":["profile"]},
                "client_secret":"local-client-secret",
            }),
        )
        .await;
    assert_eq!(saved.status, 200, "{}", saved.body);
}

async fn set_enabled(host: &Host, cookie: &str, id: &str, action: &str) -> common::Res {
    host.post(
        "oidc/admin/state",
        Some(cookie),
        json!({"provider_id":id,"action":action}),
    )
    .await
}

#[tokio::test]
async fn generic_provider_validates_proofs_and_uses_explicit_account_links() {
    let disabled = Host::start(|_| {});
    assert_eq!(disabled.get("oidc/providers", None).await.status, 404);
    let issuer = Service::start("oidc");
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(8))
        .add_root_certificate(
            reqwest::Certificate::from_pem(&std::fs::read(issuer.ca()).unwrap()).unwrap(),
        )
        .build()
        .unwrap();
    let host = Host::start(|cmd| {
        cmd.env("CALTERNAL_ACCOUNT_OIDC_ENABLED", "1")
            .env("CALTERNAL_OIDC_CA_FILE", issuer.ca());
    });
    let (setup, signed_in, _, _) = host.owner().await;
    let owner = host.get("me", Some(&signed_in)).await.body["user"]["id"].clone();
    configure(
        &host,
        &signed_in,
        "generic",
        "local",
        &format!("https://localhost:{}", issuer.port),
    )
    .await;
    configure(&host, &signed_in, "apple", "apple", "").await;
    configure(&host, &signed_in, "google", "google", "").await;
    assert_eq!(
        host.get("oidc/providers", None).await.body["providers"],
        json!([])
    );
    for provider in ["apple", "google"] {
        let unavailable = set_enabled(&host, &signed_in, provider, "enable").await;
        assert_eq!(unavailable.status, 409, "{}", unavailable.body);
    }
    let enabled = set_enabled(&host, &signed_in, "local", "enable").await;
    assert_eq!(enabled.status, 200, "{}", enabled.body);
    configure(
        &host,
        &signed_in,
        "generic",
        "local",
        &format!("https://localhost:{}", issuer.port),
    )
    .await;
    let public = host.get("oidc/providers", None).await;
    assert_eq!(public.body["providers"].as_array().unwrap().len(), 1);
    assert_eq!(public.body["providers"][0]["id"], "local");
    assert!(!public.body.to_string().contains("local-client-secret"));
    let denied = host
        .post(
            "oidc/start",
            None,
            json!({"provider_id":"local","purpose":"link"}),
        )
        .await;
    assert_eq!(denied.status, 401);
    let stale = host
        .post(
            "oidc/start",
            Some(&setup),
            json!({"provider_id":"local","purpose":"link"}),
        )
        .await;
    assert_eq!(stale.code(), "passkey_reverification_required");

    let flow = begin(&host, &client, "sign_in", None, "").await;
    let mut wrong_state = flow.callback.clone();
    let code = wrong_state
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    wrong_state.set_query(None);
    wrong_state
        .query_pairs_mut()
        .append_pair("state", "other-flow")
        .append_pair("code", &code);
    assert_eq!(callback(&host, wrong_state, &flow.cookie).await, 401);
    assert_eq!(
        callback(
            &host,
            flow.callback.clone(),
            "__Host-calternal_oidc=other-browser"
        )
        .await,
        401
    );
    let unlinked = finish(&host, flow, None).await;
    assert_eq!(unlinked.status, 403, "{}", unlinked.body);
    assert_eq!(unlinked.code(), "invite_required");
    assert_eq!(
        unlinked.body["error"]["message"],
        "You need an invitation to create an account."
    );
    assert!(unlinked.cookie.is_none());

    for scenario in ["wrong-nonce", "wrong-audience", "bad-signature"] {
        let invalid = begin(&host, &client, "link", Some(&signed_in), scenario).await;
        assert_eq!(
            finish(&host, invalid, Some(&signed_in)).await.code(),
            "provider_failed"
        );
    }
    let valid = begin(&host, &client, "link", Some(&signed_in), "").await;
    assert_eq!(
        finish(&host, valid, Some(&signed_in)).await.body,
        json!({"linked":true})
    );

    let flow = begin(&host, &client, "sign_in", None, "").await;
    let ok = finish(&host, flow, None).await;
    assert_eq!(ok.status, 200, "{}", ok.body);
    assert_eq!(ok.body["user"]["id"], owner);
    let provider = ok.session();
    assert_eq!(
        host.get("me", Some(&provider)).await.body["sign_in_provider"],
        "local"
    );
    let sensitive = host
        .post(
            "username/change",
            Some(&provider),
            json!({"username":"new-handle"}),
        )
        .await;
    assert_eq!(sensitive.code(), "passkey_reverification_required");

    for scenario in ["old-auth", "missing-auth"] {
        let stale = begin(&host, &client, "reverify", Some(&provider), scenario).await;
        assert_eq!(
            finish(&host, stale, Some(&provider)).await.code(),
            "provider_failed"
        );
    }
    let fresh = begin(&host, &client, "reverify", Some(&provider), "").await;
    let renewed = finish(&host, fresh, Some(&provider)).await;
    assert_eq!(renewed.status, 200, "{}", renewed.body);
    let fresh_cookie = renewed.session();
    let changed = host
        .post(
            "username/change",
            Some(&fresh_cookie),
            json!({"username":"new-handle"}),
        )
        .await;
    assert_eq!(changed.status, 200, "{}", changed.body);
    assert_eq!(
        set_enabled(&host, &signed_in, "local", "disable")
            .await
            .status,
        200
    );
    assert_eq!(host.get("me", Some(&fresh_cookie)).await.status, 401);
    assert_eq!(
        set_enabled(&host, &signed_in, "local", "enable")
            .await
            .status,
        200
    );
    let unlinked = host
        .post(
            "oidc/unlink",
            Some(&signed_in),
            json!({"provider_id":"local"}),
        )
        .await;
    assert_eq!(unlinked.body, json!({"unlinked":true}));
    let flow = begin(&host, &client, "sign_in", None, "").await;
    assert_eq!(finish(&host, flow, None).await.code(), "invite_required");
}
