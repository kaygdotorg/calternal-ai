//! Revision 10 presentation syntax and install-time checks.
use calternal_format::{Diagnostic, FillBinding, InstallOptions, Plugin, PluginFile, parse_plugin};

const MANIFEST: &str = r#"plugin: example
name: Example
version: 1.0.0
format: 1
slots:
  editor: single, gives text message, draft state.draft, quote reply, picked by person, inputs notes.send, label "Write, edit or quote"
fills:
  example:editor: operation notes.send, as "A \"quoted\" title""#;
const KIND: &str = "kind: notes
name: Notes
format: 1
folder: Notes
editable: operations
fields:
  score: number kg, badge
  counted: = count(records), badge
  total: = this.counted + score, badge
records:
  count: number
state:
  draft: long text
operations:
  send: code
    inputs:
      message: long text, required
      reply: link(notes records)";
const COLLECTION: &str = r#"collection: saved
title: Saved
format: 1
icon: archive
from: notes
views:
  strip: if(score > 0, score, 0), total"#;
const SIDEBAR: &str = r#"---
sidebar: primary
title: Primary
format: 1
---
- [Saved, recent](<../collections/saved.md>), icon: archive
- [Default](<../collections/saved.md>)"#;

fn plugin(
    manifest: &str,
    kind: &str,
    collection: &str,
    sidebar: &str,
) -> Result<Plugin, Diagnostic> {
    let manifest = format!("---\n{manifest}\n---\n");
    let kind = format!("---\n{kind}\n---\nStores notes.\n");
    let collection = format!("---\n{collection}\n---\n");
    let files = [
        PluginFile {
            path: "plugin.md",
            text: Some(&manifest),
        },
        PluginFile {
            path: "kinds/notes.md",
            text: Some(&kind),
        },
        PluginFile {
            path: "collections/saved.md",
            text: Some(&collection),
        },
        PluginFile {
            path: "sidebars/primary.md",
            text: Some(sidebar),
        },
        PluginFile {
            path: "operations.ts",
            text: Some(""),
        },
    ];
    parse_plugin(&files, &InstallOptions::default())
}

#[test]
fn compiles_revision_10_metadata_at_install() {
    let plugin = plugin(MANIFEST, KIND, COLLECTION, SIDEBAR).unwrap();
    let slot = &plugin.manifest.slots["editor"];
    let composer = slot.composer.as_ref().unwrap();
    assert_eq!(slot.label.as_deref(), Some("Write, edit or quote"));
    assert_eq!(slot.input_operation.as_deref(), Some("notes.send"));
    assert_eq!(composer.text, "message");
    assert_eq!(composer.draft.as_deref(), Some("draft"));
    assert_eq!(composer.quote.as_deref(), Some("reply"));
    let fill = &plugin.manifest.fills[0];
    assert!(matches!(&fill.binding, FillBinding::Operation(target) if target == "notes.send"));
    assert_eq!(fill.label.as_deref(), Some("A \"quoted\" title"));
    assert_eq!(
        plugin.manifest.operation_fill("example", "editor"),
        Some("notes.send")
    );
    let kind = &plugin.kinds["notes"];
    for key in ["score", "counted", "total"] {
        assert!(kind.fields[key].is_badge());
    }
    let collection = &plugin.collections["saved"];
    assert_eq!(collection.icon.as_deref(), Some("archive"));
    assert_eq!(collection.compiled_views[0].kind, "strip");
    assert_eq!(
        collection.compiled_views[0].columns,
        ["if(score > 0, score, 0)", "total"]
    );
    let entries = &plugin.sidebars["primary"].entries;
    assert_eq!(entries[0].title.as_deref(), Some("Saved, recent"));
    assert_eq!(entries[0].icon.as_deref(), Some("archive"));
    assert_eq!(entries[1].icon, None);
}

#[test]
fn a_composer_can_bind_only_text_and_a_quoted_gives_remains_supported() {
    let no_optional = MANIFEST.replace(", draft state.draft, quote reply", "");
    let compiled = plugin(&no_optional, KIND, COLLECTION, SIDEBAR).unwrap();
    let bindings = compiled.manifest.slots["editor"].composer.as_ref().unwrap();
    assert_eq!((&bindings.draft, &bindings.quote), (&None, &None));
    let descriptive = no_optional.replace("gives text message", "gives \"the message\"");
    let compiled = plugin(&descriptive, KIND, COLLECTION, SIDEBAR).unwrap();
    assert_eq!(compiled.manifest.slots["editor"].gives, "the message");
    assert!(compiled.manifest.slots["editor"].composer.is_none());
}

