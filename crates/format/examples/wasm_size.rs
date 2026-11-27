//! Link the four host entry points into one Wasm binary for size measurement.

use std::hint::black_box;

use calternal_format::{
    EvalContext, Fields, InstallOptions, OperationInput, Page, PluginFile, Row, apply_operation,
    check_item, evaluate_query, parse_agent_rules, parse_item, parse_plugin, parse_records,
    write_item,
};

const MANIFEST: &str =
    "---\nplugin: sample\nname: Sample\nformat: 1\nversion: 1.0.0\n---\nSample measurement.\n";
const KIND: &str = "---\nkind: entries\nname: Entry\nformat: 1\nfolder: Entries\neditable: outside\nfields:\n  title: text, required\n  date: date, default today\n  done: yes/no, default no\n  late: = date < today() and not done\noperations:\n  finish: set done = yes\n---\nSample entries.\n";
const COLLECTION: &str = "---\ncollection: all\ntitle: All\nformat: 1\nfrom: entries\nwhere: not done\nsort: date desc\nviews:\n  list: title, late\n---\n";

fn main() {
    if let Err(error) = run() {
        black_box(error.to_string());
    }
}

fn run() -> Result<(), calternal_format::Diagnostic> {
    let plugin = parse_plugin(
        black_box(&[
            PluginFile {
                path: "plugin.md",
                text: Some(MANIFEST),
            },
            PluginFile {
                path: "kinds/entries.md",
                text: Some(KIND),
            },
            PluginFile {
                path: "collections/all.md",
                text: Some(COLLECTION),
            },
        ]),
        black_box(&InstallOptions::default()),
    )?;
    let kind = &plugin.kinds["entries"];
    let context = EvalContext {
        today: Some("2026-11-27"),
        ..EvalContext::default()
    };
    let item = parse_item(black_box("---\ntitle: Sample\n---\n# Sample\n"))?;
    black_box(parse_records(kind, black_box(&item))?);
    black_box(parse_agent_rules(black_box(
        "---\\nrules: agents\\nformat: 1\\n---\\n",
    ))?);
    check_item(kind, &item, black_box(&context))?;
    let metadata = Fields::new();
    let row = Row {
        fields: item.fields(),
        metadata: &metadata,
    };
    black_box(evaluate_query(
        &plugin.collections["all"],
        kind,
        black_box(&[row]),
        Page::default(),
        &context,
    )?);
    let changes = apply_operation(
        kind,
        black_box("finish"),
        &OperationInput {
            item: Some(row),
            fields: &metadata,
            record: None,
        },
        &context,
    )?;
    if let [calternal_format::Change::Update(fields)] = changes.as_slice() {
        black_box(write_item(&item, fields)?);
    }
    Ok(())
}
