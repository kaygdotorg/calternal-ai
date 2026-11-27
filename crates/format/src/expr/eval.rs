use std::collections::BTreeMap;

mod scoped;
pub(crate) use scoped::{is_list_limit, reference_key};

use super::math::{aggregate, comparison, day_value, math};

use super::{Expression, Node, NodeKind, Op};
use crate::{Decimal, Diagnostic, Fields, Value, dates, lines::Site};
use crate::{Field, KindRows, Period, PeriodSnapshot};

/// An item or record supplied by the host, with authoritative fields separate.
#[derive(Clone, Copy, Debug)]
pub struct Row<'a> {
    /// Fields decoded from the item file or record.
    pub fields: &'a Fields,
    /// Host-owned built-ins, such as `id`, `author` and `edited`.
    pub metadata: &'a Fields,
}

impl<'a> Row<'a> {
    pub(crate) fn value(&self, name: &str) -> Value {
        self.get(name).cloned().unwrap_or_default()
    }

    pub(crate) fn get(self, name: &str) -> Option<&'a Value> {
        self.metadata.get(name).or_else(|| {
            (!matches!(
                name,
                "id" | "kind" | "created" | "edited" | "author" | "at" | "anchor"
            ))
            .then(|| self.fields.get(name))
            .flatten()
        })
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
    /// Original recurrence seed in the row's civil calendar, when known.
    pub repeat_start: Option<&'a str>,
    /// Whole-kind inputs for the current header formula.
    pub kinds: Option<&'a KindRows<'a>>,
    /// Current and historical values for the active civil periods.
    pub periods: Option<&'a BTreeMap<Period, PeriodSnapshot<'a>>>,
    /// Per-viewer state of the current item.
    pub state: Option<&'a Fields>,
    /// Resolved per-viewer and per-space setting values.
    pub settings: Option<&'a Fields>,
    /// Current item's authorized records with host stamps.
    pub records: &'a [Row<'a>],
    /// Ledger anchors mapped to the number of visible records at or before
    /// that anchor. Entries can preserve a read position after record removal.
    pub record_positions: Option<&'a BTreeMap<String, usize>>,
    /// The containing item when the current row is a record.
    pub item: Option<Row<'a>>,
    /// The item open in the viewer's window.
    pub open_item: Option<Row<'a>>,
    /// IDs of the already-authorized targets of this item's outgoing links.
    pub outgoing_links: &'a [&'a str],
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
    /// Original recurrence seed in the row's civil calendar, when known.
    pub repeat_start: Option<&'a str>,
    /// Period history specific to this authorized item.
    pub periods: Option<&'a BTreeMap<Period, PeriodSnapshot<'a>>>,
    /// State fields for this person and item.
    pub state: Option<&'a Fields>,
    /// Authoritative records in ledger order.
    pub records: &'a [Row<'a>],
    /// Ledger anchors mapped to the number of visible records at or before
    /// that anchor. Entries can preserve a read position after record removal.
    pub record_positions: Option<&'a BTreeMap<String, usize>>,
    /// IDs of the already-authorized targets of this item's outgoing links.
    pub outgoing_links: &'a [&'a str],
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

/// Evaluate a formula using at most 50,000 steps from the caller's remaining
/// budget. Charged work includes variable-sized copies before they allocate.
pub fn evaluate_bounded(
    expression: &Expression,
    row: Row<'_>,
    context: &EvalContext<'_>,
    steps: &mut usize,
) -> Result<Value, Diagnostic> {
    let allowed = (*steps).min(50_000);
    let mut remaining = allowed;
    let result = evaluate_in(expression, row, context, &mut remaining);
    *steps -= allowed - remaining;
    result
}

pub(crate) fn evaluate_in(
    expression: &Expression,
    row: Row<'_>,
    context: &EvalContext<'_>,
    steps: &mut usize,
) -> Result<Value, Diagnostic> {
    Evaluator::new(row, context, steps).eval(&expression.root, None)
}

pub(crate) fn evaluate_field(
    field: &Field,
    row: Row<'_>,
    context: &EvalContext<'_>,
    steps: &mut usize,
    named_total: bool,
) -> Result<Value, Diagnostic> {
    let mut evaluator = Evaluator::new(row, context, steps);
    evaluator.named_total = named_total;
    if let Some(period) = field.period {
        evaluator.period = Some(period);
        evaluator.snapshot = context.periods.and_then(|periods| periods.get(&period));
        if evaluator
            .snapshot
            .map(|snapshot| snapshot.date)
            .or(context.today)
            .and_then(|date| period.label(date))
            .is_none()
        {
            return Ok(Value::Empty);
        }
    }
    evaluator.eval(
        &field.expression.as_ref().expect("a computed field").root,
        None,
    )
}