fn at(error: Diagnostic, path: &str, source: &str, marker: &str, frontmatter: bool, message: &str) {
    let offset = source.find(marker).unwrap();
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|&byte| byte == b'\n').count() + 1 + usize::from(frontmatter);
    let column = prefix.rsplit('\n').next().unwrap().chars().count() + 1;
    assert_eq!(
        (error.path.as_deref(), error.line, error.column),
        (Some(path), line, column),
        "{error}"
    );
    assert!(error.message.contains(message), "{error}");
}

#[test]
fn refuses_invalid_badges_icons_and_strip_columns_at_the_clause() {
    for (old, replacement, marker, message) in [
        (
            "number kg, badge",
            "text, badge",
            "badge",
            "field flag badge",
        ),
        (
            "count(records), badge",
            "not yes, badge",
            "badge\n  total",
            "numeric result",
        ),
        (
            "count(records), badge",
            "count(records), mark, badge",
            "badge\n  total",
            "field flag badge",
        ),
        (
            "number kg, badge",
            "number kg, badge, badge",
            "badge\n  counted",
            "field flag badge",
        ),
    ] {
        let source = KIND.replace(old, replacement);
        at(
            plugin(MANIFEST, &source, COLLECTION, SIDEBAR).unwrap_err(),
            "kinds/notes.md",
            &source,
            marker,
            true,
            message,
        );
    }
    let invalid_icon = COLLECTION.replace("icon: archive", "icon: https://example.org");
    at(
        plugin(MANIFEST, KIND, &invalid_icon, SIDEBAR).unwrap_err(),
        "collections/saved.md",
        &invalid_icon,
        "https:",
        true,
        "Use an ID",
    );
    let invalid_column = COLLECTION.replace("), total", "), @");
    at(
        plugin(MANIFEST, KIND, &invalid_column, SIDEBAR).unwrap_err(),
        "collections/saved.md",
        &invalid_column,
        "@",
        true,
        "formula",
    );
    let invalid_entry = SIDEBAR.replace("icon: archive", "icon: Archive");
    at(
        plugin(MANIFEST, KIND, COLLECTION, &invalid_entry).unwrap_err(),
        "sidebars/primary.md",
        &invalid_entry,
        "Archive",
        false,
        "Use an ID",
    );
}

#[test]
fn refuses_invalid_slot_and_fill_clauses_at_the_binding() {
    for (old, replacement, marker, message) in [
        (
            "text message",
            "text unknown",
            "unknown",
            "text operation input",
        ),
        (
            "state.draft",
            "state.missing",
            "missing",
            "operation's kind",
        ),
        (
            "quote reply",
            "quote message",
            "message, picked",
            "record-link",
        ),
        (
            "inputs notes.send, ",
            "",
            "text message",
            "typed inputs operation",
        ),
        (
            "label \"Write, edit or quote\"",
            "label Write",
            "Write",
            "double quotes",
        ),
        (
            "as \"A \\\"quoted\\\" title\"",
            "as Unquoted",
            "Unquoted",
            "double quotes",
        ),
    ] {
        let manifest = MANIFEST.replace(old, replacement);
        at(
            plugin(&manifest, KIND, COLLECTION, SIDEBAR).unwrap_err(),
            "plugin.md",
            &manifest,
            marker,
            true,
            message,
        );
    }
}

#[test]
fn continued_clauses_point_to_the_physical_line() {
    let manifest = MANIFEST.replace(
        ", label \"Write, edit or quote\"",
        ",\n      label Unquoted",
    );
    at(
        plugin(&manifest, KIND, COLLECTION, SIDEBAR).unwrap_err(),
        "plugin.md",
        &manifest,
        "Unquoted",
        true,
        "double quotes",
    );

    let kind = KIND.replace("count(records), badge", "not yes,\n      badge");
    at(
        plugin(MANIFEST, &kind, COLLECTION, SIDEBAR).unwrap_err(),
        "kinds/notes.md",
        &kind,
        "badge\n  total",
        true,
        "numeric result",
    );
}
