//! Both implementations read every JSON case; expected JSON is independent of the test runners.
use calternal_genui::{Budgets, compile, evaluate, initial_state};
use serde_json::{Value, json};
use std::path::Path;
#[test]
fn shared_conformance_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contract/genui-fixtures");
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    let mut count = 0;
    let mut failures = vec![];
    for path in files {
        let corpus: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(corpus["version"], 1);
        for fixture in corpus["cases"].as_array().unwrap() {
            count += 1;
            let limits: Budgets = fixture
                .get("budgets")
                .map(|v| serde_json::from_value(v.clone()).unwrap())
                .unwrap_or_default();
            let compilation = compile(fixture["source"].as_str().unwrap(), limits);
            let actual = match fixture["kind"].as_str().unwrap() {
                "parse" => {
                    json!({"statements":compilation.statements,"diagnostics":compilation.diagnostics})
                }
                "validate" => {
                    json!({"accepted":compilation.program.is_some(),"diagnostics":compilation.diagnostics})
                }
                kind => {
                    if let Some(program) = compilation.program {
                        let limits: Budgets = fixture
                            .get("evaluationBudgets")
                            .map(|v| serde_json::from_value(v.clone()).unwrap())
                            .unwrap_or_default();
                        let result = if kind == "initialize" {
                            initial_state(&program, limits)
                        } else {
                            evaluate(
                                &program,
                                fixture.get("state").map(|s| s.as_object().unwrap()),
                                limits,
                            )
                        };
                        if kind == "initialize" {
                            json!({"state":result.state,"diagnostics":result.diagnostics})
                        } else {
                            serde_json::to_value(result).unwrap()
                        }
                    } else {
                        json!({"state":{},"roots":[],"diagnostics":compilation.diagnostics})
                    }
                }
            };
            if !json_equal(&actual, &fixture["expected"]) {
                failures.push(format!(
                    "{}: {} actual={actual} expected={}",
                    path.display(),
                    fixture["name"],
                    fixture["expected"]
                ));
            }
        }
    }
    assert!(count > 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    eprintln!("{count} shared GenUI fixtures passed");
}
fn json_equal(actual: &Value, expected: &Value) -> bool {
    if actual.is_number() && expected.is_number() {
        return actual.as_f64() == expected.as_f64();
    }
    match (actual, expected) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| json_equal(a, b))
        }
        (Value::Object(a), Value::Object(b)) => {
            a.len() == b.len()
                && a.iter()
                    .all(|(k, v)| b.get(k).is_some_and(|e| json_equal(v, e)))
        }
        _ => actual == expected,
    }
}
