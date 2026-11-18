//! The real server delivers account mail over authenticated TLS SMTP.
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
use serde_json::{Value, json};

const OWNER: &str = "owner@example.test";
const NEW: &str = "new@example.test";

fn code(message: &str) -> &str {
    message
        .lines()
        .find(|line| line.len() == 8 && line.bytes().all(|byte| byte.is_ascii_digit()))
        .expect("an eight-digit code in the text body")
}

fn test_mail(host: &Host, recipient: &str, json: bool) -> std::process::Output {
    let mut cmd = host.cli();
    cmd.args(["mail-test", recipient, "--appdata"])
        .arg(host.appdata());
    if json {
        cmd.arg("--json");
    }
    cmd.output().unwrap()
}

async fn verify(host: &Host, smtp: &mut Service, cookie: &str, address: &str, index: usize) {
    let start = host
        .post("email/start", Some(cookie), json!({"email":address}))
        .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let sent = smtp.mail(index, address, "Verify your Calternal email address");
    assert!(!start.body.to_string().contains(code(&sent)));
    let confirmed = host
        .post(
            "email/confirm",
            Some(cookie),
            json!({
                "challenge_id":start.body["challenge_id"], "code":code(&sent),
            }),
        )
        .await;
    assert_eq!(confirmed.status, 200, "{}", confirmed.body);
    assert_eq!(confirmed.body["email_status"], "verified");
}

#[tokio::test]
async fn smtp_test_command_reports_the_live_delivery_result() {
    let disabled = Host::start(|_| {});
    let result = test_mail(&disabled, OWNER, true);
    assert!(!result.status.success());
    let error: Value = serde_json::from_slice(&result.stdout).expect("structured CLI error");
    assert_eq!(error["error"]["code"], "mail_not_configured");

    let mut sink = Service::start("implicit");
    let host = smtp_host(&sink, "implicit");
    let result = test_mail(&host, OWNER, false);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout).contains("SMTP server accepted the test message")
    );
    sink.mail(0, OWNER, "Calternal SMTP test");
    let result = test_mail(&host, OWNER, true);
    assert!(result.status.success());
    let receipt: Value = serde_json::from_slice(&result.stdout).unwrap();
    calternal_cli::testing::assert_shape("calternal mail-test", &receipt);
    assert_eq!(receipt["data"]["accepted"], true);
    sink.mail(1, OWNER, "Calternal SMTP test");
    let result = test_mail(&host, "reject@example.test", true);
    assert!(!result.status.success());
    let error: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(error["error"]["code"], "mail_delivery_failed");
    assert!(result.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("local-test-password"));
}

fn smtp_host(sink: &Service, tls: &str) -> Host {
    Host::start(|cmd| {
        cmd.env("SSL_CERT_FILE", sink.ca())
            .env("CALTERNAL_SMTP_HOST", "localhost")
            .env("CALTERNAL_SMTP_FROM", "sender@example.test")
            .env("CALTERNAL_SMTP_USERNAME", "fixture")
            .env("CALTERNAL_SMTP_PASSWORD", "local-test-password")
            .env("CALTERNAL_SMTP_PORT", sink.port.to_string())
            .env("CALTERNAL_SMTP_TLS", tls);
    })
}

#[tokio::test]
async fn verification_and_security_notices_reach_the_right_mailbox() {
    let mut sink = Service::start("starttls");
    let host = smtp_host(&sink, "starttls");
    let (_, cookie, _, _) = host.owner().await;
    assert_eq!(host.get("status", None).await.body["mail_available"], true);
    verify(&host, &mut sink, &cookie, OWNER, 0).await;

    let start = host
        .post("passkeys/add/start", Some(&cookie), json!({}))
        .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let mut device = common::Device::new();
    let added = host.post("passkeys/add/finish", Some(&cookie), json!({
        "ceremony":start.body["ceremony"], "credential":device.create_at(&host.origin, &start),
        "name":"Second device",
    })).await;
    assert_eq!(added.status, 200, "{}", added.body);
    sink.mail(1, OWNER, "A passkey was added to your Calternal account");

    let start = host
        .post(
            "recovery/start",
            None,
            json!({"method":"email","email":OWNER}),
        )
        .await;
    assert_eq!(start.status, 200, "{}", start.body);
    let sent = sink.mail(2, OWNER, "Confirm your Calternal recovery request");
    let held = host
        .post(
            "recovery/prove",
            None,
            json!({
                "flow_id":start.body["flow_id"], "code":code(&sent),
            }),
        )
        .await;
    assert_eq!(held.status, 200, "{}", held.body);
    assert!(held.body["hold_until"].is_number());
    sink.mail(3, OWNER, "Calternal recovery waiting period started");
    let status = host
        .post(
            "recovery/status",
            None,
            json!({"flow_id":start.body["flow_id"]}),
        )
        .await;
    assert_eq!(status.body["stage"], "waiting");
    let veto = host.post("recovery/veto", Some(&cookie), json!({})).await;
    assert_eq!(veto.body["cancelled"], true);

    verify(&host, &mut sink, &cookie, NEW, 4).await;
    sink.mail(5, OWNER, "Your Calternal email address changed");
}
