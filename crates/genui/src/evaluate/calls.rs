//! Pure built-ins and receiver methods, including bounded interpreted list callbacks.
//! Callbacks keep the same interpreter, lexical scopes, suffixes and step budget;
//! this split adds no runtime function values, host effects or separate evaluator.
use super::{Interpreter, Result};
use crate::{
    ast::{Expr, ExprKind as K, Pos},
    catalog,
    error::Failure,
    math,
    value::{Scope, Value as V},
};

impl Interpreter<'_> {
    pub(super) fn call(&self, name: &str, a: &[V], pos: Pos) -> Result<V> {
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
    pub(super) fn method(
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
fn js_whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
