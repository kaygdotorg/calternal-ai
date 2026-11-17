//! Golden human and JSON output for machine readiness.
use calternal_cli::{
    agent::readiness_reply,
    output::Output,
    testing::{assert_golden, assert_shape},
};
use serde_json::json;
use time::UtcOffset;

#[test]
fn readiness_output_matches_goldens() {
    let sample = json!({
        "check_ttl_ms":300000,
        "machines":[{"machine_id":"019a8c00-0000-7000-8000-000000000001",
            "name":"Development","enrolled":true,"connected":true,"available":true,
            "execution":"sandboxed","last_seen_ms":1760000000000i64,
            "providers":{"calternal-claude-code":{"check":{
                "installed":true,"signed_in":false,"sign_in_command":"claude auth login"
            },"enabled":true,"ready":false}}
        }],
        "pool":[{"enrolled":true,"connected":false,"available":false,"execution":"sandboxed",
            "providers":{"calternal-claude-code":{"check":null,"enabled":true,"ready":false}}}]
    });
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for width in [40, 120] {
        let out = Output::fixed_terminal(false, width, UtcOffset::UTC, true);
        let text = out.render(&readiness_reply(&out, sample.clone()));
        assert_golden(&path.join(format!("agent-readiness-{width}.txt")), &text);
    }
    let out = Output::fixed(false, None, UtcOffset::UTC);
    assert_golden(
        &path.join("agent-readiness.txt"),
        &out.render(&readiness_reply(&out, sample.clone())),
    );
    let out = Output::fixed(true, None, UtcOffset::UTC);
    let text = out.render(&readiness_reply(&out, sample));
    assert_shape(
        "calternal agent readiness",
        &serde_json::from_str(&text).unwrap(),
    );
    assert_golden(&path.join("agent-readiness.json"), &text);
}
