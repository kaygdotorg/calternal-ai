//! Human/JSON output goldens share the real thread renderer and width adapter.
use calternal_cli::{output::Output, testing::assert_golden, threads::reply};
use serde_json::json;
use time::UtcOffset;
#[test]
fn thread_results_have_readable_width_adaptation_and_stable_json() {
    let samples = [
        (
            "list",
            json!({"threads":[{"id":"019a8c00-0000-7000-8000-000000000001","metadata":{"title":"Long conversation title that should wrap clearly","archived":false,"message_count":17,"provider":"calternal-claude-code","model":"sonnet","mode":"least_access","last_activity_at":"2026-11-17T12:00:00.000Z"},"turn_state":"waiting"}]}),
        ),
        (
            "get",
            json!({"thread":{"id":"019a8c00-0000-7000-8000-000000000001","address":"/threads/019a8c00-0000-7000-8000-000000000001","revision":"v1.fixture","turn_state":"running","metadata":{"title":"Plan","archived":false,"deleted":false},"markdown":"---\ntitle: Plan\n---\nHello\n"}}),
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
        let reply = reply(value);
        for width in [40, 80, 120] {
            let output = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
            assert_golden(
                &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/golden/threads-{name}-{width}.txt")),
                &output.render(&reply),
            );
        }
        let output = Output::fixed(true, None, UtcOffset::UTC);
        assert_golden(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(format!("tests/golden/threads-{name}.json")),
            &output.render(&reply),
        );
    }
}
