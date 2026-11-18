//! Human/JSON output goldens share the real thread renderer and width adapter.
use calternal_cli::{output::Output, testing::assert_golden, threads::reply};
use serde_json::json;
use time::UtcOffset;
#[test]
fn thread_results_have_readable_width_adaptation_and_stable_json() {
    let samples = [
        (
            "list",
            json!({"threads":[{"id":"019a8c00-0000-7000-8000-000000000001","metadata":{"title":"Long conversation title that should wrap clearly","archived":false,"message_count":17,"provider":"calternal-claude-code","model":"sonnet","mode":"least_access","last_activity_at":"2026-11-17T12:00:00.000Z"},"turn_state":"waiting","turn_reason":{"code":"waiting_on_person","request_id":"019a8c00-0000-7000-8000-000000000002"}}]}),
        ),
        (
            "get",
            json!({"thread":{"id":"019a8c00-0000-7000-8000-000000000001","address":"/threads/019a8c00-0000-7000-8000-000000000001","revision":"v1.fixture","turn_state":"queued","turn_reason":{"code":"retrying","attempt":2,"next_attempt_ms":1794916800000i64,"rule":"provider_backoff"},"metadata":{"title":"Plan","archived":false,"deleted":false},"markdown":"---\ntitle: Plan\n---\nHello\n"}}),
        ),
        (
            "rename",
            json!({"thread":{"id":"019a8c00-0000-7000-8000-000000000001","address":"/threads/019a8c00-0000-7000-8000-000000000001","metadata":{"title":"Renamed plan","archived":true,"deleted":false},"markdown":"---\ntitle: Renamed plan\narchived: true\n---\n"}}),
        ),
        (
            "unarchive",
            json!({"thread":{"id":"019a8c00-0000-7000-8000-000000000001","address":"/threads/019a8c00-0000-7000-8000-000000000001","metadata":{"title":"Renamed plan","archived":false,"deleted":false},"markdown":"---\ntitle: Renamed plan\narchived: false\n---\n"}}),
        ),
        ("empty", json!({"threads":[]})),
    ];
    for (name, value) in samples {
        for width in [40, 80, 120] {
            let output = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
            assert_golden(
                &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/golden/threads-{name}-{width}.txt")),
                &output.render(&reply(&output, value.clone())),
            );
        }
        let output = Output::fixed(true, None, UtcOffset::UTC);
        assert_golden(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/golden/threads-{name}.json")),
            &output.render(&reply(&output, value.clone())),
        );
    }
}

#[test]
fn checkpoint_additions_render_with_coordinates_and_markdown() {
    let data = json!({"delta":{
        "id":"019a8c00-0000-7000-8000-000000000001",
        "base_revision":"v3.fixture","revision":"v4.fixture",
        "offset":72,"markdown":"Hello 🌏",
    }});
    let out = Output::fixed_terminal(false, 80, UtcOffset::UTC, true);
    let human = out.render(&reply(&out, data.clone()));
    for expected in ["v3.fixture", "v4.fixture", "72", "Hello 🌏"] {
        assert!(human.contains(expected), "{human}");
    }
    let out = Output::fixed(true, None, UtcOffset::UTC);
    let wire: serde_json::Value =
        serde_json::from_str(&out.render(&reply(&out, data.clone()))).unwrap();
    assert_eq!(wire, json!({"ok":true,"data":data}));
}
