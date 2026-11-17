//! Interpreter values retain reference identity internally; exported descriptions remain JSON data.
use crate::{ast::Pos, error::Failure};
use serde_json::{Map, Value as Json};
use std::{collections::HashMap, rc::Rc};
#[derive(Clone, Debug)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Rc<Vec<Value>>),
    Object(Rc<Vec<(String, Value)>>),
}
impl Value {
    pub fn from_json(v: &Json) -> Self {
        match v {
            Json::Null => Self::Null,
            Json::Bool(b) => Self::Bool(*b),
            Json::Number(n) => Self::Number(n.as_f64().unwrap()),
            Json::String(s) => Self::String(s.clone()),
            Json::Array(a) => Self::list(a.iter().map(Self::from_json).collect()),
            Json::Object(o) => Self::object(
                o.iter()
                    .map(|(k, v)| (k.clone(), Self::from_json(v)))
                    .collect(),
            ),
        }
    }
    pub fn json(&self) -> Json {
        match self {
            Self::Null => Json::Null,
            Self::Bool(b) => (*b).into(),
            Self::Number(n) => serde_json::json!(n),
            Self::String(s) => s.clone().into(),
            Self::List(a) => Json::Array(a.iter().map(Self::json).collect()),
            Self::Object(o) => Json::Object(
                o.iter()
                    .map(|(k, v)| (k.clone(), v.json()))
                    .collect::<Map<_, _>>(),
            ),
        }
    }
    pub fn list(a: Vec<Value>) -> Self {
        Self::List(Rc::new(a))
    }
    pub fn object(a: Vec<(String, Value)>) -> Self {
        let mut out: Vec<(String, Value)> = vec![];
        for (k, v) in a {
            if let Some(entry) = out.iter_mut().find(|(name, _)| *name == k) {
                entry.1 = v;
            } else {
                out.push((k, v));
            }
        }
        // ECMAScript enumerates array-index keys first, in numeric order.
        out.sort_by_key(|(k, _)| {
            k.parse::<u32>()
                .ok()
                .filter(|i| *i != u32::MAX && i.to_string() == *k)
                .map(|i| (0, i))
                .unwrap_or((1, 0))
        });
        Self::Object(Rc::new(out))
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        if let Self::Object(o) = self {
            o.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
    pub fn str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn truthy(&self) -> bool {
        !matches!(self, Self::Null | Self::Bool(false))
            && !matches!(self,Self::Number(n) if *n==0.0)
            && !matches!(self,Self::String(s) if s.is_empty())
    }
    pub fn number(&self, pos: Pos) -> Result<f64, Failure> {
        if let Self::Number(n) = self {
            Ok(*n)
        } else {
            Err(Failure::new(pos, "expected a number"))
        }
    }
    pub fn text(&self, pos: Pos) -> Result<String, Failure> {
        Ok(match self {
            Self::Null => String::new(),
            Self::Bool(v) => v.to_string(),
            Self::Number(v) => number_text(*v),
            Self::String(s) => s.clone(),
            _ => {
                return Err(Failure::new(
                    pos,
                    "cannot convert a structured value to text",
                ));
            }
        })
    }
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::List(a), Self::List(b)) => Rc::ptr_eq(a, b),
            (Self::Object(a), Self::Object(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub fn finite(n: f64) -> Self {
        if n.is_finite() {
            Self::Number(if n == 0.0 { 0.0 } else { n })
        } else {
            Self::Null
        }
    }
}
pub(crate) type Scope = HashMap<String, Value>;
pub(crate) fn number_text(n: f64) -> String {
    if n == 0.0 {
        return "0".into();
    }
    let negative = n < 0.0;
    let raw = format!("{:?}", n.abs());
    let (mantissa, exp) = raw
        .split_once('e')
        .map(|(m, e)| (m, e.parse::<i32>().unwrap()))
        .unwrap_or((&raw, 0));
    let decimal = mantissa.find('.').unwrap_or(mantissa.len()) as i32;
    let mut digits = mantissa.replace('.', "");
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }
    let point = decimal + exp;
    let s = if point > 0 && point <= 21 {
        if point as usize >= digits.len() {
            format!("{}{}", digits, "0".repeat(point as usize - digits.len()))
        } else {
            format!(
                "{}.{}",
                &digits[..point as usize],
                &digits[point as usize..]
            )
        }
    } else if point <= 0 && point > -6 {
        format!("0.{}{}", "0".repeat((-point) as usize), digits)
    } else {
        let power = point - 1;
        let tail = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        format!(
            "{}{tail}e{}{power}",
            &digits[..1],
            if power >= 0 { "+" } else { "" }
        )
    };
    if negative { format!("-{s}") } else { s }
}
