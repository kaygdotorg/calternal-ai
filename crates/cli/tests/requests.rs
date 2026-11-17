//! Golden output for the account inbox and each decision.
use calternal_cli::{
    output::Output,
    requests::reply,
    testing::{assert_golden, assert_shape},
};
use serde_json::json;
use time::UtcOffset;

#[test]
fn request_output_matches_goldens() {
    let id = "019a8c00-0000-7000-8000-000000000001";
    let space = "019a8c00-0000-7000-8000-000000000002";
    let samples = [
        (
            "list",
            json!({"requests":[{"id":id,"space_id":space,"thread_id":null,"detail":{"kind":"approval","app":"notes","write":true,"action":{"operation":"notes.create"}}}]}),
        ),
        ("answer", json!({"id":id,"status":"answered"})),
        ("approve", json!({"id":id,"status":"approved"})),
        ("deny", json!({"id":id,"status":"denied"})),
    ];
    let golden = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for (name, value) in samples {
        let rendered = reply(value, "fixture-request");
        for width in [40, 100] {
            let output = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
            assert_golden(
                &golden.join(format!("requests-{name}-{width}.txt")),
                &output.render(&rendered),
            );
        }
        let output = Output::fixed(true, None, UtcOffset::UTC);
        let text = output.render(&rendered);
        let doc = serde_json::from_str(&text).unwrap();
        assert_shape(&format!("calternal requests {name}"), &doc);
        assert_shape(&format!("cl requests {name}"), &doc);
        assert_golden(&golden.join(format!("requests-{name}.json")), &text);
    }
}
