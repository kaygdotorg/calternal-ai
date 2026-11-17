//! Fixture-only JSON-lines adapter for direct TypeScript/Rust AST and snapshot comparisons.
use calternal_genui::{Budgets, compile, evaluate, initial_state};
use serde_json::{Value, json};
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        let input: Value =
            serde_json::from_str(&line.expect("fixture line")).expect("fixture JSON");
        let budgets: Budgets = input
            .get("budgets")
            .map(|v| serde_json::from_value(v.clone()).expect("fixture budgets"))
            .unwrap_or_default();
        let compiled = compile(input["source"].as_str().expect("fixture source"), budgets);
        let evaluation = compiled
            .program
            .as_ref()
            .filter(|_| matches!(input["kind"].as_str(), Some("evaluate" | "initialize")))
            .map(|p| {
                let budgets: Budgets = input
                    .get("evaluationBudgets")
                    .map(|v| serde_json::from_value(v.clone()).expect("fixture budgets"))
                    .unwrap_or_default();
                if input["kind"] == "initialize" {
                    let r = initial_state(p, budgets);
                    json!({"state":r.state,"diagnostics":r.diagnostics})
                } else {
                    serde_json::to_value(evaluate(
                        p,
                        input.get("state").map(|s| s.as_object().unwrap()),
                        budgets,
                    ))
                    .unwrap()
                }
            });
        println!(
            "{}",
            json!({"accepted":compiled.program.is_some(),"statements":compiled.statements,"compilationDiagnostics":compiled.diagnostics,"evaluation":evaluation})
        );
    }
}
