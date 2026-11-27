//! Reading List extension vectors from §7.2.

use calternal_format::{
    Change, EvalContext, Extension, Fields, InstallOptions, OperationInput, PluginFile, Row, Value,
    parse_plugin,
};

const MAIL: &str = "---\nplugin: mail\nname: Mail\nformat: 1\nversion: 1.0.0\n---\nMail.\n";
const MESSAGES: &str = "\
---
kind: messages
name: Message
format: 1
folder: Mail
editable: operations
protected: from, outside sender
fields:
  from: text
  outside sender: yes/no
  saved: text
  url: url
operations:
  remove
---
Messages.
";
const SIDEBAR: &str = "---\nsidebar: inbox\ntitle: Inbox\nformat: 1\n---\n- section: Messages\n";
const COLLECTION: &str = "---\ncollection: unread\ntitle: Unread\nformat: 1\nfrom: reading\nviews:\n  list: title\n---\n";
const READING: &str = "---\nkind: reading\nname: Reading\nformat: 1\nfolder: Reading\neditable: outside\nfields:\n  title: text, required\n---\nReading List.\n";
const EXTENSIONS: &str = r#"---
plugin: reading-list
name: Reading List
format: 1
version: 1.0.0
extends:
  [mail.messages]:
    fields:
      saved: yes/no, default no
      note: long text
      newsletter: = contains(from, "newsletter") and saved, mark
    operations:
      send to reading list: code, menu
      mark saved: set saved = yes, menu
      open related: open where saved = yes, menu
    together:
      saved, note
    row: + saved
    page: + section "Reading List": saved, newsletter
  any kind with url:
    operations:
      save link: code, menu
  mail:sidebars/inbox:
    - [To read](<collections/unread.md>)
  home:
    - [On this day](<collections/unread.md>)
---
Reading List contributions.
"#;

fn mail() -> calternal_format::Plugin {
    parse_plugin(
        &[
            PluginFile {
                path: "plugin.md",
                text: Some(MAIL),
            },
            PluginFile {
                path: "kinds/messages.md",
                text: Some(MESSAGES),
            },
            PluginFile {
                path: "sidebars/inbox.md",
                text: Some(SIDEBAR),
            },
        ],
        &InstallOptions::default(),
    )
    .unwrap()
}

fn reading(
    manifest: &str,
    mail: &calternal_format::Plugin,
) -> Result<calternal_format::Plugin, calternal_format::Diagnostic> {
    parse_plugin(
        &[
            PluginFile {
                path: "plugin.md",
                text: Some(manifest),
            },
            PluginFile {
                path: "collections/unread.md",
                text: Some(COLLECTION),
            },
            PluginFile {
                path: "kinds/reading.md",
                text: Some(READING),
            },
            PluginFile {
                path: "operations.ts",
                text: None,
            },
        ],
        &InstallOptions {
            installed: std::slice::from_ref(mail),
            ..Default::default()
        },
    )
}

#[test]
fn extensions_use_qualified_writes_and_local_field_precedence() {
    let mail = mail();
    let kind = &mail.kinds["messages"];
    let plugin = reading(EXTENSIONS, &mail).unwrap();
    let [
        Extension::Kind(ext),
        Extension::Kind(dynamic),
        Extension::Sidebar(sidebar),
        Extension::Sidebar(home),
    ] = plugin.manifest.extensions.as_slice()
    else {
        panic!("unexpected extension declarations")
    };
    assert!(dynamic.target.matches("mail", kind));
    assert_eq!(sidebar.sidebar.as_ref().unwrap().1, "inbox");
    assert!(home.sidebar.is_none());
    assert_eq!(sidebar.entries[0].collection.as_deref(), Some("unread"));
    assert_eq!(ext.page[0].heading.as_deref(), Some("Reading List"));
    assert!(ext.fields["newsletter"].mark);
    assert_eq!(
        ext.storage_key("reading-list", "saved").as_deref(),
        Some("reading-list.saved")
    );
    let fields = Fields::from([
        ("from".into(), Value::Text("newsletter@example.test".into())),
        ("saved".into(), Value::Text("old owner's field".into())),
        ("reading-list.saved".into(), Value::Bool(true)),
    ]);
    let empty = Fields::new();
    let row = Row {
        fields: &fields,
        metadata: &empty,
    };
    let context = EvalContext::default();
    assert_eq!(
        ext.compute_fields("reading-list", "mail", kind, row, &context)
            .unwrap()["reading-list.newsletter"],
        Value::Bool(true)
    );
    let input = OperationInput {
        item: Some(row),
        fields: &empty,
        record: None,
    };
    let plan = |name| {
        ext.plan_operation("reading-list", "mail", kind, name, &input, &context)
            .unwrap()
    };
    assert!(
        matches!(&plan("mark saved")[..], [Change::Update(fields)] if fields == &Fields::from([("reading-list.saved".into(), Value::Bool(true))]))
    );
    assert!(matches!(
        &plan("open related")[..],
        [Change::Open { create: None, .. }]
    ));
    assert!(matches!(
        &plan("send to reading list")[..],
        [Change::Dispatch { .. }]
    ));
}

#[test]
fn extensions_cannot_replace_protected_fields_or_write_owner_state() {
    let mail = mail();
    for (old, replacement) in [
        ("row: + saved", "row: + from"),
        ("set saved = yes", "set outside sender = yes"),
        ("set saved = yes", "set state.saved = yes"),
        ("code, menu", "code, swipe"),
        ("open where saved = yes", "timer 1d after mark saved"),
        (
            "contains(from, \"newsletter\")",
            "count(from(messages.link)) > 0",
        ),
        ("saved, note", "saved, from"),
    ] {
        assert!(
            reading(&EXTENSIONS.replace(old, replacement), &mail).is_err(),
            "{replacement}"
        );
    }
}
