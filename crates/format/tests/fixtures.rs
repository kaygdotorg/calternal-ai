use calternal_format::{
    Change, EvalContext, Fields, InstallOptions, OperationInput, Page, Plugin, PluginFile,
    QueryRowContext, Row, Value, apply_operation, check_item, evaluate, evaluate_query,
    parse_agent_rules, parse_expression, parse_item, parse_plugin, write_item,
};

const CHAT: &[PluginFile<'_>] = &[
    PluginFile {
        path: "plugin.md",
        text: Some(include_str!("../../../plugins-v2/calternal-chat/plugin.md")),
    },
    PluginFile {
        path: "kinds/chats.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/kinds/chats.md"
        )),
    },
    PluginFile {
        path: "collections/chats.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/collections/chats.md"
        )),
    },
    PluginFile {
        path: "collections/pinned.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/collections/pinned.md"
        )),
    },
    PluginFile {
        path: "collections/groups.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/collections/groups.md"
        )),
    },
    PluginFile {
        path: "collections/people.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/collections/people.md"
        )),
    },
    PluginFile {
        path: "collections/archived.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/collections/archived.md"
        )),
    },
    PluginFile {
        path: "sidebars/chat.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/sidebars/chat.md"
        )),
    },
    PluginFile {
        path: "operations.ts",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/operations.ts"
        )),
    },
    PluginFile {
        path: "templates/conversation.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat/templates/conversation.md"
        )),
    },
];

const HARNESS: &[PluginFile<'_>] = &[
    PluginFile {
        path: "plugin.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat-claude-code/plugin.md"
        )),
    },
    PluginFile {
        path: "kinds/runs.md",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat-claude-code/kinds/runs.md"
        )),
    },
    PluginFile {
        path: "operations.ts",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat-claude-code/operations.ts"
        )),
    },
    PluginFile {
        path: "programs/calternal-provider-claude-code",
        text: Some(include_str!(
            "../../../plugins-v2/calternal-chat-claude-code/programs/calternal-provider-claude-code"
        )),
    },
];

const OFFICIAL: &[(&str, &str)] = &[
    ("calternal-chat", "Chat"),
    ("calternal-chat-claude-code", "Claude Code"),
    ("calternal-reading-list", "Reading List"),
];
const SPEC: &str = include_str!("../../../docs/proposals/kind-and-collection-format-2026-11-24.md");

fn chat() -> Plugin {
    parse_plugin(
        CHAT,
        &InstallOptions {
            official: OFFICIAL,
            ..InstallOptions::default()
        },
    )
    .unwrap()
}

#[test]
fn chat_add_validate_query_and_write() {
    let plugin = chat();
    let kind = &plugin.kinds["chats"];
    assert_eq!(plugin.collections.len(), 5);
    assert!(kind.fields["unread"].is_per_viewer());
    assert_eq!(plugin.sidebars["chat"].entries.len(), 5);
    assert!(!plugin.description_only);
    let context = EvalContext::default();
    let inputs = Fields::from([("title".into(), Value::Text("First conversation".into()))]);
    let changes = apply_operation(
        kind,
        "add",
        &OperationInput {
            item: None,
            fields: &inputs,
            record: None,
        },
        &context,
    )
    .unwrap();
    let [Change::Add(fields)] = changes.as_slice() else {
        panic!("{changes:?}")
    };
    assert_eq!(fields["archived"], Value::Bool(false));
    let source = write_item(&parse_item("").unwrap(), fields).unwrap();
    let item = parse_item(&source).unwrap();
    check_item(kind, &item, &context).unwrap();
    let metadata = Fields::from([("id".into(), Value::Text("id-1".into()))]);
    let rows = [Row {
        fields: item.fields(),
        metadata: &metadata,
    }];
    let result = evaluate_query(
        &plugin.collections["chats"],
        kind,
        &rows,
        Page::default(),
        &context,
    )
    .unwrap();
    assert_eq!(result.matched, 1);
    assert_eq!(result.rows[0].index, 0);
    assert_eq!(
        result.rows[0].computed["unread"],
        Value::Number("0".parse().unwrap())
    );
    let pinned = Fields::from([("pinned".into(), Value::Bool(true))]);
    let row_contexts = [(
        "id-1".into(),
        QueryRowContext {
            state: Some(&pinned),
            ..QueryRowContext::default()
        },
    )]
    .into();
    let context = EvalContext {
        rows: Some(&row_contexts),
        ..context
    };
    assert_eq!(
        evaluate_query(
            &plugin.collections["pinned"],
            kind,
            &rows,
            Page::default(),
            &context
        )
        .unwrap()
        .matched,
        1
    );
}

