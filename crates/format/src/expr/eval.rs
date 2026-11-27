use std::{cmp::Ordering, collections::BTreeMap};

use super::{Expression, Node, NodeKind, Op};
use crate::{Decimal, Diagnostic, Fields, Value, dates, lines::Site};

/// An item or record supplied by the host, with authoritative fields separate.
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    /// Fields decoded from the item file or record.
    pub fields: &'a Fields,
    /// Host-owned built-ins, such as `id`, `author` and `edited`.
    pub metadata: &'a Fields,
}

impl Row<'_> {
    pub(crate) fn value(&self, name: &str) -> Value {
        self.metadata
            .get(name)
            .or_else(|| {
                (!matches!(
                    name,
                    "id" | "kind" | "created" | "edited" | "author" | "at" | "anchor"
                ))
                .then(|| self.fields.get(name))
                .flatten()
            })
            .cloned()
            .unwrap_or_default()
    }

    fn as_value(self) -> Value {
        let mut fields = self.fields.clone();
        fields.retain(|key, _| {
            !matches!(
                key.as_str(),
                "id" | "kind" | "created" | "edited" | "author" | "at" | "anchor"
            )
        });
        fields.extend(self.metadata.clone());
        Value::Map(fields)
    }
}

/// Caller-supplied time, viewer fields and already-authorized lookups.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvalContext<'a> {
    /// Current date in the viewer's civil calendar.
    pub today: Option<&'a str>,
    /// Current receipt time, including its offset when available.
    pub now: Option<&'a str>,
    /// Offset supplied by the host for civil-date conversion of offset times.
    pub offset_seconds: Option<i32>,
    /// Per-viewer state of the current item.
    pub state: Option<&'a Fields>,
    /// Resolved per-viewer and per-space setting values.
    pub settings: Option<&'a Fields>,
    /// Current item's authorized records with host stamps.
    pub records: &'a [Row<'a>],
    /// The containing item when the current row is a record.
    pub item: Option<Row<'a>>,
    /// The item open in the viewer's window.
    pub open_item: Option<Row<'a>>,
    /// Link targets indexed by reference string. Unavailable targets are omitted.
    pub links: Option<&'a BTreeMap<String, Row<'a>>>,
    /// Per-item context indexed by authoritative item ID for in-memory queries.
    pub rows: Option<&'a BTreeMap<String, QueryRowContext<'a>>>,
    /// Indexed reverse candidates keyed by kind and link field, for this row.
    pub reverse: Option<&'a BTreeMap<String, Vec<Row<'a>>>>,
}

/// Viewer data to use when a query evaluates one item.
#[derive(Clone, Copy, Debug, Default)]
pub struct QueryRowContext<'a> {
    /// State fields for this person and item.
    pub state: Option<&'a Fields>,
    /// Authoritative records in ledger order.
    pub records: &'a [Row<'a>],
    /// Already-authorized reverse lookup candidates.
    pub reverse: Option<&'a BTreeMap<String, Vec<Row<'a>>>>,
}

/// Evaluate one formula over an authorized item, using a fixed work budget.
pub fn evaluate(
    expression: &Expression,
    row: Row<'_>,
    context: &EvalContext<'_>,
) -> Result<Value, Diagnostic> {
    let mut steps = 50_000;
    evaluate_in(expression, row, context, &mut steps)
}

pub(crate) fn evaluate_in(
    expression: &Expression,
    row: Row<'_>,
    context: &EvalContext<'_>,
    steps: &mut usize,
) -> Result<Value, Diagnostic> {
    let mut evaluator = Evaluator {
        context,
        row,
        steps,
        hops: 0,
    };
    evaluator.eval(&expression.root, None)
}

struct Evaluator<'a, 'b> {
    context: &'b EvalContext<'a>,
    row: Row<'a>,
    steps: &'b mut usize,
    hops: usize,
}

