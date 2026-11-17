//! Human/JSON output goldens share the real thread renderer and width adapter.
use calternal_cli::{output::Output, testing::assert_golden, threads::reply};
use serde_json::json;
use time::UtcOffset;
#[test]
fn thread_results_have_readable_width_adaptation_and_stable_json() {
    let samples = [
        (
            "list",
            json!({"threads":[{"id":"019a8c00-0000-7000-8000-000000000001","metadata":{"title":"Long conversation title that should wrap clearly","archived":false}}]}),
        ),
        (
            "get",
            json!({"thread":{"id":"019a8c00-0000-7000-8000-000000000001","address":"/threads/019a8c00-0000-7000-8000-000000000001","revision":"v1.fixture","metadata":{"title":"Plan","archived":false,"deleted":false},"markdown":"---\ntitle: Plan\n---\nHello\n"}}),
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
