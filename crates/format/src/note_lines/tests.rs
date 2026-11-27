//! Vectors from the existing app, docs/plans/0067-vectors/lines.json.

use super::*;

#[test]
fn existing_app_line_vectors() {
    let vectors = [
        ("box-space", "- [ ] task", "t"),
        ("box-120", "- [x] task", "t"),
        ("box-88", "- [X] task", "t"),
        ("box-47", "- [/] task", "t"),
        ("box-45", "- [-] task", "t"),
        ("box-62", "- [>] task", "t"),
        ("box-63", "- [?] task", "t"),
        ("box-10003", "- [✓] task", "t"),
        ("empty-box-eol", "- [ ]", "t"),
        ("custom-eol", "- [?]", "t"),
        ("ordered", "1. [x] task\n2) [/] task", "t,t"),
        ("markers", "* [-] task\n+ [>] task", "t,t"),
        ("no-space-after", "- [x]oops", "."),
        ("escaped-box", "- \\[x] task", "."),
        ("bare-box", "[ ] task", "."),
        ("bare-time", "10:00 meeting\n11:30 next", "e,e"),
        ("list-time", "- 10:00 meeting", "e"),
        ("timed-task", "- [ ] 10:00 meeting", "te"),
        (
            "not-clock",
            "25:00 nope\n09:99 nope\n10:00ish nope",
            ".,.,.",
        ),
        ("inline-code", "10:00 `hidden`", "x"),
        (
            "multiline-code",
            "intro `code\n10:00 hidden\nend`\n11:00 visible",
            "x,x,x,e",
        ),
        (
            "double-code",
            "intro ``one `\n10:00 hidden\nend``\n11:00 visible",
            "x,x,x,e",
        ),
        ("unmatched-code", "intro `open\n10:00 visible", ".,e"),
        ("unmatched-double", "intro ``open\n10:00 visible", ".,e"),
        (
            "list-code",
            "- prefix `code\n  10:00 hidden\n  end`\n- 11:00 visible",
            "x,x,x,e",
        ),
        (
            "quote-code",
            "> prefix `code\n> 10:00 hidden\n> end`\n- 11:00 visible",
            "x,x,x,e",
        ),
        (
            "task-code",
            "- [ ] text `code\n  10:00 hidden\n  end`",
            "x,x,x",
        ),
        (
            "backtick-fence",
            "```md\n- [ ] hidden\n10:00 hidden\n```\n10:30 visible",
            "x,x,x,x,e",
        ),
        ("tilde-fence", "~~~\n10:00 hidden\n~~~", "x,x,x"),
        ("unclosed-fence", "```\n10:00 hidden\n- [x] hidden", "x,x,x"),
        ("indented-code", "    10:00 hidden\n    - [ ] hidden", "x,x"),
        ("tab-code", "\t10:00 hidden\n\t- [ ] hidden", "x,x"),
        (
            "list-fence",
            "- parent\n\n  ```\n  10:00 hidden\n  ```\n  - [ ] child",
            ".,.,x,x,x,t",
        ),
        (
            "list-indented",
            "- parent\n\n      10:00 hidden\n\n  - [ ] child",
            ".,.,x,.,t",
        ),
        (
            "block-comment",
            "<!--\n10:00 hidden\n- [ ] hidden\n-->\n10:30 visible",
            "x,x,x,x,e",
        ),
        ("inline-comment", "10:00 before <!-- hidden --> after", "x"),
        (
            "multiline-comment",
            "before <!--\n10:00 hidden\n--> after\n11:00 visible",
            "x,x,x,e",
        ),
        ("unclosed-comment", "<!--\n10:00 hidden", "x,x"),
        (
            "adjacent-comment",
            "10:00 <!-- x --> visible\n11:00 visible",
            "x,e",
        ),
        (
            "html-block",
            "<div>\n10:00 hidden\n</div>\n\n11:00 visible",
            "x,x,x,.,e",
        ),
        ("inline-html", "10:00 <b>visible</b>", "x"),
        (
            "percent-comment",
            "%%\n10:00 visible\n- [ ] visible\n%%",
            ".,e,t,.",
        ),
        ("inline-percent", "10:00 %% ordinary prose %%", "e"),
        (
            "nested-list",
            "- [ ] parent\n  - [x] child\n    - [/] grandchild",
            "t,t,t",
        ),
        (
            "lazy-continuation",
            "- parent\n10:00 continued\n  - [ ] child",
            ".,e,t",
        ),
        ("nested-event", "- 10:00 parent\n  - 11:00 child", "e,e"),
        ("quote-event", "> 10:00 unsupported", "."),
        ("quote-task", "> - [ ] task", "t"),
        ("callout-task", "> [!todo]\n> - [/] task", ".,t"),
        ("nested-quote", "> > - [X] task", "t"),
        ("quote-fence", "> ```\n> - [ ] hidden\n> ```", "x,x,x"),
        (
            "frontmatter",
            "---\n10:00: yaml\n---\n10:00 visible",
            "f,f,f,e",
        ),
        ("empty-frontmatter", "---\n---\n10:00 visible", "f,f,e"),
        (
            "unclosed-frontmatter",
            "---\n10:00 hidden\n- [ ] hidden",
            "f,f,f",
        ),
        ("body-divider", "text\n\n---\n10:00 visible", ".,.,.,e"),
        ("crlf", "- [ ] task\r\n10:00 visible\r\n", "t,e,."),
        ("crlf-code", "`start\r\n10:00 hidden\r\nend`", "x,x,x"),
        (
            "crlf-frontmatter",
            "---\r\ntitle: test\r\n---\r\n10:00 visible",
            "f,f,f,e",
        ),
        ("unicode", "😀 café\n10:00 工作 #café\n- [✓] 完了", ".,e,t"),
        (
            "unicode-code",
            "😀 `é\n10:00 隠す\n終`\n11:00 visible",
            "x,x,x,e",
        ),
        ("emoji", "10:00 :unknown_NAME: :SMILE: :smile:", "e"),
        (
            "sub-super",
            "10:00 ~sub~ ^super^ ~with space~ ^with space^",
            "e",
        ),
        ("escaped-delimiters", "10:00 \\~sub\\~ \\^super\\^", "e"),
        (
            "link-format",
            "10:00 [~sub~ ^super^ :UNKNOWN:](<note.md>)",
            "e",
        ),
        (
            "autolinks",
            "10:00 www.example.org/a#tag user@example.org <https://x.org>",
            "e",
        ),
        (
            "table",
            "| when | task |\n| --- | --- |\n| 10:00 | [ ] hidden |",
            ".,.,.",
        ),
        ("heading", "# 10:00 not-event\n\n10:00 visible", ".,.,e"),
        ("escaped-backticks", "10:00 \\`literal\\`", "e"),
        ("code-link", "10:00 [`label`](<note.md>)", "x"),
        (
            "comment-child",
            "- 10:00 parent\n  <!--\n  - [ ] hidden\n  -->\n  - [ ] visible",
            "e,x,x,x,t",
        ),
    ];
    for (id, source, expected) in vectors {
        let lines: Vec<_> = source.lines().collect();
        let regions = regions(&lines);
        let items = scan_note_lines(source).unwrap();
        let mut found: Vec<_> = regions
            .iter()
            .map(|region| match region {
                Region::Body => ".".into(),
                Region::Excluded => "x".into(),
                Region::Frontmatter => "f".into(),
            })
            .collect::<Vec<String>>();
        for item in items {
            let label = &mut found[item.number - 1];
            if label == "." {
                label.clear();
            }
            label.push(match item.item {
                LineItem::Checkbox { .. } => 't',
                LineItem::Time { .. } => 'e',
            });
        }
        if source.ends_with('\n') {
            found.push(".".into());
        }
        assert_eq!(found.join(","), expected, "{id}");
    }
}