struct Evaluator<'a, 'b> {
    context: &'b EvalContext<'a>,
    row: Row<'a>,
    steps: &'b mut usize,
    hops: usize,
    candidate: Option<Row<'a>>,
    named_total: bool,
    period: Option<Period>,
    snapshot: Option<&'a PeriodSnapshot<'a>>,
    historical: bool,
}

impl<'a, 'b> Evaluator<'a, 'b> {
    fn new(row: Row<'a>, context: &'b EvalContext<'a>, steps: &'b mut usize) -> Self {
        Self {
            context,
            row,
            steps,
            hops: 0,
            candidate: None,
            named_total: false,
            period: None,
            snapshot: None,
            historical: false,
        }
    }

    fn active_row(&self) -> Option<Row<'a>> {
        if !self.historical {
            return Some(self.row);
        }
        let fields = self.snapshot?.fields?;
        Some(Row {
            fields,
            metadata: self.row.metadata,
        })
    }

    fn charge(&mut self, count: usize, site: Site) -> Result<(), Diagnostic> {
        *self.steps = self
            .steps
            .checked_sub(count)
            .ok_or_else(|| site.error("The formula is too costly"))?;
        Ok(())
    }

    fn charge_bytes(&mut self, bytes: usize, site: Site) -> Result<(), Diagnostic> {
        self.charge(bytes.div_ceil(64), site)
    }

    fn charge_fields(&mut self, fields: &Fields, site: Site) -> Result<(), Diagnostic> {
        self.charge(fields.len(), site)?;
        for (key, value) in fields {
            self.charge_bytes(key.len(), site)?;
            self.charge_value(value, site)?;
        }
        Ok(())
    }

    fn charge_value(&mut self, value: &Value, site: Site) -> Result<(), Diagnostic> {
        match value {
            Value::Text(text) | Value::Date(text) | Value::Time(text) => {
                self.charge_bytes(text.len(), site)
            }
            Value::Map(fields) => self.charge_fields(fields, site),
            Value::List(values) => {
                self.charge(values.len(), site)?;
                for value in values {
                    self.charge_value(value, site)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn clone_value(&mut self, value: Option<&Value>, site: Site) -> Result<Value, Diagnostic> {
        if let Some(value) = value {
            self.charge_value(value, site)?;
        }
        Ok(value.cloned().unwrap_or_default())
    }

    fn clone_row(&mut self, row: Option<Row<'_>>, site: Site) -> Result<Value, Diagnostic> {
        let Some(row) = row else {
            return Ok(Value::Empty);
        };
        self.charge_fields(row.fields, site)?;
        self.charge_fields(row.metadata, site)?;
        Ok(row.as_value())
    }

    fn eval(&mut self, node: &Node, element: Option<&Value>) -> Result<Value, Diagnostic> {
        self.charge(1, node.site)?;
        match &node.kind {
            NodeKind::Value(value) => self.clone_value(Some(value), node.site),
            NodeKind::Name(name) => self.name(name, element, node.site),
            NodeKind::Member(base, field) => self.member(base, field, element, node.site),
            NodeKind::List(values) => {
                self.charge(values.len(), node.site)?;
                values
                    .iter()
                    .map(|value| self.eval(value, element))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::List)
            }
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
        match name {
            "this" => self.clone_row(self.active_row(), site),
            "period" if self.period.is_some() => {
                let date = self
                    .snapshot
                    .map(|snapshot| snapshot.date)
                    .or(self.context.today);
                Ok(date
                    .and_then(|date| self.period?.label(date))
                    .map_or(Value::Empty, Value::Text))
            }
            "item" => self.clone_row(self.context.item, site),
            "open item" => self.clone_row(self.context.open_item, site),
            "records" => {
                self.charge(self.context.records.len(), site)?;
                self.context
                    .records
                    .iter()
                    .map(|row| self.clone_row(Some(*row), site))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::List)
            }
            _ => {
                let value = match element {
                    None => self
                        .candidate
                        .or(self.active_row())
                        .and_then(|row| row.get(name)),
                    Some(Value::Map(fields)) => fields.get(name),
                    Some(_) => None,
                };
                self.clone_value(value, site)
            }
        }
    }

    fn member(
        &mut self,
        base: &Node,
        field: &str,
        element: Option<&Value>,
        site: Site,
    ) -> Result<Value, Diagnostic> {
        if let NodeKind::Name(name) = &base.kind {
            let row = match name.as_str() {
                "this" => Some(self.active_row()),
                "item" => Some(self.context.item),
                "open item" => Some(self.context.open_item),
                _ => None,
            };
            if let Some(row) = row {
                let value = row.and_then(|row| row.get(field));
                return self.clone_value(value, site);
            }
            let scope = match name.as_str() {
                "state" => Some(self.context.state),
                "settings" => Some(self.context.settings),
                _ => None,
            };
            if let Some(scope) = scope {
                return self.clone_value(scope.and_then(|fields| fields.get(field)), site);
            }
        }
        let base = self.eval(base, element)?;
        if let Value::List(values) = base {
            if values.len() > 1_000 && values.iter().any(|value| matches!(value, Value::Text(_))) {
                return Err(scoped::too_many_matches(site));
            }
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
            Value::Text(key) => {
                let value = self.link(&key, site)?.and_then(|row| row.get(field));
                self.clone_value(value, site)
            }
            Value::Empty => Ok(Value::Empty),
            _ => Err(site.error("This value has no fields")),
        }
    }

    fn link(&self, key: &str, site: Site) -> Result<Option<Row<'a>>, Diagnostic> {
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
            "links to" => {
                let target = self.eval(&args[0], element)?;
                let id = match &target {
                    Value::Text(id) => Some(id),
                    Value::Map(fields) => match fields.get("id") {
                        Some(Value::Text(id)) => Some(id),
                        _ => None,
                    },
                    _ => None,
                };
                self.charge(self.context.outgoing_links.len(), site)?;
                return Ok(Value::Bool(id.is_some_and(|id| {
                    self.context.outgoing_links.contains(&id.as_str())
                })));
            }
            "from" => return self.reverse(args, element, site),
            "all" => return self.all(args, site),
            "previous" => return self.previous(&args[0], site),
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
                    .and_then(dates::clock_seconds)
                    .map_or(Value::Empty, Value::Time));
            }
            _ => {}
        }

        if name == "count"
            && let NodeKind::Call(after, position) = &args[0].kind
            && after == "records after"
        {
            let anchor = self.eval(&position[0], element)?;
            let start = match &anchor {
                Value::Text(anchor) => {
                    if let Some(position) = self
                        .context
                        .record_positions
                        .and_then(|positions| positions.get(anchor))
                    {
                        if *position > self.context.records.len() {
                            return Err(site.error(
                                "The ledger read position is outside the visible record order",
                            ));
                        }
                        *position
                    } else {
                        self.charge(self.context.records.len(), site)?;
                        self.context.records.iter().position(|row| {
                            row.metadata.get("anchor").is_some_and(
                                |value| matches!(value, Value::Text(candidate) if candidate == anchor),
                            )
                        }).map_or(0, |index| index + 1)
                    }
                }
                _ => 0,
            };
            return Ok(Value::Number(Decimal::integer(
                (self.context.records.len() - start) as i128,
            )));
        }
        if name == "count"
            && args.len() == 1
            && matches!(&args[0].kind, NodeKind::Name(name) if name == "records")
        {
            return Ok(Value::Number(Decimal::integer(
                self.context.records.len() as i128
            )));
        }
        let first = self.eval(&args[0], element)?;
        if matches!(
            name,
            "count" | "sum" | "average" | "min" | "max" | "first" | "last"
        ) {
            if let Value::List(values) = first {
                let limit = if values.iter().any(|value| {
                    matches!(value, Value::Text(key)
                    if args.get(1).is_some() || self.context.links.is_some_and(|links| links.contains_key(key)))
                }) {
                    1_000
                } else {
                    10_000
                };
                let values = self.filter(values, args.get(1), site, limit)?;
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
            ("contains", Value::Text(text), Value::Text(part)) => {
                self.charge_bytes(text.len().saturating_add(part.len()), site)?;
                Value::Bool(text.contains(&part))
            }
            ("domain", Value::Text(url), _) => {
                self.charge_bytes(url.len(), site)?;
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
                let civil = |value: &Value| match value {
                    Value::Date(value) | Value::Time(value) | Value::Text(value) => {
                        Some(value.clone())
                    }
                    _ => None,
                };
                let seed = self
                    .context
                    .repeat_start
                    .map(str::to_owned)
                    .or_else(|| civil(&self.row.value("start")))
                    .or_else(|| civil(&self.row.value("scheduled")));
                let after = civil(&after)
                    .or_else(|| {
                        seed.as_deref()
                            .filter(|value| dates::time(value).is_some())
                            .and_then(|_| self.context.now.and_then(dates::clock_seconds))
                    })
                    .or_else(|| self.context.today.map(str::to_owned));
                if let Some(after) = after {
                    let seed = if self.row.value("repeat from") == Value::Text("completion".into())
                    {
                        &after
                    } else {
                        seed.as_ref().unwrap_or(&after)
                    };
                    crate::repeat::next_in(&rule, seed, &after, self.steps, site)?
                        .unwrap_or_default()
                } else {
                    Value::Empty
                }
            }
            ("streak", Value::List(records), _) => super::functions::streak(records),
            (_, Value::Empty, _) => Value::Empty,
            _ => return Err(site.error(format!("The arguments to {name} have the wrong types"))),
        };
        Ok(result)
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

#[cfg(test)]
mod tests;
