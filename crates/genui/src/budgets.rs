//! Deployment ceilings. Limits count only the current description and its local state/output.
use crate::{
    ast::{ExprKind, Pos, Program},
    error::Failure,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
/// Hard ceilings for one compile/initialization/evaluation; a host may lower each field.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Budgets {
    /// Maximum UTF-8 bytes of source, AST, state or evaluated output separately.
    pub serialized_bytes: usize,
    /// Maximum expressions in the AST and produced native nodes separately.
    pub nodes: usize,
    /// Maximum AST, value and evaluation dependency nesting.
    pub depth: usize,
    /// Maximum declared or supplied state entries.
    pub state_entries: usize,
    /// Maximum expression visits across initializers and roots in a pass.
    pub steps: usize,
    /// Maximum elements in any constructed or supplied list.
    pub list_length: usize,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            serialized_bytes: 262144,
            nodes: 2000,
            depth: 64,
            state_entries: 256,
            steps: 200000,
            list_length: 1000,
        }
    }
}
impl Budgets {
    /// Reject limits above deployment ceilings. O(1), no allocations on success, locks or I/O.
    /// Returns the invalid field name; zero is a valid fail-closed ceiling.
    pub fn validate(self) -> Result<Self, String> {
        let ceiling = Self::default();
        for (name, value, max) in [
            (
                "serializedBytes",
                self.serialized_bytes,
                ceiling.serialized_bytes,
            ),
            ("nodes", self.nodes, ceiling.nodes),
            ("depth", self.depth, ceiling.depth),
            ("stateEntries", self.state_entries, ceiling.state_entries),
            ("steps", self.steps, ceiling.steps),
            ("listLength", self.list_length, ceiling.list_length),
        ] {
            if value > max {
                return Err(format!("invalid budget: {name}"));
            }
        }
        Ok(self)
    }
    pub(crate) fn check_value(&self, value: &Value, pos: Pos) -> Result<(), Failure> {
        let mut stack = vec![(value, 0)];
        while let Some((v, depth)) = stack.pop() {
            if depth > self.depth {
                return Err(Failure::limit(pos, "depth"));
            }
            match v {
                Value::Array(a) => {
                    if a.len() > self.list_length {
                        return Err(Failure::limit(pos, "listLength"));
                    }
                    stack.extend(a.iter().map(|v| (v, depth + 1)));
                }
                Value::Object(o) => {
                    for (k, v) in o {
                        if forbidden(k) {
                            return Err(Failure::new(pos, format!("`{k}` is not accessible")));
                        }
                        stack.push((v, depth + 1));
                    }
                }
                _ => {}
            }
        }
        if json_bytes(value) > self.serialized_bytes {
            return Err(Failure::limit(pos, "serializedBytes"));
        }
        Ok(())
    }
    pub(crate) fn check_program(&self, p: &Program) -> Result<(), Failure> {
        let pos = Pos::start();
        if p.serialized_bytes > self.serialized_bytes {
            return Err(Failure::limit(pos, "serializedBytes"));
        }
        if p.states.len() > self.state_entries {
            return Err(Failure::limit(pos, "stateEntries"));
        }
        let mut stack: Vec<_> = p
            .states
            .iter()
            .chain(&p.named)
            .map(|d| (&d.value, 1))
            .chain(p.roots.iter().map(|e| (e, 1)))
            .collect();
        let mut count = 0;
        while let Some((e, depth)) = stack.pop() {
            count += 1;
            if count > self.nodes {
                return Err(Failure::limit(pos, "nodes"));
            }
            if depth > self.depth {
                return Err(Failure::limit(pos, "depth"));
            }
            if let ExprKind::List { items } = &e.kind {
                if items.len() > self.list_length {
                    return Err(Failure::limit(pos, "listLength"));
                }
            }
            stack.extend(e.children().into_iter().map(|c| (c, depth + 1)));
        }
        Ok(())
    }
}
pub(crate) fn forbidden(name: &str) -> bool {
    matches!(name, "__proto__" | "prototype" | "constructor")
}

// Count canonical ECMAScript JSON number spellings so exact byte boundaries match TS.
pub(crate) fn json_bytes(value: &Value) -> usize {
    let mut size = 0;
    let mut pending = vec![value];
    while let Some(v) = pending.pop() {
        match v {
            Value::Null => size += 4,
            Value::Bool(b) => size += if *b { 4 } else { 5 },
            Value::Number(n) => {
                size += crate::value::number_text(n.as_f64().expect("finite JSON number")).len()
            }
            Value::String(s) => size += serde_json::to_string(s).expect("JSON text").len(),
            Value::Array(a) => {
                size += 2 + a.len().saturating_sub(1);
                pending.extend(a);
            }
            Value::Object(o) => {
                size += 2 + o.len().saturating_sub(1);
                for (k, v) in o {
                    size += serde_json::to_string(k).expect("JSON key").len() + 1;
                    pending.push(v);
                }
            }
        }
    }
    size
}
