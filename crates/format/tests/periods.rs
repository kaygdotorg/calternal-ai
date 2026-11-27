//! Money's named totals and civil-period formulas.

use calternal_format::{
    EvalContext, Fields, InstallOptions, Page, Period, PeriodSnapshot, PluginFile, Row, Value,
    evaluate_period_field, evaluate_query, parse_plugin,
};
use std::collections::BTreeMap;

const MANIFEST: &str =
    "---\nplugin: budget\nname: Budget\nversion: 1.0.0\nformat: 1\n---\nBudget example.\n";
const KIND: &str = r#"---
kind: categories
name: Category
format: 1
folder: Budget
editable: outside
fields:
  group: text
  spent: number
  assigned: number
  overspent: number
  available: per month = first(previous(available), 0) + assigned - spent
  daily: per day = period
  weekly: per week = period
  annual: per year = period
---
Budget categories.
"#;
const COLLECTION: &str = r#"---
collection: categories
title: Categories
format: 1
from: categories
where: group = "Income"
totals:
  count
  sum(spent)
  ready to assign: per month = first(previous(ready to assign), 0)
      + sum(all(categories, group = "Income").spent)
      - sum(all(categories, group != "Income").assigned)
      + sum(previous(all(categories).overspent))
views:
  list: group, available
---
"#;

fn number(number: &str) -> Value {
    Value::Number(number.parse().unwrap())
}

fn install(collection: &str) -> Result<calternal_format::Plugin, calternal_format::Diagnostic> {
    parse_plugin(
        &[
            PluginFile {
                path: "plugin.md",
                text: Some(MANIFEST),
            },
            PluginFile {
                path: "kinds/categories.md",
                text: Some(KIND),
            },
            PluginFile {
                path: "collections/categories.md",
                text: Some(collection),
            },
        ],
        &InstallOptions::default(),
    )
}

#[test]
fn money_totals_use_the_whole_kind_and_previous_month() {
    let plugin = install(COLLECTION).unwrap();
    let kind = &plugin.kinds["categories"];
    let category = |group: &str, spent, assigned, overspent| {
        Fields::from([
            ("group".into(), Value::Text(group.into())),
            ("spent".into(), number(spent)),
            ("assigned".into(), number(assigned)),
            ("overspent".into(), number(overspent)),
        ])
    };
    let source = [
        category("Income", "80", "0", "0"),
        category("Food", "0", "60", "0"),
    ];
    let historical = category("Food", "0", "0", "10");
    let old_fields = Fields::from([
        ("ready to assign".into(), number("5")),
        ("available".into(), number("30")),
    ]);
    let metadata = Fields::new();
    let row = |fields| Row {
        fields,
        metadata: &metadata,
    };
    let rows = source.each_ref().map(row);
    let whole = BTreeMap::from([("categories".into(), rows.to_vec())]);
    let old_whole = BTreeMap::from([("categories".into(), vec![row(&historical)])]);
    let previous = PeriodSnapshot {
        date: "2026-10-01",
        fields: Some(&old_fields),
        kinds: Some(&old_whole),
        ..Default::default()
    };
    let month = PeriodSnapshot {
        date: "2026-11-01",
        previous: Some(&previous),
        ..Default::default()
    };
    let periods = BTreeMap::from([(Period::Month, month)]);
    let context = EvalContext {
        today: Some("2026-11-27"),
        periods: Some(&periods),
        kinds: Some(&whole),
        ..Default::default()
    };
    let result = evaluate_query(
        &plugin.collections["categories"],
        kind,
        &rows,
        Page::default(),
        &context,
    )
    .unwrap();
    assert_eq!(result.matched, 1);
    assert_eq!(result.totals, [number("1"), number("80")]);
    assert_eq!(result.named_totals["ready to assign"], number("35"));
    assert!(result.named_total_errors.is_empty());
    for (name, expected) in [
        ("available", number("90")),
        ("daily", Value::Text("2026-11-27".into())),
        ("weekly", Value::Text("2026-W48".into())),
        ("annual", Value::Text("2026".into())),
    ] {
        assert_eq!(
            evaluate_period_field(&kind.fields[name], rows[1], &context).unwrap(),
            expected
        );
    }
    assert_eq!(
        Period::Month
            .starts("2026-08-14", "2026-11-27", 20)
            .unwrap(),
        ["2026-08-01", "2026-09-01", "2026-10-01", "2026-11-01"]
    );
    assert_eq!(
        Period::Week.starts("2026-01-04", "2026-01-06", 20).unwrap(),
        ["2025-12-29", "2026-01-05"]
    );
}

#[test]
fn whole_kind_cap_counts_matches_and_keeps_other_totals() {
    let collection = COLLECTION.replace(
        "  ready to assign: per month = first(previous(ready to assign), 0)\n      + sum(all(categories, group = \"Income\").spent)\n      - sum(all(categories, group != \"Income\").assigned)\n      + sum(previous(all(categories).overspent))",
        "  income count: = count(all(categories, group = \"Income\"))\n  expense count: = count(all(categories, group != \"Income\"))");
    let plugin = install(&collection).unwrap();
    let income = Fields::from([("group".into(), Value::Text("Income".into()))]);
    let expense = Fields::from([("group".into(), Value::Text("Expense".into()))]);
    let metadata = Fields::new();
    let income = Row {
        fields: &income,
        metadata: &metadata,
    };
    let expense = Row {
        fields: &expense,
        metadata: &metadata,
    };
    let mut candidates = vec![expense; 1_001];
    candidates.extend(std::iter::repeat_n(income, 1_000));
    let kinds = BTreeMap::from([("categories".into(), candidates)]);
    let context = EvalContext {
        kinds: Some(&kinds),
        ..Default::default()
    };
    let result = evaluate_query(
        &plugin.collections["categories"],
        &plugin.kinds["categories"],
        &[],
        Page::default(),
        &context,
    )
    .unwrap();
    assert_eq!(result.named_totals["income count"], number("1000"));
    assert_eq!(result.named_totals["expense count"], Value::Empty);
    assert!(
        result.named_total_errors["expense count"]
            .message
            .contains("Too many items")
    );
    assert!(install(&collection.replace("group != ", "missing != ")).is_err());
}
