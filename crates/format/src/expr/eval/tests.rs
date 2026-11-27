use super::*;

#[test]
fn now_normalizes_host_clock_to_whole_seconds() {
    let expression = crate::parse_expression("now()").unwrap();
    let fields = Fields::new();
    let row = Row {
        fields: &fields,
        metadata: &fields,
    };
    for (clock, want) in [
        (
            "2026-11-27T12:30:00.123Z",
            Value::Time("2026-11-27T12:30:00Z".into()),
        ),
        (
            "2026-11-27T12:30:00.123+05:30",
            Value::Time("2026-11-27T12:30:00+05:30".into()),
        ),
        ("2026-11-27T12:30:00.Z", Value::Empty),
    ] {
        let context = EvalContext {
            now: Some(clock),
            ..EvalContext::default()
        };
        assert_eq!(evaluate(&expression, row, &context).unwrap(), want);
    }
}
#[test]
fn formula_budget_accounts_for_row_clones_and_string_searches() {
    let small = Fields::from([("title".to_owned(), Value::Text("short".into()))]);
    let large = Fields::from([("title".to_owned(), Value::Text("a".repeat(2_000_000)))]);
    let very_large = Fields::from([("title".to_owned(), Value::Text("a".repeat(3_500_000)))]);
    let metadata = Fields::new();
    let eval = |source, fields| {
        let expression = crate::parse_expression(source).unwrap();
        let context = EvalContext::default();
        evaluate(
            &expression,
            Row {
                fields,
                metadata: &metadata,
            },
            &context,
        )
    };
    assert_eq!(
        eval("contains(title, \"b\")", &small).unwrap(),
        Value::Bool(false)
    );
    assert!(eval("title", &large).is_ok());
    for result in [
        eval("this", &very_large),
        eval("contains(title, \"b\")", &large),
    ] {
        assert!(result.unwrap_err().message.contains("too costly"));
    }
}