impl Evaluator<'_, '_> {
    fn charge(&mut self, count: usize, site: Site) -> Result<(), Diagnostic> {
        *self.steps = self
            .steps
            .checked_sub(count)
            .ok_or_else(|| site.error("The formula is too costly"))?;
        Ok(())
    }

    fn eval(&mut self, node: &Node, element: Option<&Value>) -> Result<Value, Diagnostic> {
        self.charge(1, node.site)?;
        match &node.kind {
            NodeKind::Value(value) => Ok(value.clone()),
            NodeKind::Name(name) => self.name(name, element, node.site),
            NodeKind::Member(base, field) => self.member(base, field, element, node.site),
            NodeKind::List(values) => values
                .iter()
                .map(|value| self.eval(value, element))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List),
            NodeKind::Unary(op, operand) => {
                let value = self.eval(operand, element)?;
                if *op == Op::Not {
                    return Ok(Value::Bool(!value.truth()));
                }
                math(Op::Sub, Value::Number(Decimal::ZERO), value, node.site)
            }
            NodeKind::Binary(op, left, right) => {
                let left = self.eval(left, element)?;
                match op {
                    Op::And if !left.truth() => return Ok(Value::Bool(false)),
                    Op::Or if left.truth() => return Ok(Value::Bool(true)),
                    _ => {}
                }
                let right = self.eval(right, element)?;
                match op {
                    Op::And | Op::Or => Ok(Value::Bool(right.truth())),
                    Op::In => Ok(Value::Bool(
                        matches!(right, Value::List(values) if values.iter().any(|value| left != Value::Empty && left == *value)),
                    )),
                    Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => {
                        comparison(*op, &left, &right, node.site)
                    }
                    _ => math(*op, left, right, node.site),
                }
            }
            NodeKind::Call(name, args) => self.call(name, args, element, node.site),
        }
    }

    fn name(
        &mut self,
        name: &str,
        element: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        Ok(match name {
            "this" => self.row.as_value(),
            "item" => self.context.item.map_or(Value::Empty, Row::as_value),
            "open item" => self.context.open_item.map_or(Value::Empty, Row::as_value),
            "records" => {
                self.charge(self.context.records.len(), site)?;
                Value::List(
                    self.context
                        .records
                        .iter()
                        .map(|row| row.as_value())
                        .collect(),
                )
            }
            _ => element.map_or_else(
                || self.row.value(name),
                |value| {
                    if let Value::Map(fields) = value {
                        fields.get(name).cloned().unwrap_or_default()
                    } else {
                        Value::Empty
                    }
                },
            ),
        })
    }

    fn member(
        &mut self,
        base: &Node,
        field: &str,
        element: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        if let NodeKind::Name(name) = &base.kind {
            let scope = match name.as_str() {
                "state" => Some(self.context.state),
                "settings" => Some(self.context.settings),
                _ => None,
            };
            if let Some(scope) = scope {
                return Ok(scope
                    .and_then(|fields| fields.get(field))
                    .cloned()
                    .unwrap_or_default());
            }
        }
        let base = self.eval(base, element)?;
        if let Value::List(values) = base {
            self.charge(values.len(), site)?;
            return values
                .into_iter()
                .map(|value| self.read_member(value, field, site))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::List);
        }
        self.read_member(base, field, site)
    }

    fn read_member(&mut self, value: Value, field: &str, site: Site) -> Result<Value, Diagnostic> {
        match value {
            Value::Map(mut fields) => Ok(fields.remove(field).unwrap_or_default()),
            Value::Text(key) => self
                .link(&key, site)
                .map(|row| row.map_or(Value::Empty, |row| row.value(field))),
            Value::Empty => Ok(Value::Empty),
            _ => Err(site.error("This value has no fields")),
        }
    }

    fn link(&self, key: &str, site: Site) -> Result<Option<Row<'_>>, Diagnostic> {
        if self.hops >= 1 {
            return Err(site.error("A formula follows at most one link"));
        }
        Ok(self.context.links.and_then(|links| links.get(key)).copied())
    }

    fn call(
        &mut self,
        name: &str,
        args: &[Node],
        element: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        match name {
            "if" => {
                let test = self.eval(&args[0], element)?;
                return self.eval(&args[if test.truth() { 1 } else { 2 }], element);
            }
            "from" => return self.reverse(args, element, site),
            "today" => {
                return Ok(self
                    .context
                    .today
                    .filter(|value| dates::day(value).is_some())
                    .map_or(Value::Empty, |date| Value::Date(date.into())));
            }
            "now" => {
                return Ok(self
                    .context
                    .now
                    .filter(|value| dates::time(value).is_some())
                    .map_or(Value::Empty, |time| Value::Time(time.into())));
            }
            _ => {}
        }

        if name == "count" {
            if let NodeKind::Call(after, position) = &args[0].kind {
                if after == "records after" {
                    let anchor = self.eval(&position[0], element)?;
                    let mut start = 0;
                    if let Value::Text(anchor) = &anchor {
                        if let Some(index) = self.context.records.iter().position(|row| {
                            row.metadata
                                .get("anchor")
                                .is_some_and(|value| value == &Value::Text(anchor.clone()))
                        }) {
                            start = index + 1;
                        }
                    }
                    self.charge(self.context.records.len(), site)?;
                    return Ok(Value::Number(Decimal::integer(
                        (self.context.records.len() - start) as i128,
                    )));
                }
            }
        }
        let first = self.eval(&args[0], element)?;
        if matches!(
            name,
            "count" | "sum" | "average" | "min" | "max" | "first" | "last"
        ) {
            if let Value::List(values) = first {
                let values = self.filter(values, args.get(1), site, 10_000)?;
                return aggregate(name, values, site);
            }
            if name == "first" {
                if first != Value::Empty {
                    return Ok(first);
                }
                for arg in &args[1..] {
                    let value = self.eval(arg, element)?;
                    if value != Value::Empty {
                        return Ok(value);
                    }
                }
                return Ok(Value::Empty);
            }
            if matches!(name, "min" | "max") && args.len() == 2 {
                return aggregate(name, vec![first, self.eval(&args[1], element)?], site);
            }
            if first == Value::Empty {
                return aggregate(name, Vec::new(), site);
            }
            return Err(site.error(format!("{name} expects a list")));
        }

        let second = args
            .get(1)
            .map(|node| self.eval(node, element))
            .transpose()?
            .unwrap_or_default();
        let result = match (name, first, second) {
            ("contains", Value::Text(text), Value::Text(part)) => Value::Bool(text.contains(&part)),
            ("domain", Value::Text(url), _) => {
                super::functions::domain(&url).map_or(Value::Empty, Value::Text)
            }
            ("under", Value::List(tags), Value::Text(parent)) => {
                let prefix = format!("{parent}/");
                Value::List(
                    tags.into_iter()
                        .filter(|tag| matches!(tag, Value::Text(text) if text.starts_with(&prefix)))
                        .collect(),
                )
            }
            ("days", a, b) => match (
                day_value(&a, self.context.offset_seconds),
                day_value(&b, self.context.offset_seconds),
            ) {
                (Some(a), Some(b)) => Value::Duration {
                    months: 0,
                    seconds: (b - a) * 86400,
                },
                _ => Value::Empty,
            },
            ("date", value, _) => day_value(&value, self.context.offset_seconds)
                .and_then(dates::render_day)
                .map_or(Value::Empty, Value::Date),
            (unit @ ("week" | "month" | "year"), value, _) => {
                day_value(&value, self.context.offset_seconds)
                    .and_then(dates::render_day)
                    .and_then(|date| dates::period(&date, unit))
                    .map_or(Value::Empty, Value::Text)
            }
            ("round", Value::Number(n), precision) => {
                Value::Number(n.rounded(round_places(precision, site)?))
            }
            ("round", Value::Money { amount, currency }, precision) => Value::Money {
                amount: amount.rounded(round_places(precision, site)?),
                currency,
            },
            ("format", value, Value::Text(pattern)) => {
                super::functions::format(value, &pattern, site)?
            }
            ("next", Value::Text(rule), after) => {
                let start = day_value(&after, self.context.offset_seconds)
                    .or_else(|| self.context.today.and_then(dates::day));
                match start {
                    Some(start) => super::functions::next(&rule, start, self.steps, site)?,
                    None => Value::Empty,
                }
            }
            ("streak", Value::List(records), _) => super::functions::streak(records),
            (_, Value::Empty, _) => Value::Empty,
            _ => return Err(site.error(format!("The arguments to {name} have the wrong types"))),
        };
        Ok(result)
    }

    fn reverse(
        &mut self,
        args: &[Node],
        outer: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        if outer.is_some() || self.hops != 0 {
            return Err(site.error("A formula follows at most one link"));
        }
        let key = reverse_key(&args[0])
            .ok_or_else(|| site.error("Name a kind and its link field in from"))?;
        let predicate = args.get(1).filter(
            |node| !matches!(&node.kind, NodeKind::Name(name) if name == "ignoring default"),
        );
        let candidates = self
            .context
            .reverse
            .and_then(|reverse| reverse.get(&key))
            .map_or(&[][..], Vec::as_slice);
        if candidates.len() > 10_000 {
            return Err(site.error("Too many items to test"));
        }
        self.charge(candidates.len(), site)?;
        self.hops += 1;
        let values = self.filter(
            candidates.iter().map(|row| row.as_value()).collect(),
            predicate,
            site,
            1000,
        );
        self.hops -= 1;
        values.map(Value::List)
    }

    fn filter(
        &mut self,
        values: Vec<Value>,
        predicate: Option<&Node>,
        site: Site,
        limit: usize,
    ) -> Result<Vec<Value>, Diagnostic> {
        if values.len() > 10_000 {
            return Err(site.error("Too many items to test"));
        }
        self.charge(values.len(), site)?;
        let mut output = Vec::new();
        for value in values {
            if let Some(condition) = predicate {
                let scope = match &value {
                    Value::Text(key) => self.link(key, site)?.map_or(Value::Empty, Row::as_value),
                    _ => value.clone(),
                };
                let previous = self.hops;
                self.hops += usize::from(matches!(value, Value::Text(_)));
                let passes = self.eval(condition, Some(&scope));
                self.hops = previous;
                if !passes?.truth() {
                    continue;
                }
            }
            if output.len() == limit {
                return Err(site.error("Too many items matched the formula"));
            }
            output.push(value);
        }
        Ok(output)
    }
}

