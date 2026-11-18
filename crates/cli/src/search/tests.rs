use super::*;
use crate::{
    output::Output,
    testing::{assert_golden, assert_shape},
};

#[test]
fn search_commands_match_human_and_json_goldens() {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for (name, op, data) in [
        (
            "people",
            SearchOperation::People,
            json!({"people":[{"id":"019aabb0-1100-7111-8111-111111111111","name":"Account A","kind":"account"}]}),
        ),
        (
            "skills",
            SearchOperation::Skills,
            json!({"skills":[{"name":"review","description":"Review a draft."}]}),
        ),
        (
            "manifests",
            SearchOperation::Manifests,
            json!({"manifests":[{"plugin":"calternal-notes","version":"0.1.0","commands":[{"id":"notes.new","label":"New note","group":"Notes","op":"note.new","words":["create note"]}],"settings_sections":[{"id":"notes.display","label":"Display","go":"/settings/notes.display"}]}]}),
        ),
    ] {
        for (machine, ext) in [(false, "txt"), (true, "json")] {
            let out = Output::fixed(machine, Some(120), time::UtcOffset::UTC);
            let text = out.render(&reply(op, data.clone()).request_id("req-search"));
            assert_golden(&base.join(format!("search-{name}.{ext}")), &text);
            if machine {
                assert_shape(
                    &format!("calternal search {name}"),
                    &serde_json::from_str(&text).unwrap(),
                );
            }
        }
        let narrow = Output::fixed(false, Some(40), time::UtcOffset::UTC);
        assert_golden(
            &base.join(format!("search-{name}-width-40.txt")),
            &narrow.render(&reply(op, data)),
        );
    }
}
