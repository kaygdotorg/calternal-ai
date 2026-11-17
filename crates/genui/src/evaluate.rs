//! Bounded pure evaluator. Native views consume descriptions; this module never draws or executes actions.
use crate::{
    ast::{Diagnostic, Pos, Program},
    budgets::Budgets,
    error::{Diagnostics, Failure},
    value::{Scope, Value as V},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
// Child modules extend one interpreter, so all evaluation shares the same counters/cache.
mod calls;
mod components;
mod expressions;

type Result<T> = std::result::Result<T, Failure>;
/// One complete native description snapshot, with no executable callbacks or effects.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Evaluation {
    /// Initialized or supplied state; empty if evaluation terminates with an error.
    pub state: Map<String, Value>,
    /// Native root descriptions; resource failures discard all partial roots.
    pub roots: Vec<Value>,
    /// Deterministic errors in evaluation order, capped at twenty.
    pub diagnostics: Vec<Diagnostic>,
}
/// Initialize and evaluate a compiled description with optional external local state.
/// Rechecks all budgets on every call. Cost is O(steps × bounded value bytes + local name lookups),
/// with bounded state/cache/tree allocations and no locks, I/O, fsyncs, round trips or action effects.
/// Failed initializers/resource limits discard all partial output; component errors become Error nodes.
pub fn evaluate(
    program: &Program,
    state: Option<&Map<String, Value>>,
    limits: Budgets,
) -> Evaluation {
    evaluate_inner(program, state, limits, false)
}
/// Initialize local state without evaluating native roots. Uses the same bounded interpreter.
/// O(initializer steps × bounded value bytes), local state/cache allocations only; no locks or I/O.
/// Returns positioned diagnostics and empty state if initialization fails.
pub fn initial_state(program: &Program, limits: Budgets) -> Evaluation {
    evaluate_inner(program, None, limits, true)
}
fn evaluate_inner(
    program: &Program,
    state: Option<&Map<String, Value>>,
    limits: Budgets,
    initial_only: bool,
) -> Evaluation {
    let mut interpreter = Interpreter {
        program,
        state: HashMap::new(),
        limits,
        steps: 0,
        nodes: 0,
        depth: 0,
        suffix: String::new(),
        cache: HashMap::new(),
        diagnostics: Diagnostics::default(),
    };
    let result = (|| {
        limits
            .validate()
            .map_err(|s| Failure::new(Pos::start(), s))?;
        limits.check_program(program)?;
        if let Some(state) = state {
            if state.len() > limits.state_entries {
                return Err(Failure::limit(Pos::start(), "stateEntries"));
            }
            limits.check_value(&Value::Object(state.clone()), Pos::start())?;
            let mut pending: Vec<_> = state.values().collect();
            while let Some(v) = pending.pop() {
                match v {
                    Value::Object(o) => {
                        if o.get("kind").and_then(Value::as_str).is_some_and(|s| {
                            matches!(s, "node" | "action" | "binding" | "function")
                        }) {
                            return Err(Failure::new(
                                Pos::start(),
                                "reserved interpreter value kind",
                            ));
                        }
                        pending.extend(o.values());
                    }
                    Value::Array(a) => pending.extend(a),
                    _ => {}
                }
            }
            for (k, v) in state {
                interpreter.state.insert(k.clone(), V::from_json(v));
            }
        } else {
            for decl in &program.states {
                let v = interpreter.expr(&decl.value, &Scope::new())?;
                interpreter.state.insert(decl.name.clone(), v);
            }
        }
        let mut roots = vec![];
        if !initial_only {
            for e in &program.roots {
                let v = interpreter.expr(e, &Scope::new())?;
                roots.extend(interpreter.children(
                    v,
                    e.pos,
                    &format!("{}:{}", e.pos.line, e.pos.column),
                )?);
            }
        }
        let state: Map<_, _> = if let Some(external) = state {
            external
                .keys()
                .map(|k| (k.clone(), interpreter.state[k].json()))
                .collect()
        } else {
            program
                .states
                .iter()
                .map(|d| (d.name.clone(), interpreter.state[&d.name].json()))
                .collect()
        };
        let roots: Vec<_> = roots.iter().map(V::json).collect();
        let mut pending: Vec<_> = roots.iter().collect();
        let mut node_count = 0;
        while let Some(v) = pending.pop() {
            match v {
                Value::Object(o) => {
                    if o.get("kind").and_then(Value::as_str) == Some("node") {
                        node_count += 1;
                        if node_count > limits.nodes {
                            return Err(Failure::limit(Pos::start(), "nodes"));
                        }
                    }
                    pending.extend(o.values());
                }
                Value::Array(a) => pending.extend(a),
                _ => {}
            }
        }
        limits.check_value(&json!({"state":state,"roots":roots}), Pos::start())?;
        Ok((state, roots))
    })();
    match result {
        Ok((state, roots)) => Evaluation {
            state,
            roots,
            diagnostics: interpreter.diagnostics.items,
        },
        Err(e) => {
            interpreter.diagnostics.report(e.pos, e.message);
            Evaluation {
                state: Map::new(),
                roots: vec![],
                diagnostics: interpreter.diagnostics.items,
            }
        }
    }
}
struct Interpreter<'a> {
    program: &'a Program,
    state: HashMap<String, V>,
    limits: Budgets,
    steps: usize,
    nodes: usize,
    depth: usize,
    suffix: String,
    cache: HashMap<String, V>,
    diagnostics: Diagnostics,
}
