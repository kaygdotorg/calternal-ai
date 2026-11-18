//! Installed worker diagnostics over the production HTTP operations.
use super::*;

pub(super) const OPERATIONS: &[&str] = &["plugins.diagnostics"];

#[tokio::test]
async fn installed_diagnostics_keep_a_bounded_crash_tail_in_the_workers_space() {
    let f = Fixture::new().await;
    let space = &f.spaces[2];
    let private_space = &f.spaces[0];
    let package = "diagnostic-worker";
    let operation = "diagnostic-worker.crash";
    let archive = f.dir.path().join("diag-worker.calplugin");
    std::fs::write(&archive, json!({
        "manifest": {
            "id": package, "name": "Diagnostic worker", "icon": "puzzle",
            "version": "0.1.0", "api_version": 1,
            "kinds": [], "collections": [], "views": [], "attention": [], "settings": [],
            "schedules": [], "webhooks": [], "secrets": [], "hooks": [], "themes": [], "host_calls": [],
            "operations": [{
                "id": operation, "required_capability": "space.read", "mutates": false,
                "idempotency": "read_only", "risk": "read", "input_schema": {"type":"object"},
                "output_schema": {"type":"object"}
            }]
        },
        "worker": r#"require('node:readline').createInterface({ input: process.stdin }).on('line', line => {
          if (JSON.parse(line).type === 'ready') { console.log(line); return; }
          process.stderr.write('x'.repeat(5000) + 'bounded-tail-marker', () => process.exit(9));
        });"#
    }).to_string()).unwrap();
    crate::owned_slot_cases::install(&f, &archive, space);
    for target in [space, private_space] {
        f.op(
            1,
            "plugins.enable",
            json!({"space_id":target,"plugin_id":package}),
        )
        .await
        .ok();
    }
    let input = |target: &str| json!({"space_id":target,"plugin_id":package});
    let cold = f.op(1, "plugins.diagnostics", input(space)).await;
    cold.ok();
    assert_eq!(cold.body["data"]["readiness"], "ready");
    assert_eq!(
        f.op(2, "plugins.diagnostics", input(space)).await.status,
        403
    );
    assert_eq!(
        f.op(3, "plugins.diagnostics", input(space)).await.status,
        404
    );
    assert_ne!(
        f.op(1, operation, json!({"space_id":space})).await.status,
        200
    );
    let crashed = f.op(1, "plugins.diagnostics", input(space)).await;
    crashed.ok();
    let diagnostics = &crashed.body["data"];
    assert_eq!(diagnostics["readiness"], "failed");
    let tail = diagnostics["stderr_tail"].as_str().unwrap();
    assert!(tail.ends_with("bounded-tail-marker"));
    assert!(tail.len() <= 4096);
    assert_eq!(diagnostics["recent_errors"][0]["operation"], operation);
    let elsewhere = f.op(1, "plugins.diagnostics", input(private_space)).await;
    elsewhere.ok();
    assert_eq!(elsewhere.body["data"]["stderr_tail"], Value::Null);
    assert_eq!(elsewhere.body["data"]["readiness"], "not_started");
    assert_eq!(elsewhere.body["data"]["recent_errors"], json!([]));
}
