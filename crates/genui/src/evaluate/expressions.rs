//! Expression dispatch, lazy operators and data access over the shared interpreter state.
//! Node creation and callable implementations stay in their own modules; the caller's
//! counters, lexical scope, named-value cache and source-derived identity are unchanged.
use super::{Interpreter, Result};
use crate::{
    ast::{Expr, ExprKind as K, Pos},
    budgets::forbidden,
    catalog,
    error::Failure,
    value::{Scope, Value as V},
};

impl Interpreter<'_> {
    pub(super) fn expr(&mut self, e: &Expr, scope: &Scope) -> Result<V> {
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
}