#[test]
fn claude_code_reads_its_fixture_and_needs_a_bound_program() {
    let chat = chat();
    let options = InstallOptions {
        official: OFFICIAL,
        installed: &[chat],
        ..InstallOptions::default()
    };
    let harness = parse_plugin(HARNESS, &options).unwrap();
    assert!(harness.kinds["runs"].hidden);
    let inputs = &harness.kinds["runs"].operations["start"].inputs;
    assert_eq!(
        inputs.keys().map(String::as_str).collect::<Vec<_>>(),
        ["chat", "prompt"]
    );
    for (key, target) in [
        ("chat", "calternal-chat.chats"),
        ("prompt", "calternal-chat.chats records"),
    ] {
        assert!(inputs[key].required);
        assert_eq!(
            inputs[key].field_type,
            Some(calternal_format::FieldType::Link(target.into()))
        );
    }
    let program = &harness.manifest.programs["calternal-provider-claude-code"];
    assert_eq!(
        program.path.as_deref(),
        Some("programs/calternal-provider-claude-code")
    );
    assert!(!program.image);
    let missing = &HARNESS[..HARNESS.len() - 1];
    assert_eq!(
        parse_plugin(missing, &options).unwrap_err().path.as_deref(),
        Some("plugin.md")
    );
}

#[test]
fn reply_declarations_select_existing_calls_on_the_slot_owner() {
    let options = InstallOptions {
        official: OFFICIAL,
        installed: &[chat()],
        ..InstallOptions::default()
    };
    let source = HARNESS[0].text.unwrap();
    let reply = "replies: calternal-chat.chats.send";
    let parse = |manifest: &str| {
        let mut files = HARNESS.to_vec();
        files[0].text = Some(manifest);
        parse_plugin(&files, &options)
    };
    assert_eq!(
        parse(source).unwrap().manifest.replies["calternal-chat.chats"],
        "calternal-chat.chats.send"
    );
    assert!(
        parse(&source.replace(reply, ""))
            .unwrap()
            .manifest
            .replies
            .is_empty()
    );
    for replacement in [
        "replies: calternal-chat.send",
        "replies: calternal-chat.chats.rename",
        "replies: calternal-chat.chats.send, calternal-chat.chats.rename",
    ] {
        assert_eq!(
            parse(&source.replace(reply, replacement))
                .unwrap_err()
                .path
                .as_deref(),
            Some("plugin.md")
        );
    }
    assert!(
        parse(&source.replace(
            "calls: calternal-chat.chats.send",
            "calls: calternal-chat.chats.rename",
        ))
        .is_err()
    );
    assert!(parse(&source.replace("calternal-chat:harness:", "calternal-chat:missing:")).is_err());
}

