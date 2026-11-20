//! First real run of the Threads app on the plugin API v2 host (issue #101).
//!
//! Drives the built `calternal-threads` app and its separate
//! `calternal-threads-claude-code` harness package through the real installer
//! and server binary in a new space. Each step asserts the host route it needs,
//! so a failure names the first missing seam on #101 rather than a later
//! symptom. Built behind the `threads-v2` feature until those seams land, so
//! the default conformance run is unaffected.
//!
//! Needs `npm run build -w @calternal/threads` (which writes both packages'
//! `dist/`) and a server built with `--features test-plugins`.
use super::*;

const APP: &str = "calternal-threads";
const HARNESS: &str = "calternal-threads-claude-code";

fn dist(package: &str) -> PathBuf {
    let path = source_root().join("plugins").join(package).join("dist");
    assert!(
        path.join("manifest.json").is_file(),
        "{package} is not built; run npm run build -w @calternal/threads"
    );
    path
}

#[tokio::test]
async fn threads_create_send_reply_replay_and_stop_on_the_real_host() {
    let f = Fixture::new().await;
    let created = f
        .op(1, "spaces.create", json!({"name":"Threads first run"}))
        .await;
    created.ok();
    let space = created.body["data"]["space"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // The owning app first: the harness contribution names its extension point.
    for package in [APP, HARNESS] {
        crate::owned_slot_cases::install(&f, &dist(package), &space);
        f.op(
            1,
            "plugins.enable",
            json!({"space_id":space,"plugin_id":package}),
        )
        .await
        .ok();
    }

    // Seams 7 and 8 on #101: no v2 client route exists and its HTTP binding is
    // unpinned. This is the first step a renderer takes, so the run stops here
    // until slices 5 and 6 add it. Create, Send, first output, reload replay
    // and Stop follow in this order once it answers.
    let opened = f
        .op(
            1,
            "app.open",
            json!({"space_id":space,"app":{"package":APP,"app":"threads"}}),
        )
        .await;
    assert_eq!(
        opened.status, 200,
        "app.open is the first missing v2 route (#101 seams 7, 8): {}",
        opened.body
    );
}
