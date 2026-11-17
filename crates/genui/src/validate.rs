//! Static catalog, accessibility, dependency and initialization validation.
use crate::{
    ast::{Decl, Expr, ExprKind as K, NamedArg, Pos, Program, Statement},
    budgets::{Budgets, forbidden},
    catalog,
    error::Diagnostics,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
pub(crate) fn slots<'a>(
    args: &'a [Expr],
    named: &'a [NamedArg],
    spec: &Value,
) -> HashMap<String, &'a Expr> {
    let mut out = HashMap::new();
    for (arg, param) in args.iter().zip(catalog::params(spec)) {
        out.insert(param["name"].as_str().unwrap().to_string(), arg);
    }
    for arg in named {
        out.entry(arg.name.clone()).or_insert(&arg.value);
    }
    out
}
pub(crate) fn validate(
    statements: &[Statement],
    diagnostics: &mut Diagnostics,
    limits: Budgets,
) -> Program {
    let mut p = Program {
        serialized_bytes: crate::budgets::json_bytes(
            &serde_json::to_value(statements).expect("AST is JSON"),
        ),
        states: vec![],
        named: vec![],
        roots: vec![],
    };
    for statement in statements {
        let (d, is_state) = match statement {
            Statement::Root { value } => {
                p.roots.push(value.clone());
                continue;
            }
            Statement::State { decl } => (decl, true),
            Statement::Named { decl } => (decl, false),
        };
        check_forbidden(&d.name, d.pos, diagnostics);
        if catalog::constant(&d.name).is_some() {
            diagnostics.report(d.pos, format!("reserved constant: {}", d.name));
        }
        let table = if is_state {
            &mut p.states
        } else {
            &mut p.named
        };
        if table.iter().any(|prev| prev.name == d.name) {
            diagnostics.report(
                d.pos,
                format!(
                    "duplicate declaration: {}{}",
                    if is_state { "$" } else { "" },
                    d.name
                ),
            );
        } else {
            table.push(d.clone());
        }
    }
    if p.roots.is_empty() {
        if let Some(d) = p.named.iter().find(|d| d.name == "root") {
            p.roots.push(Expr {
                pos: d.pos,
                kind: K::Ident {
                    name: "root".into(),
                },
            });
        }
    }
    if p.roots.is_empty() {
        diagnostics.report(
            Pos::start(),
            "Nothing to render: add a bare component expression such as Card(...)",
        );
    }
    if let Err(e) = limits.check_program(&p) {
        diagnostics.report(e.pos, e.message);
        return p;
    }
    let mut validator = Validator { p: &p, diagnostics };
    let mut refs: HashMap<String, Vec<String>> = HashMap::new();
    let mut state_refs = HashMap::new();
    for d in &p.named {
        let mut n = vec![];
        let mut s = vec![];
        validator.walk(&d.value, &[], &mut n, &mut s, true);
        refs.insert(d.name.clone(), n);
        state_refs.insert(d.name.clone(), s);
    }
    for root in &p.roots {
        validator.walk(root, &[], &mut vec![], &mut vec![], false);
    }
    let mut done = HashSet::new();
    let mut visiting = HashSet::new();
    for d in &p.named {
        let mut stack = vec![(d.name.clone(), false)];
        while let Some((name, exit)) = stack.pop() {
            if exit {
                visiting.remove(&name);
                done.insert(name);
                continue;
            }
            if done.contains(&name) {
                continue;
            }
            if visiting.contains(&name) {
                let d = p.named.iter().find(|d| d.name == name).unwrap();
                validator
                    .diagnostics
                    .report(d.pos, format!("cyclic named value: {name}"));
                continue;
            }
            visiting.insert(name.clone());
            stack.push((name.clone(), true));
            if let Some(names) = refs.get(&name) {
                for r in names.iter().rev() {
                    stack.push((r.clone(), false));
                }
            }
        }
    }
    for (i, d) in p.states.iter().enumerate() {
        let mut names = vec![];
        let mut states = vec![];
        validator.walk(&d.value, &[], &mut names, &mut states, false);
        let mut seen = HashSet::new();
        let mut stack: Vec<_> = names.into_iter().rev().collect();
        while let Some(name) = stack.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(s) = state_refs.get(&name) {
                for s in s {
                    push_unique(&mut states, s);
                }
            }
            if let Some(n) = refs.get(&name) {
                stack.extend(n.iter().rev().cloned());
            }
        }
        for s in states {
            if p.states
                .iter()
                .position(|d| d.name == s)
                .is_some_and(|at| at >= i)
            {
                validator.diagnostics.report(
                    d.pos,
                    format!("state initializer reads uninitialized state: ${s}"),
                );
            }
        }
    }
    p
}
fn push_unique(out: &mut Vec<String>, name: &str) {
    if !out.iter().any(|s| s == name) {
        out.push(name.into());
    }
}
fn check_forbidden(name: &str, pos: Pos, diagnostics: &mut Diagnostics) {
    if forbidden(name) {
        diagnostics.report(pos, format!("`{name}` is not accessible"));
    }
}
struct Validator<'a> {
    p: &'a Program,
    diagnostics: &'a mut Diagnostics,
}
impl Validator<'_> {
    fn named(&self, name: &str) -> Option<&Decl> {
        self.p.named.iter().find(|d| d.name == name)
    }
    fn walk(
        &mut self,
        e: &Expr,
        params: &[String],
        names: &mut Vec<String>,
        reads: &mut Vec<String>,
        allow_arrow: bool,
    ) {
        match &e.kind {
            K::Ident { name } if !params.contains(name) && catalog::constant(name).is_none() => {
                if self.named(name).is_none() {
                    self.diagnostics
                        .report(e.pos, format!("unknown name: {name}"));
                } else {
                    push_unique(names, name);
                }
            }
            K::State { name } => {
                push_unique(reads, name);
                if !self.p.states.iter().any(|d| d.name == *name) {
                    self.diagnostics
                        .report(e.pos, format!("unknown state: ${name}"));
                }
            }
            K::Member { name, .. } => check_forbidden(name, e.pos, self.diagnostics),
            K::Index { index, .. } => {
                if let K::Literal {
                    value: Value::String(name),
                } = &index.kind
                {
                    check_forbidden(name, index.pos, self.diagnostics);
                }
            }
            K::Object { entries } => {
                for entry in entries {
                    check_forbidden(&entry.key, entry.pos, self.diagnostics);
                }
            }
            _ => {}
        }
        if let K::Arrow { params: own, body } = &e.kind {
            if !allow_arrow {
                self.diagnostics.report(
                    e.pos,
                    "arrow functions are only allowed in named functions and list methods",
                );
            }
            let mut seen = HashSet::new();
            for name in own {
                check_forbidden(name, e.pos, self.diagnostics);
                if !seen.insert(name) {
                    self.diagnostics
                        .report(e.pos, format!("duplicate parameter: {name}"));
                }
            }
            let mut scope = params.to_vec();
            scope.extend(own.iter().cloned());
            self.walk(body, &scope, names, reads, false);
            return;
        }
        if let K::Call { name, args, named } = &e.kind {
            if let Some(spec) = catalog::component(name).or_else(|| catalog::action(name)) {
                let mapped = slots(args, named, spec);
                let ps = catalog::params(spec);
                let mut given: HashSet<_> = ps
                    .iter()
                    .take(args.len())
                    .map(|p| p["name"].as_str().unwrap())
                    .collect();
                if args.len() > ps.len() {
                    self.diagnostics
                        .report(e.pos, format!("too many arguments: {name}"));
                }
                for arg in named {
                    if !ps.iter().any(|p| p["name"] == arg.name) {
                        self.diagnostics
                            .report(arg.pos, format!("unknown argument: {name}.{}", arg.name));
                    }
                    if !given.insert(&arg.name) {
                        self.diagnostics
                            .report(arg.pos, format!("duplicate argument: {}", arg.name));
                    }
                }
                let accessible = catalog::required(spec);
                for p in ps {
                    let field = p["name"].as_str().unwrap();
                    let access = accessible.contains(&field);
                    let Some(v) = mapped.get(field) else {
                        if p["optional"] != true {
                            self.diagnostics.report(
                                e.pos,
                                if access {
                                    format!("accessibility: {name}.{field} is required")
                                } else {
                                    format!("missing argument: {name}.{field}")
                                },
                            );
                        }
                        continue;
                    };
                    if access {
                        if let K::Literal { value } = &v.kind {
                            if value.as_str().is_none_or(|s| s.trim().is_empty()) {
                                self.diagnostics.report(
                                    v.pos,
                                    format!("accessibility: {name}.{field} must be nonblank text"),
                                );
                            }
                        }
                    }
                    let kind = p["kind"].as_str().unwrap();
                    if kind == "binding" && !matches!(v.kind, K::State { .. }) {
                        self.diagnostics
                            .report(v.pos, format!("{name}.{field} must be a $state reference"));
                    }
                    if let K::Literal { value } = &v.kind {
                        if !(value.is_null() && (p["optional"] == true || p["nullable"] == true)) {
                            if kind == "number" && !value.is_number()
                                || kind == "boolean" && !value.is_boolean()
                                || kind == "list"
                            {
                                self.diagnostics
                                    .report(v.pos, format!("{name}.{field} needs {kind}"));
                            }
                            if kind == "enum"
                                && !p["values"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .any(|v| v.as_str() == Some(&scalar_text(value)))
                            {
                                let options = p["values"]
                                    .as_array()
                                    .unwrap()
                                    .iter()
                                    .filter_map(Value::as_str)
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                self.diagnostics.report(
                                    v.pos,
                                    format!("{name}.{field} must be one of {options}"),
                                );
                            }
                        }
                    }
                }
            } else {
                let own = self.named(name).map(|d| d.value.clone());
                let own_params = match &own {
                    Some(Expr {
                        kind: K::Arrow { params, .. },
                        ..
                    }) => Some(params.len()),
                    _ => None,
                };
                let arity = catalog::function_arity(name)
                    .or_else(|| own_params.map(|len| (0, len as isize)));
                if let Some((min, max)) = arity {
                    if own_params.is_some() {
                        push_unique(names, name);
                    }
                    if let Some(first) = named.first() {
                        self.diagnostics
                            .report(first.pos, format!("{name} takes positional arguments only"));
                    }
                    if args.len() < min || max >= 0 && args.len() > max as usize {
                        self.diagnostics
                            .report(e.pos, format!("wrong argument count: {name}"));
                    }
                } else {
                    self.diagnostics
                        .report(e.pos, format!("unknown call: {name}"));
                }
            }
        }
        if let K::Method {
            object,
            name,
            args,
            named,
        } = &e.kind
        {
            if let Some((min, max)) = catalog::method_arity(name) {
                if args.len() < min || args.len() > max as usize {
                    self.diagnostics
                        .report(e.pos, format!("wrong argument count: .{name}"));
                }
            } else {
                self.diagnostics
                    .report(e.pos, format!("unknown method: {name}"));
            }
            if let Some(first) = named.first() {
                self.diagnostics
                    .report(first.pos, "methods take positional arguments only");
            }
            self.walk(object, params, names, reads, false);
            for (i, arg) in args.iter().enumerate() {
                let lambda = i == 0 && catalog::lambda_method(name);
                if lambda {
                    if let K::Arrow { params, .. } = &arg.kind {
                        if params.len() > 2 {
                            self.diagnostics
                                .report(arg.pos, format!(".{name} accepts at most two parameters"));
                        }
                    } else {
                        self.diagnostics
                            .report(arg.pos, format!(".{name} requires an arrow function"));
                    }
                }
                self.walk(arg, params, names, reads, lambda);
            }
            for arg in named {
                self.walk(&arg.value, params, names, reads, false);
            }
            return;
        }
        for child in e.children() {
            self.walk(child, params, names, reads, false);
        }
    }
}
fn scalar_text(v: &Value) -> String {
    v.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_string())
}