#[test]
fn ledger_order_distinguishes_messages_with_identical_timestamps() {
    let fields = Fields::new();
    let stamps = ["k2m8q1w4zre7k3f8", "k2m8q1w4zre7k3f9", "k2m8q1w4zre7k3fa"].map(|anchor| {
        Fields::from([
            ("anchor".into(), Value::Text(anchor.into())),
            ("at".into(), Value::Time("2026-11-27T09:00:00Z".into())),
        ])
    });
    let records = stamps.each_ref().map(|metadata| Row {
        fields: &fields,
        metadata,
    });
    let state = Fields::from([("read through".into(), stamps[1]["anchor"].clone())]);
    let context = EvalContext {
        records: &records,
        state: Some(&state),
        ..EvalContext::default()
    };
    let expression = parse_expression("count(records after state.read through)").unwrap();
    assert_eq!(
        evaluate(
            &expression,
            Row {
                fields: &fields,
                metadata: &fields
            },
            &context
        )
        .unwrap(),
        Value::Number("1".parse().unwrap())
    );
    let redacted = [records[0], records[2]];
    let positions = [("k2m8q1w4zre7k3f9".into(), 1)].into();
    let context = EvalContext {
        records: &redacted,
        record_positions: Some(&positions),
        ..context
    };
    assert_eq!(
        evaluate(
            &expression,
            Row {
                fields: &fields,
                metadata: &fields
            },
            &context
        )
        .unwrap(),
        Value::Number("1".parse().unwrap())
    );
}

fn blocks(source: &str) -> impl Iterator<Item = &str> {
    source
        .split("```")
        .skip(1)
        .step_by(2)
        .filter_map(|block| block.strip_prefix("markdown\n"))
}

#[test]
fn reading_list_and_appendix_b() {
    let reading = SPEC
        .split_once("## 16. Reading List, written out in full")
        .unwrap()
        .1
        .split_once("## 17.")
        .unwrap()
        .0;
    let mut files = Vec::new();
    for block in blocks(reading) {
        let (entry, document) = block.split_once('\n').unwrap();
        let path = entry
            .strip_prefix("<!-- ")
            .and_then(|entry| entry.strip_suffix(" -->"))
            .unwrap();
        files.push(PluginFile {
            path,
            text: Some(document),
        });
    }
    files.push(PluginFile {
        path: "operations.ts",
        text: None,
    });
    let plugin = parse_plugin(
        &files,
        &InstallOptions {
            official: OFFICIAL,
            ..InstallOptions::default()
        },
    )
    .unwrap();
    let kind = &plugin.kinds["reading"];
    let item = parse_item("---\ntitle: Saved\nurl: https://example.test/link\n---\n").unwrap();
    let metadata = Fields::new();
    let row = Row {
        fields: item.fields(),
        metadata: &metadata,
    };
    let result = evaluate_query(
        &plugin.collections["unread"],
        kind,
        &[row],
        Page::default(),
        &EvalContext::default(),
    )
    .unwrap();
    assert_eq!(result.rows.len(), 1);
    let changes = apply_operation(
        kind,
        "mark read",
        &OperationInput {
            item: Some(row),
            fields: &metadata,
            record: None,
        },
        &EvalContext::default(),
    )
    .unwrap();
    assert!(
        matches!(changes.as_slice(), [Change::Update(fields)] if fields["read"] == Value::Bool(true))
    );

    let appendix = SPEC
        .split_once("## Appendix B. The four kinds rewritten")
        .unwrap()
        .1
        .split_once("## Appendix C.")
        .unwrap()
        .0;
    let manifest =
        "---\nplugin: example\nname: Example\nversion: 1.0.0\nformat: 1\n---\nExample.\n";
    let mut count = 0;
    for document in blocks(appendix).filter(|block| block.starts_with("---\nkind: ")) {
        let id = document
            .lines()
            .nth(1)
            .unwrap()
            .strip_prefix("kind: ")
            .unwrap();
        let path = format!("kinds/{id}.md");
        let files = [
            PluginFile {
                path: "plugin.md",
                text: Some(manifest),
            },
            PluginFile {
                path: "operations.ts",
                text: None,
            },
            PluginFile {
                path: &path,
                text: Some(document),
            },
        ];
        let compiled = parse_plugin(&files, &InstallOptions::default())
            .unwrap_or_else(|error| panic!("{id}: {error}"));
        let kind = &compiled.kinds[id];
        if id == "events" {
            assert_eq!(kind.line_claim.as_ref().unwrap().targets, ["days"]);
            assert_eq!(kind.dav[0].properties.len(), 8);
            let lines =
                calternal_format::scan_note_lines("23:30 - 01:00 Late session\nall day Away")
                    .unwrap();
            let claim = kind.line_claim.as_ref().unwrap();
            let timed = claim.time_fields(&lines[0].item, "2026-11-27").unwrap();
            assert_eq!(timed["end"], Value::Time("2026-11-28T01:00:00".into()));
            let all_day = claim.time_fields(&lines[1].item, "2026-11-27").unwrap();
            assert_eq!(all_day["start"], Value::Date("2026-11-27".into()));
        }
        if id == "tasks" {
            let claim = kind.line_claim.as_ref().unwrap();
            assert_eq!(
                claim.targets,
                ["calternal-notes.notes", "calternal-calendar.days"]
            );
            assert_eq!(
                claim.checkbox.as_ref().unwrap().value('X'),
                Some(&Value::Text("done".into()))
            );
        }
        count += 1;
    }
    assert_eq!(count, 3);
}