fn reverse_key(node: &Node) -> Option<String> {
    match &node.kind {
        NodeKind::Name(name) => Some(name.clone()),
        NodeKind::Member(base, field) => Some(format!("{}.{}", reverse_key(base)?, field)),
        _ => None,
    }
}

fn round_places(value: Value, site: Site) -> Result<u32, Diagnostic> {
    match value {
        Value::Empty => Ok(0),
        Value::Number(value) => value
            .whole()
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n <= 18)
            .ok_or_else(|| site.error("Round to between 0 and 18 decimal places")),
        _ => Err(site.error("The number of decimal places must be a whole number")),
    }
}

pub(crate) fn day_value(value: &Value, offset: Option<i32>) -> Option<i64> {
    match value {
        Value::Date(date) => dates::day(date),
        Value::Time(time) => dates::time(time).map(|(seconds, at)| {
            let offset = offset.filter(|_| {
                time.get(11..)
                    .is_some_and(|tail| tail.contains(['Z', '+', '-']))
            });
            (seconds + i64::from(offset.unwrap_or(at) - at)).div_euclid(86400)
        }),
        _ => None,
    }
}

pub(crate) fn compare(
    left: &Value,
    right: &Value,
    site: Site,
) -> Result<Option<Ordering>, Diagnostic> {
    Ok(match (left, right) {
        (Value::Number(a), Value::Number(b)) => Some(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Some(a.cmp(b)),
        (Value::Text(a), Value::Text(b)) => Some(a.cmp(b)),
        (Value::Date(a), Value::Date(b)) => Some(a.cmp(b)),
        (Value::Time(a), Value::Time(b)) => dates::time(a)
            .zip(dates::time(b))
            .map(|((a, ao), (b, bo))| (a - i64::from(ao)).cmp(&(b - i64::from(bo)))),
        (Value::Time(_), Value::Date(_)) | (Value::Date(_), Value::Time(_)) => {
            day_value(left, None)
                .zip(day_value(right, None))
                .map(|(a, b)| a.cmp(&b))
        }
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if a == b => Some(x.cmp(y)),
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if x == y => Some(a.cmp(b)),
        (
            Value::Money {
                amount: a,
                currency: ac,
            },
            Value::Money {
                amount: b,
                currency: bc,
            },
        ) => {
            if ac != bc {
                return Err(site.error("Compare or combine amounts in the same currency"));
            }
            Some(a.cmp(b))
        }
        _ => None,
    })
}

fn comparison(op: Op, left: &Value, right: &Value, site: Site) -> Result<Value, Diagnostic> {
    if matches!(left, Value::Empty) || matches!(right, Value::Empty) {
        return Ok(Value::Bool(match op {
            Op::Eq => left == right,
            Op::Ne => left != right,
            _ => false,
        }));
    }
    let order = compare(left, right, site)?;
    let equal = order.map_or(left == right, |order| order == Ordering::Equal);
    Ok(Value::Bool(match op {
        Op::Eq => equal,
        Op::Ne => !equal,
        Op::Lt => order == Some(Ordering::Less),
        Op::Le => matches!(order, Some(Ordering::Less | Ordering::Equal)),
        Op::Gt => order == Some(Ordering::Greater),
        Op::Ge => matches!(order, Some(Ordering::Greater | Ordering::Equal)),
        _ => unreachable!(),
    }))
}

pub(crate) fn aggregate(name: &str, values: Vec<Value>, site: Site) -> Result<Value, Diagnostic> {
    if name == "count" {
        return Ok(Value::Number(Decimal::integer(values.len() as i128)));
    }
    if name == "first" {
        return Ok(values
            .into_iter()
            .find(|value| *value != Value::Empty)
            .unwrap_or_default());
    }
    if name == "last" {
        return Ok(values.into_iter().next_back().unwrap_or_default());
    }
    let mut values = values.into_iter().filter(|value| *value != Value::Empty);
    let Some(first) = values.next() else {
        return Ok(if name == "sum" {
            Value::Number(Decimal::ZERO)
        } else {
            Value::Empty
        });
    };
    let mut value = first;
    let mut count = 1;
    for next in values {
        count += 1;
        match name {
            "min" | "max" => {
                let order = compare(&next, &value, site)?
                    .ok_or_else(|| site.error("These values cannot be ordered together"))?;
                if (name == "min" && order.is_lt()) || (name == "max" && order.is_gt()) {
                    value = next
                }
            }
            "sum" | "average" => value = math(Op::Add, value, next, site)?,
            _ => return Err(site.error("Unknown total")),
        }
    }
    if name == "average" {
        math(Op::Div, value, Value::Number(Decimal::integer(count)), site)
    } else {
        Ok(value)
    }
}

fn math(op: Op, left: Value, right: Value, site: Site) -> Result<Value, Diagnostic> {
    let overflow =
        || site.error("The arithmetic result exceeds the supported range or divides by zero");
    let number = |a: Decimal, b: Decimal| {
        match op {
            Op::Add => a.add(b),
            Op::Sub => a.add(b.neg()),
            Op::Mul => a.mul(b),
            Op::Div => a.div(b),
            _ => None,
        }
        .ok_or_else(overflow)
    };
    let result = match (left, right) {
        (Value::Empty, _) | (_, Value::Empty) => Value::Empty,
        (Value::Number(a), Value::Number(b)) => Value::Number(number(a, b)?),
        (Value::Number(a), Value::Money { amount, currency })
            if op == Op::Mul || (op == Op::Sub && a == Decimal::ZERO) =>
        {
            Value::Money {
                amount: number(a, amount)?,
                currency,
            }
        }
        (Value::Money { amount, currency }, Value::Number(b))
            if matches!(op, Op::Mul | Op::Div) =>
        {
            Value::Money {
                amount: number(amount, b)?,
                currency,
            }
        }
        (
            Value::Money {
                amount: a,
                currency: ac,
            },
            Value::Money {
                amount: b,
                currency: bc,
            },
        ) => {
            if ac != bc {
                return Err(site.error("Use the same currency for both amounts"));
            }
            match op {
                Op::Add | Op::Sub => Value::Money {
                    amount: number(a, b)?,
                    currency: ac,
                },
                Op::Div => Value::Number(number(a, b)?),
                _ => return Err(site.error("Multiplying two money amounts is undefined")),
            }
        }
        (
            Value::Duration {
                months: a,
                seconds: x,
            },
            Value::Duration {
                months: b,
                seconds: y,
            },
        ) if matches!(op, Op::Add | Op::Sub) => {
            let (months, seconds) = if op == Op::Add {
                (a.checked_add(b), x.checked_add(y))
            } else {
                (a.checked_sub(b), x.checked_sub(y))
            };
            Value::Duration {
                months: months.ok_or_else(overflow)?,
                seconds: seconds.ok_or_else(overflow)?,
            }
        }
        (Value::Number(value), Value::Duration { months, seconds })
            if op == Op::Sub && value == Decimal::ZERO =>
        {
            Value::Duration {
                months: months.checked_neg().ok_or_else(overflow)?,
                seconds: seconds.checked_neg().ok_or_else(overflow)?,
            }
        }
        (Value::Date(date), Value::Duration { months, seconds })
            if matches!(op, Op::Add | Op::Sub) =>
        {
            let (months, seconds) = if op == Op::Add {
                (Some(months), Some(seconds))
            } else {
                (months.checked_neg(), seconds.checked_neg())
            };
            let day = dates::shifted_day(
                dates::day(&date).ok_or_else(overflow)?,
                months.ok_or_else(overflow)?,
                seconds.ok_or_else(overflow)?,
            );
            Value::Date(day.and_then(dates::render_day).ok_or_else(overflow)?)
        }
        (Value::Time(time), Value::Duration { months, seconds })
            if matches!(op, Op::Add | Op::Sub) =>
        {
            let (months, seconds) = if op == Op::Add {
                (Some(months), Some(seconds))
            } else {
                (months.checked_neg(), seconds.checked_neg())
            };
            Value::Time(
                dates::shifted_time(
                    &time,
                    months.ok_or_else(overflow)?,
                    seconds.ok_or_else(overflow)?,
                )
                .ok_or_else(overflow)?,
            )
        }
        (Value::Date(a), Value::Date(b)) if op == Op::Sub => Value::Duration {
            months: 0,
            seconds: (dates::day(&a).ok_or_else(overflow)?
                - dates::day(&b).ok_or_else(overflow)?)
                * 86400,
        },
        (Value::Time(a), Value::Time(b)) if op == Op::Sub => {
            let (a, ao) = dates::time(&a).ok_or_else(overflow)?;
            let (b, bo) = dates::time(&b).ok_or_else(overflow)?;
            Value::Duration {
                months: 0,
                seconds: (a - i64::from(ao)) - (b - i64::from(bo)),
            }
        }
        _ => return Err(site.error("These values do not support this arithmetic operation")),
    };
    Ok(result)
}
