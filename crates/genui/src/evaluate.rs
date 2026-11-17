//! Bounded pure evaluator. Native views consume descriptions; this module never draws or executes actions.
use crate::{
    ast::{Diagnostic, Expr, ExprKind as K, NamedArg, Pos, Program},
    budgets::{Budgets, forbidden},
    catalog,
    error::{Diagnostics, Failure},
    math, validate,
    value::{Scope, Value as V},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
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
impl Interpreter<'_> {
    fn expr(&mut self, e: &Expr, scope: &Scope) -> Result<V> {
        self.steps += 1;
        if self.steps > self.limits.steps {
            return Err(Failure::limit(e.pos, "steps"));
        }
        self.depth += 1;
        if self.depth > self.limits.depth {
            self.depth -= 1;
            return Err(Failure::limit(e.pos, "depth"));
        }
        let result = self.compute(e, scope).and_then(|v| {
            self.limits.check_value(&v.json(), e.pos)?;
            Ok(v)
        });
        self.depth -= 1;
        result
    }
    fn compute(&mut self, e: &Expr, scope: &Scope) -> Result<V> {
        Ok(match &e.kind {
            K::Literal { value } => V::from_json(value),
            K::State { name } => self.state.get(name).cloned().unwrap_or(V::Null),
            K::Ident { name } => {
                if let Some(v) = scope.get(name) {
                    return Ok(v.clone());
                }
                if let Some(v) = catalog::constant(name) {
                    return Ok(V::Number(v));
                }
                if let Some(v) = self.cache.get(name) {
                    return Ok(v.clone());
                }
                let d = self
                    .program
                    .named
                    .iter()
                    .find(|d| d.name == *name)
                    .ok_or_else(|| Failure::new(e.pos, format!("unknown name: {name}")))?;
                let suffix = std::mem::take(&mut self.suffix);
                let value = self.expr(&d.value, &Scope::new());
                self.suffix = suffix;
                let value = value?;
                self.cache.insert(name.clone(), value.clone());
                value
            }
            K::List { items } => V::list(
                items
                    .iter()
                    .map(|i| self.expr(i, scope))
                    .collect::<Result<_>>()?,
            ),
            K::Object { entries } => {
                let mut out = vec![];
                for entry in entries {
                    self.forbidden(&entry.key, entry.pos)?;
                    let v = self.expr(&entry.value, scope)?;
                    if entry.key == "kind"
                        && v.str().is_some_and(|s| {
                            matches!(s, "node" | "action" | "binding" | "function")
                        })
                    {
                        return Err(Failure::new(entry.pos, "reserved interpreter value kind"));
                    }
                    out.push((entry.key.clone(), v));
                }
                V::object(out)
            }
            K::Template { quasis, parts } => {
                let mut s = quasis[0].clone();
                for (i, part) in parts.iter().enumerate() {
                    s.push_str(&self.expr(part, scope)?.text(part.pos)?);
                    s.push_str(&quasis[i + 1]);
                }
                V::String(s)
            }
            K::Unary { op, operand } => {
                let v = self.expr(operand, scope)?;
                if op == "!" {
                    V::Bool(!v.truthy())
                } else {
                    V::finite(-v.number(e.pos)?)
                }
            }
            K::Conditional { test, then, r#else } => {
                let yes = self.expr(test, scope)?.truthy();
                self.expr(if yes { then } else { r#else }, scope)?
            }
            K::Binary { op, left, right } => self.binary(op, left, right, e.pos, scope)?,
            K::Member { object, name } => {
                let v = self.expr(object, scope)?;
                self.member(v, name, e.pos)?
            }
            K::Index { object, index } => {
                let v = self.expr(object, scope)?;
                let i = self.expr(index, scope)?;
                match &v {
                    V::Null => V::Null,
                    V::List(a) => {
                        let i = i.number(e.pos)?;
                        if i.fract() == 0.0 && i >= 0.0 {
                            a.get(i as usize).cloned().unwrap_or(V::Null)
                        } else {
                            V::Null
                        }
                    }
                    V::String(s) => {
                        let i = i.number(e.pos)?;
                        if i.fract() == 0.0 && i >= 0.0 {
                            s.encode_utf16()
                                .nth(i as usize)
                                .map(|c| V::String(String::from_utf16_lossy(&[c])))
                                .unwrap_or(V::Null)
                        } else {
                            V::Null
                        }
                    }
                    V::Object(_) => {
                        if let Some(name) = i.str() {
                            self.member(v.clone(), name, e.pos)?
                        } else {
                            return Err(Failure::new(e.pos, "invalid index receiver or key"));
                        }
                    }
                    _ => return Err(Failure::new(e.pos, "invalid index receiver or key")),
                }
            }
            K::Arrow { .. } => {
                return Err(Failure::new(
                    e.pos,
                    "arrow functions are only allowed in named functions and list methods",
                ));
            }
            K::Method {
                object, name, args, ..
            } => self.method(object, name, args, e.pos, scope)?,
            K::Call { name, args, named } => {
                if let Some(spec) = catalog::component(name) {
                    return self.component(name, args, named, e.pos, spec, scope);
                }
                if let Some(spec) = catalog::action(name) {
                    return self.action(name, args, named, e.pos, spec, scope);
                }
                let own = self.program.named.iter().find(|d| d.name == *name);
                if let Some(Expr {
                    kind: K::Arrow { params, body },
                    ..
                }) = own.map(|d| &d.value)
                {
                    let vals = args
                        .iter()
                        .map(|a| self.expr(a, scope))
                        .collect::<Result<Vec<_>>>()?;
                    let inner = params
                        .iter()
                        .enumerate()
                        .map(|(i, p)| (p.clone(), vals.get(i).cloned().unwrap_or(V::Null)))
                        .collect();
                    let suffix = self.suffix.clone();
                    self.suffix
                        .push_str(&format!("@{}:{}", e.pos.line, e.pos.column));
                    let v = self.expr(body, &inner);
                    self.suffix = suffix;
                    v?
                } else {
                    let a = args
                        .iter()
                        .map(|a| self.expr(a, scope))
                        .collect::<Result<Vec<_>>>()?;
                    self.call(name, &a, e.pos)?
                }
            }
        })
    }
    fn binary(
        &mut self,
        op: &str,
        left: &Expr,
        right: &Expr,
        pos: Pos,
        scope: &Scope,
    ) -> Result<V> {
        let a = self.expr(left, scope)?;
        match op {
            "&&" => {
                return if a.truthy() {
                    self.expr(right, scope)
                } else {
                    Ok(a)
                };
            }
            "||" => {
                return if a.truthy() {
                    Ok(a)
                } else {
                    self.expr(right, scope)
                };
            }
            "??" => {
                return if matches!(a, V::Null) {
                    self.expr(right, scope)
                } else {
                    Ok(a)
                };
            }
            _ => {}
        }
        let b = self.expr(right, scope)?;
        if op == "==" || op == "===" {
            return Ok(V::Bool(a.same(&b)));
        }
        if op == "!=" || op == "!==" {
            return Ok(V::Bool(!a.same(&b)));
        }
        if matches!(op, "<" | "<=" | ">" | ">=") {
            let order = match (&a, &b) {
                (V::Number(a), V::Number(b)) => a.partial_cmp(b),
                (V::String(a), V::String(b)) => Some(a.encode_utf16().cmp(b.encode_utf16())),
                _ => {
                    return Err(Failure::new(
                        pos,
                        "comparison needs two numbers or two strings",
                    ));
                }
            };
            return Ok(V::Bool(match op {
                "<" => order == Some(std::cmp::Ordering::Less),
                "<=" => order.is_some_and(|o| o != std::cmp::Ordering::Greater),
                ">" => order == Some(std::cmp::Ordering::Greater),
                _ => order.is_some_and(|o| o != std::cmp::Ordering::Less),
            }));
        }
        if op == "+" && (matches!(a, V::String(_)) || matches!(b, V::String(_))) {
            return Ok(V::String(a.text(pos)? + &b.text(pos)?));
        }
        let (x, y) = (a.number(pos)?, b.number(pos)?);
        Ok(V::finite(match op {
            "+" => x + y,
            "-" => x - y,
            "*" => x * y,
            "/" => x / y,
            "%" => x % y,
            _ => x.powf(y),
        }))
    }
    fn forbidden(&self, name: &str, pos: Pos) -> Result<()> {
        if forbidden(name) {
            Err(Failure::new(pos, format!("`{name}` is not accessible")))
        } else {
            Ok(())
        }
    }
    fn member(&self, v: V, name: &str, pos: Pos) -> Result<V> {
        self.forbidden(name, pos)?;
        match &v {
            V::Null => Ok(V::Null),
            V::Object(_) => Ok(v.get(name).cloned().unwrap_or(V::Null)),
            V::List(a) if name == "length" => Ok(V::Number(a.len() as f64)),
            V::String(s) if name == "length" => Ok(V::Number(s.encode_utf16().count() as f64)),
            _ => Err(Failure::new(pos, format!("cannot read property: {name}"))),
        }
    }
    fn node(&mut self, name: &str, key: &str, props: V, pos: Pos) -> Result<V> {
        self.nodes += 1;
        if self.nodes > self.limits.nodes {
            return Err(Failure::limit(pos, "nodes"));
        }
        Ok(V::object(vec![
            ("kind".into(), V::String("node".into())),
            ("type".into(), V::String(name.into())),
            ("key".into(), V::String(key.into())),
            ("props".into(), props),
        ]))
    }
    fn children(&mut self, v: V, pos: Pos, key: &str) -> Result<Vec<V>> {
        let mut out = vec![];
        let mut pending = vec![v];
        while let Some(v) = pending.pop() {
            match &v {
                V::Null | V::Bool(false) => {}
                V::String(_) | V::Number(_) => {
                    let text = v.text(pos)?;
                    out.push(self.node(
                        "Text",
                        &format!("{key}.{}", out.len()),
                        V::object(vec![("text".into(), V::String(text))]),
                        pos,
                    )?);
                }
                V::List(a) => pending.extend(a.iter().rev().cloned()),
                V::Object(_) if v.get("kind").and_then(V::str) == Some("node") => out.push(v),
                _ => return Err(Failure::new(pos, "children must be components or text")),
            }
        }
        Ok(out)
    }
    fn actions(&self, v: V, pos: Pos) -> Result<V> {
        let mut out = vec![];
        let mut pending = vec![v];
        while let Some(v) = pending.pop() {
            match &v {
                V::Null | V::Bool(false) => {}
                V::List(a) => pending.extend(a.iter().rev().cloned()),
                V::Object(_) if v.get("kind").and_then(V::str) == Some("action") => out.push(v),
                _ => return Err(Failure::new(pos, "expected action descriptions")),
            }
        }
        Ok(V::list(out))
    }
    fn param(
        &mut self,
        owner: &str,
        p: &Value,
        e: &Expr,
        scope: &Scope,
        key: &str,
    ) -> Result<Option<V>> {
        let field = p["name"].as_str().unwrap();
        let kind = p["kind"].as_str().unwrap();
        if kind == "binding" {
            let K::State { name } = &e.kind else {
                return Err(Failure::new(
                    e.pos,
                    format!("{owner}.{field} must be a $state reference"),
                ));
            };
            let mut v = self.state.get(name).cloned().unwrap_or(V::Null);
            let holds = p["holds"].as_str();
            let fits = matches!(v, V::Null)
                || holds.is_none()
                || match holds.unwrap_or("") {
                    "scalar" => matches!(v, V::String(_) | V::Number(_) | V::Bool(_)),
                    "list" => matches!(v, V::List(_)),
                    "text" => matches!(v, V::String(_)),
                    "number" => matches!(v, V::Number(_)),
                    "boolean" => matches!(v, V::Bool(_)),
                    _ => false,
                };
            if !fits {
                v = match (holds, &v) {
                    (Some("list"), V::String(s)) if s.is_empty() => V::list(vec![]),
                    (Some("list"), V::String(_) | V::Number(_) | V::Bool(_)) => {
                        V::list(vec![v.clone()])
                    }
                    (Some("number"), V::String(s)) if decimal(s) => {
                        V::finite(s.trim().parse().unwrap_or(f64::NAN))
                    }
                    (Some("text"), V::Number(_) | V::Bool(_)) => V::String(v.text(e.pos)?),
                    _ => {
                        return Err(Failure::new(
                            e.pos,
                            format!("{owner}.{field} state needs {}", holds.unwrap_or("")),
                        ));
                    }
                };
            }
            return Ok(Some(V::object(vec![
                ("kind".into(), V::String("binding".into())),
                ("name".into(), V::String(name.clone())),
                ("value".into(), v),
            ])));
        }
        let v = self.expr(e, scope)?;
        if kind == "nodes" {
            return Ok(Some(V::list(self.children(v, e.pos, key)?)));
        }
        if kind == "actions" {
            return Ok(Some(self.actions(v, e.pos)?));
        }
        if matches!(v, V::Null) {
            if p["optional"] == true {
                return Ok(None);
            }
            if p["nullable"] == true || kind == "any" {
                return Ok(Some(v));
            }
        }
        if matches!(kind, "string" | "text") && !matches!(v, V::Null | V::List(_) | V::Object(_)) {
            return Ok(Some(V::String(v.text(e.pos)?)));
        }
        let fits = match kind {
            "number" => matches!(v, V::Number(_)),
            "boolean" => matches!(v, V::Bool(_)),
            "list" => matches!(v, V::List(_)),
            "any" => true,
            "enum" => v.str().is_some_and(|s| {
                p["values"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(s)))
            }),
            "series" => series(&v),
            _ => false,
        };
        if fits {
            Ok(Some(v))
        } else {
            Err(Failure::new(e.pos, format!("{owner}.{field} needs {kind}")))
        }
    }
    fn component(
        &mut self,
        name: &str,
        args: &[Expr],
        named: &[NamedArg],
        pos: Pos,
        spec: &Value,
        scope: &Scope,
    ) -> Result<V> {
        let key = format!("{}:{}{}", pos.line, pos.column, self.suffix);
        let result = (|| {
            let slots = validate::slots(args, named, spec);
            let mut props = vec![];
            for p in catalog::params(spec) {
                let field = p["name"].as_str().unwrap();
                if let Some(arg) = slots.get(field) {
                    if let Some(v) = self.param(name, p, arg, scope, &key)? {
                        props.push((field.to_string(), v));
                    }
                } else if p["optional"] != true {
                    return Err(Failure::new(
                        pos,
                        format!("missing argument: {name}.{field}"),
                    ));
                }
            }
            let props = V::object(props);
            for field in catalog::required(spec) {
                if props
                    .get(field)
                    .and_then(V::str)
                    .is_none_or(|s| s.trim().is_empty())
                {
                    return Err(Failure::new(
                        pos,
                        format!("accessibility: {name}.{field} must be nonblank text"),
                    ));
                }
            }
            self.node(name, &key, props, pos)
        })();
        match result {
            Err(e) if !e.limit => {
                self.diagnostics.report(e.pos, e.message.clone());
                self.node(
                    "Error",
                    &key,
                    V::object(vec![("message".into(), V::String(e.message))]),
                    pos,
                )
            }
            other => other,
        }
    }
    fn action(
        &mut self,
        name: &str,
        args: &[Expr],
        named: &[NamedArg],
        pos: Pos,
        spec: &Value,
        scope: &Scope,
    ) -> Result<V> {
        let slots = validate::slots(args, named, spec);
        if name == "Set" {
            let Some(Expr {
                kind: K::State { name: target },
                ..
            }) = slots.get("state").copied()
            else {
                return Err(Failure::new(pos, "Set.state must be a $state reference"));
            };
            let v = self.expr(slots["value"], scope)?;
            return Ok(V::object(vec![
                ("kind".into(), V::String("action".into())),
                ("type".into(), V::String("set".into())),
                ("name".into(), V::String(target.clone())),
                ("value".into(), v),
            ]));
        }
        let v = self
            .expr(slots[if name == "OpenUrl" { "url" } else { "text" }], scope)?
            .text(pos)?;
        if name == "OpenUrl"
            && !v.to_ascii_lowercase().starts_with("https://")
            && !v.to_ascii_lowercase().starts_with("http://")
        {
            return Err(Failure::new(
                pos,
                "OpenUrl needs an http:// or https:// URL",
            ));
        }
        Ok(V::object(vec![
            ("kind".into(), V::String("action".into())),
            (
                "type".into(),
                V::String(
                    match name {
                        "OpenUrl" => "openUrl",
                        "Send" => "send",
                        _ => "copy",
                    }
                    .into(),
                ),
            ),
            (
                if name == "OpenUrl" { "url" } else { "text" }.into(),
                V::String(v),
            ),
        ]))
    }
    fn call(&self, name: &str, a: &[V], pos: Pos) -> Result<V> {
        let n = |i: usize| a[i].number(pos);
        let numbers = |v: &V| -> Result<Vec<f64>> {
            let V::List(a) = v else {
                return Err(Failure::new(pos, "expected a list of numbers"));
            };
            a.iter()
                .filter(|v| !matches!(v, V::Null))
                .map(|v| v.number(pos))
                .collect()
        };
        Ok(match name {
            "round" => V::finite(math::round(n(0)?, math::digits(a.get(1), pos)?)),
            "fmt" => V::String(math::fmt(
                n(0)?,
                a.get(1).map(|v| math::digits(Some(v), pos)).transpose()?,
            )),
            "abs" => V::Number(n(0)?.abs()),
            "floor" => V::finite(n(0)?.floor()),
            "ceil" => V::finite(n(0)?.ceil()),
            "sqrt" => V::finite(n(0)?.sqrt()),
            "pow" => V::finite(n(0)?.powf(n(1)?)),
            "sin" => V::finite(n(0)?.sin()),
            "cos" => V::finite(n(0)?.cos()),
            "tan" => V::finite(n(0)?.tan()),
            "asin" => V::finite(n(0)?.asin()),
            "acos" => V::finite(n(0)?.acos()),
            "atan" => V::finite(n(0)?.atan()),
            "atan2" => V::finite(n(0)?.atan2(n(1)?)),
            "clamp" => V::finite(n(2)?.min(n(1)?.max(n(0)?))),
            "lerp" => V::finite(n(0)? + (n(1)? - n(0)?) * n(2)?),
            "random" => V::Number(math::random(n(0)?)),
            "sum" => V::finite(numbers(&a[0])?.iter().sum()),
            "min" | "max" => {
                let v = if a.len() == 1 && matches!(a[0], V::List(_)) {
                    numbers(&a[0])?
                } else {
                    a.iter()
                        .map(|v| v.number(pos))
                        .collect::<Result<Vec<_>>>()?
                };
                if v.is_empty() {
                    V::Null
                } else {
                    V::finite(
                        v.into_iter()
                            .reduce(|a, b| if name == "min" { a.min(b) } else { a.max(b) })
                            .unwrap(),
                    )
                }
            }
            "range" => {
                let (start, end, step) = (n(0)?, n(1)?, if a.len() > 2 { n(2)? } else { 1.0 });
                if step == 0.0 {
                    return Err(Failure::new(pos, "range step cannot be zero"));
                }
                let count = ((end - start) / step).ceil().max(0.0);
                if count > self.limits.list_length as f64 {
                    return Err(Failure::limit(pos, "listLength"));
                }
                V::list(
                    (0..count as usize)
                        .map(|i| V::finite(start + i as f64 * step))
                        .collect(),
                )
            }
            "keys" | "values" | "entries" => {
                let V::Object(o) = &a[0] else {
                    return Err(Failure::new(pos, "expected an object"));
                };
                V::list(
                    o.iter()
                        .map(|(k, v)| match name {
                            "keys" => V::String(k.clone()),
                            "values" => v.clone(),
                            _ => V::list(vec![V::String(k.clone()), v.clone()]),
                        })
                        .collect(),
                )
            }
            _ => return Err(Failure::new(pos, format!("unknown call: {name}"))),
        })
    }
    fn method(
        &mut self,
        object: &Expr,
        name: &str,
        args: &[Expr],
        pos: Pos,
        scope: &Scope,
    ) -> Result<V> {
        let v = self.expr(object, scope)?;
        if let V::List(list) = &v {
            if catalog::lambda_method(name) {
                let Some(Expr {
                    kind: K::Arrow { params, body },
                    ..
                }) = args.first()
                else {
                    return Err(Failure::new(
                        pos,
                        format!(".{name} requires an arrow function"),
                    ));
                };
                let base = self.suffix.clone();
                let result = (|| {
                    let mut out = vec![];
                    for (i, v) in list.iter().enumerate() {
                        let mut inner = scope.clone();
                        for (k, p) in params.iter().enumerate() {
                            inner.insert(
                                p.clone(),
                                if k == 0 {
                                    v.clone()
                                } else {
                                    V::Number(i as f64)
                                },
                            );
                        }
                        self.suffix = if name == "map" {
                            format!("{base}#{i}")
                        } else {
                            base.clone()
                        };
                        let item = self.expr(body, &inner)?;
                        let yes = item.truthy();
                        match name {
                            "map" => out.push(item),
                            "filter" if yes => out.push(v.clone()),
                            "find" if yes => return Ok(v.clone()),
                            "some" if yes => return Ok(V::Bool(true)),
                            "every" if !yes => return Ok(V::Bool(false)),
                            _ => {}
                        }
                    }
                    Ok(match name {
                        "find" => V::Null,
                        "some" => V::Bool(false),
                        "every" => V::Bool(true),
                        _ => V::list(out),
                    })
                })();
                self.suffix = base;
                return result;
            }
        }
        let a = args
            .iter()
            .map(|e| self.expr(e, scope))
            .collect::<Result<Vec<_>>>()?;
        let string = |i: usize| {
            a.get(i)
                .and_then(V::str)
                .ok_or_else(|| Failure::new(pos, "expected text"))
        };
        match &v {
            V::List(list) => match name {
                "join" => {
                    let separator = if a.is_empty() { "," } else { string(0)? };
                    return Ok(V::String(
                        list.iter()
                            .map(|v| v.text(pos))
                            .collect::<Result<Vec<_>>>()?
                            .join(separator),
                    ));
                }
                "slice" => {
                    let (start, end) = bounds(&a, list.len(), pos)?;
                    return Ok(V::list(list[start..end].to_vec()));
                }
                "includes" => return Ok(V::Bool(list.iter().any(|v| v.same(&a[0])))),
                "indexOf" => {
                    return Ok(V::Number(
                        list.iter()
                            .position(|v| v.same(&a[0]))
                            .map(|i| i as f64)
                            .unwrap_or(-1.0),
                    ));
                }
                _ => {}
            },
            V::String(s) => match name {
                "includes" => return Ok(V::Bool(s.contains(string(0)?))),
                "startsWith" => return Ok(V::Bool(s.starts_with(string(0)?))),
                "endsWith" => return Ok(V::Bool(s.ends_with(string(0)?))),
                "trim" => return Ok(V::String(s.trim_matches(js_whitespace).to_string())),
                "toUpperCase" => return Ok(V::String(s.to_uppercase())),
                "toLowerCase" => return Ok(V::String(s.to_lowercase())),
                "slice" => {
                    let u: Vec<_> = s.encode_utf16().collect();
                    let (start, end) = bounds(&a, u.len(), pos)?;
                    return Ok(V::String(String::from_utf16_lossy(&u[start..end])));
                }
                "split" => {
                    let delimiter = string(0)?;
                    return Ok(V::list(if delimiter.is_empty() {
                        s.encode_utf16()
                            .map(|u| V::String(String::from_utf16_lossy(&[u])))
                            .collect()
                    } else {
                        s.split(delimiter).map(|s| V::String(s.into())).collect()
                    }));
                }
                _ => {}
            },
            V::Number(n) if name == "toFixed" => {
                return Ok(V::String(math::fixed(*n, math::digits(a.first(), pos)?)));
            }
            _ => {}
        }
        Err(Failure::new(pos, format!("method unavailable: {name}")))
    }
}
fn bounds(args: &[V], len: usize, pos: Pos) -> Result<(usize, usize)> {
    let at = |v: &V| -> Result<usize> {
        let n = v.number(pos)?.trunc();
        Ok(if n < 0.0 {
            (len as f64 + n).max(0.0) as usize
        } else {
            n.min(len as f64) as usize
        })
    };
    let start = args.first().map(at).transpose()?.unwrap_or(0);
    let end = args.get(1).map(at).transpose()?.unwrap_or(len);
    Ok((start, end.max(start)))
}
fn decimal(s: &str) -> bool {
    let s = s.trim();
    let s = s.strip_prefix('-').unwrap_or(s);
    let mut parts = s.split('.');
    let first = parts.next().unwrap_or("");
    !first.is_empty()
        && first.bytes().all(|c| c.is_ascii_digit())
        && parts
            .next()
            .is_none_or(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_digit()))
        && parts.next().is_none()
}
fn series(v: &V) -> bool {
    fn nums(v: &V) -> bool {
        matches!(v,V::List(a)if a.iter().all(|v|matches!(v,V::Null|V::Number(_))))
    }
    nums(v)
        || matches!(v,V::Object(o)if o.iter().all(|(_,v)|nums(v)||v.get("values").is_some_and(nums)))
}
fn js_whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