#[test]
fn agent_rules_need_qualified_collections_or_folder_suffixes() {
    let source = "---\nrules: agents\nformat: 1\n---\n- deny calternal-money.* on transactions where amount > 500 EUR\n- allow change in Notes/Work/\n- deny fetch github.com for calternal-chat-codex\n";
    assert_eq!(parse_agent_rules(source).unwrap().rules.len(), 3);
    let error = parse_agent_rules(&source.replace("Notes/Work/", "today")).unwrap_err();
    assert_eq!(error.line, 6);
}

#[test]
fn record_body_starts_at_the_first_undeclared_field() {
    let plugin = chat();
    let kind = &plugin.kinds["chats"];
    let source = "---\ntitle: Example\n---\nBefore records.\n**Lee** · 2026-11-20 17:02 ^k2m8q1w4zre7k3f9 <!--ca1 AAAAAAAAAAAAAAAAAAAAAA-->\nmentions: [user]\nNote: felt heavy\nmentions: body text\n---\n**Ari** · 2026-11-20 17:05 ^k2m8q1w4zre7k3fa\nSecond.\n";
    let item = parse_item(source).unwrap();
    let records = calternal_format::parse_records(kind, &item).unwrap();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].anchor, "k2m8q1w4zre7k3f9");
    assert_eq!(records[1].anchor, "k2m8q1w4zre7k3fa");
    assert_eq!(records[0].body, "Note: felt heavy\nmentions: body text\n");
    assert_eq!(
        records[0].fields["mentions"],
        Value::List(vec![Value::Text("user".into())])
    );
    assert!(source[records[0].range.clone()].starts_with("**Lee**"));
    let fields = Fields::from([("title".into(), Value::Text("Renamed".into()))]);
    let edited = write_item(&item, &fields).unwrap();
    assert_eq!(
        edited,
        source.replace("title: Example", "title: \"Renamed\"")
    );
    assert!(edited.contains(" <!--ca1 AAAAAAAAAAAAAAAAAAAAAA-->"));
    assert_eq!(records[1].body, "Second.\n");
}

