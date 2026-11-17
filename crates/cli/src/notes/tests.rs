use super::*;
use crate::testing::{assert_golden, assert_shape};
use std::path::PathBuf;

#[test]
fn notes_commands_match_human_and_json_goldens() {
    let summary = json!({
        "id":"019aabb0-1100-7111-8111-111111111111",
        "space_id":"019aabb0-2200-7222-8222-222222222222",
        "title":"New Plan","revision":"v2.preview","updated_at":"2026-11-17T06:12:03.000Z"
    });
    let with = |field: &str, value: &str| {
        let mut row = summary.clone();
        row[field] = json!(value);
        row
    };
    let tags = json!({"tags":[{"name":"work","count":3}]});
    for (op, data) in [
        ("notes.list", json!({"notes":[summary.clone()]})),
        (
            "notes.lookup",
            json!({"notes":[with("link_target","New Plan.md")]}),
        ),
        ("notes.delete", json!({"note":summary.clone()})),
        ("notes.restore", json!({"note":summary.clone()})),
        (
            "notes.rename",
            json!({"note":with("markdown","# New Plan\n")}),
        ),
        ("trash.list", json!({"notes":[summary.clone()]})),
        ("trash.purge", json!({"id":summary["id"],"state":"deleted"})),
        ("tags.list", tags.clone()),
        ("tags.lookup", tags),
    ] {
        let stem = if op == "notes.list" {
            "calternal-notes-list".into()
        } else {
            op.replace('.', "-")
        };
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
        for (json, ext) in [(false, "txt"), (true, "json")] {
            let out = Output::fixed(json, None, time::UtcOffset::UTC);
            let text = out.render(&reply(&out, op, data.clone()).request_id("req-notes"));
            assert_golden(&base.join(format!("{stem}.{ext}")), &text);
            if json {
                assert_shape(
                    &format!("calternal {}", op.replace('.', " ")),
                    &serde_json::from_str(&text).unwrap(),
                );
            }
        }
    }
}
