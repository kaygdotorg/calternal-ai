//! Check the ordinary Notes save and guarded Markdown placement.
use super::*;

fn edit(srv: &Server, note: &Value, markdown: &str, title: Option<&str>) -> Resp {
    let mut input = json!({
        "space_id":srv.personal,"id":note["id"],"base_revision":note["revision"],"markdown":markdown
    });
    if let Some(title) = title {
        input["title"] = json!(title);
    }
    op(srv, &srv.owner, "save", "notes.update", input)
}

#[test]
fn title_hint_cannot_rewrite_the_submitted_heading() {
    let d = TempDir::new("notes-title-bytes");
    let srv = start(&d.0, None);
    let n = create(&srv, "create", &srv.personal, "# Real title\n");
    let markdown =
        "---\n# YAML comment\n---\n```md\n# Example heading\n```\n# Real title\nEdited.\n";
    let saved = edit(&srv, &n, markdown, Some("YAML comment"));
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.body["data"]["note"]["markdown"], markdown);
    assert_eq!(saved.body["data"]["note"]["title"], "Real title");
}

#[test]
fn normalized_editor_save_retains_existing_crlf_and_bom() {
    let d = TempDir::new("notes-crlf-bytes");
    let srv = start(&d.0, None);
    let n = create(
        &srv,
        "create",
        &srv.personal,
        "\u{feff}# Windows\r\nkeep\r\nchange\r\n",
    );
    let saved = edit(&srv, &n, "# Windows\nkeep\nchanged\n", None);
    assert_eq!(saved.status, 200, "{}", saved.body);
    let file = note_file(&d.0, &srv.personal, n["id"].as_str().unwrap());
    let bytes = std::fs::read_to_string(file).unwrap();
    assert!(bytes.starts_with("\u{feff}"), "{bytes:?}");
    assert!(
        bytes.ends_with("# Windows\r\nkeep\r\nchanged\r\n"),
        "{bytes:?}"
    );
}
