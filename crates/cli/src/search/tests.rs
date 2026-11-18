use super::*;
use crate::{
    output::Output,
    testing::{assert_golden, assert_shape},
};

#[test]
fn search_commands_match_human_and_json_goldens() {
    let hits = json!({"hits":[{
        "address":"/notes/019aabb0-1100-7111-8111-111111111111",
        "space_id":"019aabb0-2200-7222-8222-222222222222",
        "title":"Review notes","kind":"notes",
        "excerpt":[{"text":"Review","matched":true},{"text":" notes","matched":false}]
    }],"partial":false,"next_cursor":"p-next"});
    let mut prefix_hits = hits.clone();
    prefix_hits["hits"][0]["excerpt"] = json!([]);
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    for (name, op, data) in [
        ("query", SearchOperation::Query, hits.clone()),
        ("items", SearchOperation::Items, prefix_hits),
        (
            "threads",
            SearchOperation::Threads,
            json!({"hits":[{
            "address":"/threads/019aabb0-1100-7111-8111-111111111111",
            "space_id":"019aabb0-2200-7222-8222-222222222222",
            "title":"Review plan","kind":"threads","excerpt":[]
        }],"partial":false}),
        ),
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
