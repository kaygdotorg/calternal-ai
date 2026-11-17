//! Normalize native node properties, bindings and action descriptions.
//! This module creates data only. All methods use the parent interpreter's counters
//! and diagnostics, preserving component-local recovery and whole-pass resource limits.
use super::{Interpreter, Result};
use crate::{
    ast::{Expr, ExprKind as K, NamedArg, Pos},
    catalog,
    error::Failure,
    validate,
    value::{Scope, Value as V},
};
use serde_json::Value;

impl Interpreter<'_> {
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
    pub(super) fn children(&mut self, v: V, pos: Pos, key: &str) -> Result<Vec<V>> {
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
    pub(super) fn component(
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
    pub(super) fn action(
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