#[test]
fn revision_nine_rights_sources_and_typed_inputs() {
    let original = CHAT[1].text.unwrap();
    let changed = original
        .replace(
            "editable: operations",
            "editable: operations\nrights:\n  change manager\n  read editor, bicycle-club",
        )
        .replace(
            "      text: long text, required",
            "      body: long text, required",
        );
    let changed = changed.replace("read through: time", "read through: record anchor");
    let mut files = CHAT.to_vec();
    files[1].text = Some(&changed);
    let manifest = CHAT[0]
        .text
        .unwrap()
        .replace("gives text text", "gives text body");
    files[0].text = Some(&manifest);
    let collection = files[2]
        .text
        .unwrap()
        .replace("from: chats", "from: chats, shared with me");
    files[2].text = Some(&collection);
    let plugin = parse_plugin(
        &files,
        &InstallOptions {
            official: OFFICIAL,
            ..InstallOptions::default()
        },
    )
    .unwrap();
    let kind = &plugin.kinds["chats"];
    assert_eq!(
        kind.state["read through"].field_type,
        Some(calternal_format::FieldType::Anchor)
    );
    assert_eq!(kind.rights.change.as_deref(), Some("manager"));
    assert_eq!(kind.rights.read, ["editor", "bicycle-club"]);
    assert_eq!(
        plugin.collections["chats"].share_source,
        calternal_format::ShareSource::Shared
    );
    let fields = Fields::from([
        ("body".into(), Value::Text("First\nmessage".into())),
        (
            "item".into(),
            Value::Text("019a0000-0000-7000-8000-000000000001".into()),
        ),
    ]);
    let empty = Fields::new();
    let row = Row {
        fields: &empty,
        metadata: &empty,
    };
    let changes = apply_operation(
        kind,
        "send",
        &OperationInput {
            item: Some(row),
            fields: &fields,
            record: None,
        },
        &EvalContext::default(),
    )
    .unwrap();
    assert!(
        matches!(&changes[..], [Change::Dispatch { operation, fields: output }] if operation == "send" && *output == fields)
    );
    let refused = changed.replace("harness: text", "password: text");
    files[1].text = Some(&refused);
    let error = parse_plugin(
        &files,
        &InstallOptions {
            official: OFFICIAL,
            ..InstallOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.path.as_deref(), Some("kinds/chats.md"));
    assert!(error.message.contains("reserved"));
}

#[test]
fn appendix_item_and_link_comparisons() {
    let item =
        parse_item("---\ncalternal-reading-list: yes\ncalternal-reading-list.saved: true\n---\n")
            .unwrap();
    assert_eq!(
        item.fields()["calternal-reading-list.saved"],
        Value::Bool(true)
    );
    assert_eq!(
        item.fields()["calternal-reading-list"],
        Value::Text("yes".into())
    );
    let fields = Fields::new();
    let metadata = Fields::from([("id".into(), Value::Text("project".into()))]);
    let row = Row {
        fields: &fields,
        metadata: &metadata,
    };
    let context = EvalContext {
        open_item: Some(row),
        outgoing_links: &["project"],
        ..EvalContext::default()
    };
    assert_eq!(
        evaluate(
            &parse_expression("links to open item").unwrap(),
            row,
            &context
        )
        .unwrap(),
        Value::Bool(true)
    );
    assert!(parse_expression("project.owner.name").is_err());
}

#[test]
fn refusal_uses_only_the_installed_operation_schema() {
    let original = CHAT[1].text.unwrap();
    let changed = original.replace(
        "      text: long text, required",
        "      body: long text, required",
    );
    let mut chat_files = CHAT.to_vec();
    chat_files[1].text = Some(&changed);
    let manifest = CHAT[0]
        .text
        .unwrap()
        .replace("gives text text", "gives text body")
        .replace("inputs chats.send", "inputs send");
    chat_files[0].text = Some(&manifest);
    let chat = parse_plugin(
        &chat_files,
        &InstallOptions {
            official: OFFICIAL,
            ..InstallOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        chat.manifest.slots["composer"].input_operation.as_deref(),
        Some("send")
    );
    let installed = [chat];
    let options = InstallOptions {
        installed: &installed,
        ..InstallOptions::default()
    };
    let manifest = "---\nplugin: filters\nname: Filters\nformat: 1\nversion: 1.0.0\nrefuses:\n  [calternal-chat.send]:\n    contains(body, \"test\"), \"Please choose a different message\"\n---\nProvides a message rule.\n";
    let parsed = parse_plugin(
        &[PluginFile {
            path: "plugin.md",
            text: Some(manifest),
        }],
        &options,
    )
    .unwrap();
    assert_eq!(parsed.manifest.refusals.len(), 1);
    assert!(!parsed.description_only);
    let bad = manifest.replace("contains(body", "contains(state.draft");
    let error = parse_plugin(
        &[PluginFile {
            path: "plugin.md",
            text: Some(&bad),
        }],
        &options,
    )
    .unwrap_err();
    assert_eq!((error.path.as_deref(), error.line), (Some("plugin.md"), 8));
}

#[test]
fn viewer_fields_cannot_enter_shared_search() {
    let original = CHAT[1].text.unwrap();
    let original = original.replace("records:\n", "  badge: = unread > 0\nrecords:\n");
    let mut files = CHAT.to_vec();
    files[1].text = Some(&original);
    let options = InstallOptions {
        official: OFFICIAL,
        ..InstallOptions::default()
    };
    let plugin = parse_plugin(&files, &options).unwrap();
    let kind = &plugin.kinds["chats"];
    assert!(kind.fields["badge"].is_per_viewer());
    assert!(!kind.fields["title"].is_per_viewer());
    let invalid = original.replace(
        "row: title, people, unread",
        "row: title, people, unread\nsearch: badge",
    );
    files[1].text = Some(&invalid);
    let error = parse_plugin(&files, &options).unwrap_err();
    assert!(error.message.contains("shared search index"));
}

#[test]
fn formulas_use_exact_values_and_explicit_time() {
    let fields = Fields::from([("rule".into(), Value::Text("FREQ=WEEKLY;BYDAY=MO".into()))]);
    let metadata = Fields::new();
    let row = Row {
        fields: &fields,
        metadata: &metadata,
    };
    let context = EvalContext {
        today: Some("2026-11-27"),
        ..EvalContext::default()
    };
    for (formula, expected) in [
        ("0.1 + 0.2", Value::Number("0.3".parse().unwrap())),
        ("first(empty, 1.25)", Value::Number("1.25".parse().unwrap())),
        (
            "next(rule, after: today())",
            Value::Date("2026-11-30".into()),
        ),
        ("date(today() + 1mo)", Value::Date("2026-12-27".into())),
    ] {
        assert_eq!(
            evaluate(&parse_expression(formula).unwrap(), row, &context).unwrap(),
            expected
        );
    }
    let invalid = parse_expression("10 EUR + 10 USD").unwrap();
    assert!(
        evaluate(&invalid, row, &context)
            .unwrap_err()
            .message
            .contains("same currency")
    );
}

#[test]
fn remaining_standalone_specification_examples() {
    let document = |start: &str| {
        blocks(SPEC)
            .find(|block| block.starts_with(start))
            .unwrap_or_else(|| panic!("Missing example starting with {start}"))
    };
    let manifest =
        "---\nplugin: example\nname: Example\nversion: 1.0.0\nformat: 1\n---\nExample.\n";
    let mut files = vec![
        PluginFile {
            path: "plugin.md",
            text: Some(manifest),
        },
        PluginFile {
            path: "kinds/reading.md",
            text: Some(document("---\nkind: reading")),
        },
        PluginFile {
            path: "sidebars/tasks.md",
            text: Some(document("---\nsidebar: tasks")),
        },
    ];
    let collections = ["today", "week", "all-tasks"].map(|id| (
        format!("collections/{id}.md"),
        format!("---\ncollection: {id}\ntitle: Test\nformat: 1\nfrom: files\nviews:\n  list: name\n---\n"),
    ));
    files.extend(collections.iter().map(|(path, text)| PluginFile {
        path,
        text: Some(text),
    }));
    let plugin = parse_plugin(&files, &InstallOptions::default()).unwrap();
    assert_eq!(plugin.kinds["reading"].fields.len(), 1);
    assert_eq!(plugin.sidebars["tasks"].entries.len(), 6);
    let item = parse_item(document("---\ntitle: How Discord")).unwrap();
    assert_eq!(item.fields()["read"], Value::Bool(false));
    let agents = parse_agent_rules(document("---\nrules: agents")).unwrap();
    assert_eq!(agents.rules.len(), 4);
}

#[test]
fn calendar_and_expanding_collections_share_the_window_engine() {
    use calternal_format::{
        CalendarSeries, OccurrenceOverride, expand_calendar_window, parse_repeat,
    };
    let manifest =
        "---\nplugin: calendar\nname: Calendar\nformat: 1\nversion: 1.0.0\n---\nEvents.\n";
    let kind = "---\nkind: entries\nname: Entry\nformat: 1\nfolder: Entries\neditable: outside\nfields:\n  start: when\n  repeat: repeat\n---\nEntries.\n";
    let rule = parse_repeat("FREQ=WEEKLY;BYDAY=MO,TH,FR").unwrap();
    let override_slot = [OccurrenceOverride {
        original: "2026-11-23",
        start: "2026-11-26",
    }];
    let input = [
        CalendarSeries {
            index: 2,
            start: "2026-11-23",
            rule: Some(&rule),
            exclusions: &["2026-11-27"],
            overrides: &override_slot,
        },
        CalendarSeries {
            index: 9,
            start: "2026-11-25T11:00:00",
            rule: None,
            exclusions: &[],
            overrides: &[],
        },
    ];
    for view in [
        "views:\n  calendar: start\n",
        "expand: repeat\nviews:\n  list: start\n",
    ] {
        let collection = format!(
            "---\ncollection: entries\ntitle: Entries\nformat: 1\nfrom: entries\n{view}---\n"
        );
        let plugin = parse_plugin(
            &[
                PluginFile {
                    path: "plugin.md",
                    text: Some(manifest),
                },
                PluginFile {
                    path: "kinds/entries.md",
                    text: Some(kind),
                },
                PluginFile {
                    path: "collections/entries.md",
                    text: Some(&collection),
                },
            ],
            &InstallOptions::default(),
        )
        .unwrap();
        let occurrences = expand_calendar_window(
            &plugin.collections["entries"],
            &input,
            "2026-11-24",
            "2026-11-28",
        )
        .unwrap();
        assert_eq!(
            occurrences
                .iter()
                .map(|slot| slot.index)
                .collect::<Vec<_>>(),
            [9, 2, 2]
        );
        assert_eq!(
            occurrences[1].occurrence.original,
            Value::Date("2026-11-23".into())
        );
    }
}

#[test]
fn contacts_mapping_and_duplicate_line_claims_are_checked() {
    let manifest =
        "---\nplugin: example\nname: Example\nformat: 1\nversion: 1.0.0\n---\nExample.\n";
    let contact = concat!(
        "---\nkind: contacts\nname: Contact\nformat: 1\nfolder: Contacts\neditable: outside\n",
        "fields:\n  title: text, required\n  email: text\n  note: long text\n",
        "vcard:\n  title: FN\n  email: EMAIL\n  note: NOTE\n---\nContacts.\n",
    );
    let files = [
        PluginFile {
            path: "plugin.md",
            text: Some(manifest),
        },
        PluginFile {
            path: "kinds/contacts.md",
            text: Some(contact),
        },
    ];
    let contacts = parse_plugin(&files, &InstallOptions::default()).unwrap();
    assert_eq!(
        contacts.kinds["contacts"].dav[0].component,
        calternal_format::DavComponent::Card
    );
    let unsafe_contact = contact.replace("email: EMAIL", "email: URL");
    let mut invalid = files;
    invalid[1].text = Some(&unsafe_contact);
    assert!(parse_plugin(&invalid, &InstallOptions::default()).is_err());

    let checkboxes = concat!(
        "---\nkind: entries\nname: Entry\nformat: 1\nfolder: Entries\neditable: outside\n",
        "lines: checkbox in [notes.pages]\nfields:\n  done: yes/no\n---\nEntries.\n",
    );
    let other = checkboxes
        .replace("kind: entries", "kind: other")
        .replace("folder: Entries", "folder: Other");
    let files = [
        files[0],
        PluginFile {
            path: "kinds/entries.md",
            text: Some(checkboxes),
        },
        PluginFile {
            path: "kinds/other.md",
            text: Some(&other),
        },
    ];
    let error = parse_plugin(&files, &InstallOptions::default()).unwrap_err();
    assert!(error.message.contains("same line form"));
}
