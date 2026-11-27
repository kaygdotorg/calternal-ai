//! Kind rights and the Money approval refusal from revision 10.

use calternal_format::{
    ChangeRight, Fields, InstallOptions, PluginFile, ReadAudience, RefusalCall, Value,
    evaluate_refusals, parse_plugin,
};

const MONEY: &str =
    "---\nplugin: calternal-money\nname: Money\nversion: 1.0.0\nformat: 1\n---\nMoney example.\n";
const TRANSACTIONS: &str = "\
---
kind: transactions
name: Transaction
format: 1
folder: Money
editable: outside
rights:
  change manager
  read manager, family-adults
fields:
  amount: money EUR
operations:
  add: amount
  change
  remove
  Stop: set amount = 0 EUR
  undo: set amount = 0 EUR
  leave space: set amount = 0 EUR
---
Transactions.
";

fn filter(id: &str, action: &str, condition: &str, message: &str) -> String {
    format!(
        "---\nplugin: {id}\nname: Filter {id}\nformat: 1\nversion: 1.0.0\nrefuses:\n  [calternal-money.transactions.{action}]:\n    {condition}, {message}\n---\nPayment approval.\n"
    )
}

#[test]
fn money_refusal_is_checked_ordered_and_cannot_block_own_removal() {
    let money = parse_plugin(
        &[
            PluginFile {
                path: "plugin.md",
                text: Some(MONEY),
            },
            PluginFile {
                path: "kinds/transactions.md",
                text: Some(TRANSACTIONS),
            },
        ],
        &InstallOptions {
            official: &[("calternal-money", "Money")],
            ..Default::default()
        },
    )
    .unwrap();
    let options = InstallOptions {
        installed: std::slice::from_ref(&money),
        ..Default::default()
    };
    let kind = &money.kinds["transactions"];
    assert_eq!(kind.rights.change_right(), Some(ChangeRight::Manager));
    assert_eq!(
        kind.rights.read_audience().collect::<Vec<_>>(),
        [
            ReadAudience::Role("manager"),
            ReadAudience::Group("family-adults"),
        ]
    );
    let compile = |manifest: &str| {
        parse_plugin(
            &[PluginFile {
                path: "plugin.md",
                text: Some(manifest),
            }],
            &options,
        )
    };
    let add = compile(&filter(
        "zulu",
        "add",
        "amount > 1000 EUR",
        r#""Payments over 1,000 EUR need separate approval.""#,
    ))
    .unwrap();
    let earlier = compile(&filter(
        "alpha",
        "add",
        "amount > 1000 EUR",
        r#""Ask a manager first.""#,
    ))
    .unwrap();
    let change = compile(&filter(
        "alpha",
        "change",
        "amount > 1000 EUR",
        r#""Ask a manager first.""#,
    ))
    .unwrap();
    let remove = compile(&filter(
        "alpha",
        "remove",
        "amount > 1000 EUR",
        r#""Ask a manager first.""#,
    ))
    .unwrap();
    let enabled = [&add, &remove, &earlier, &change]
        .map(|plugin| (&plugin.manifest, &plugin.manifest.refusals[0]));
    let value = |number: &str| Fields::from([("amount".into(), Value::Text(number.into()))]);
    let small = value("100 EUR");
    let large = value("1001 EUR");
    let empty = Fields::new();
    let call = RefusalCall {
        owner: "calternal-money",
        kind,
        operation: "add",
        target: None,
        inputs: &large,
        own_removal: false,
    };
    let result = evaluate_refusals(&enabled, &call).unwrap().unwrap();
    assert_eq!(
        (result.plugin_id, result.message),
        ("alpha", "Ask a manager first.")
    );
    assert!(
        evaluate_refusals(
            &enabled,
            &RefusalCall {
                operation: "change",
                target: Some(&large),
                inputs: &small,
                ..call
            }
        )
        .unwrap()
        .is_none()
    );
    for (own_removal, refused) in [(false, true), (true, false)] {
        assert_eq!(
            evaluate_refusals(
                &enabled,
                &RefusalCall {
                    operation: "remove",
                    target: Some(&large),
                    inputs: &empty,
                    own_removal,
                    ..call
                }
            )
            .unwrap()
            .is_some(),
            refused
        );
    }
    for (action, condition, message) in [
        ("Stop", "true", "\"Try again\""),
        ("undo", "true", "\"Try again\""),
        ("leave space", "true", "\"Try again\""),
        ("add", "amount > 1000 EUR", "Not quoted"),
        ("add", "state.approved", "\"Try again\""),
        ("add", "today() != empty", "\"Try again\""),
        ("add", "undeclared > 10", "\"Try again\""),
    ] {
        assert!(
            compile(&filter("alpha", action, condition, message)).is_err(),
            "{action} {condition}"
        );
    }
}
