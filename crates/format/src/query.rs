mod calendar;
pub use calendar::{CalendarOccurrence, CalendarSeries, expand_calendar_window};

use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    Collection, Diagnostic, EvalContext, Field, Fields, Kind, Row, Value,
    expr::{
        eval::{evaluate_field, evaluate_in, is_list_limit},
        math::aggregate,
    },
    operation::typed_fields,
};

/// Offset and maximum number of matching rows to return.
#[derive(Clone, Copy, Debug)]
pub struct Page {
    /// Skip this many visible matches after sorting.
    pub offset: usize,
    /// Return at most this many matches.
    pub limit: usize,
}

impl Default for Page {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
        }
    }
}

/// Visible rows and aggregate values calculated over the pre-page matches.
#[derive(Clone, Debug)]
pub struct QueryResult {
    /// Sorted rows in the requested page.
    pub rows: Vec<QueryRow>,
    /// The total number of rows matching the filters.
    pub matched: usize,
    /// Group membership and group totals.
    pub groups: Vec<QueryGroup>,
    /// Overall totals in collection declaration order.
    pub totals: Vec<Value>,
    /// One value for each whole-kind header formula.
    pub named_totals: Fields,
    /// Diagnostics isolated to a named total. Its corresponding value is empty.
    pub named_total_errors: BTreeMap<String, Diagnostic>,
}

/// One row in the result, backreferenced to the host input.
#[derive(Clone, Debug)]
pub struct QueryRow {
    /// Index into the supplied rows.
    pub index: usize,
    /// Computed columns for this row.
    pub computed: Fields,
    /// Group keys for this row, in the order returned by its group formula.
    pub groups: Vec<Value>,
}

/// One collection group with aggregate totals.
#[derive(Clone, Debug)]
pub struct QueryGroup {
    /// The value used as the group key.
    pub key: Value,
    /// Number of visible matching rows in the group.
    pub matched: usize,
    /// Group aggregates in the same order as the collection declaration.
    pub totals: Vec<Value>,
}

/// Evaluate a compiled collection over the supplied visible rows.
/// The host supplies manual order and resolves rights, sources and shares first.
pub fn evaluate_query(
    collection: &Collection,
    kind: &Kind,
    rows: &[Row<'_>],
    page: Page,
    context: &EvalContext<'_>,
) -> Result<QueryResult, Diagnostic> {
    evaluate_query_matching(collection, kind, rows, page, context, &|_, _| true)
}

/// Evaluate a collection and apply a host-supplied match before grouping,
/// totals and pagination. The predicate sees the candidate index and its
/// computed row fields. The host bounds the cost and inputs of the predicate.
pub fn evaluate_query_matching(
    collection: &Collection,
    kind: &Kind,
    rows: &[Row<'_>],
    page: Page,
    context: &EvalContext<'_>,
    matches_row: &dyn Fn(usize, Row<'_>) -> bool,
) -> Result<QueryResult, Diagnostic> {
    evaluate_query_bounded(
        collection,
        kind,
        rows,
        page,
        context,
        matches_row,
        &mut 1_000_000,
    )
}

/// Evaluate matching rows within a caller-supplied cumulative work budget.
/// Charges formulas and sized copies before allocating query intermediates.
pub fn evaluate_query_bounded(
    collection: &Collection,
    kind: &Kind,
    rows: &[Row<'_>],
    page: Page,
    context: &EvalContext<'_>,
    matches_row: &dyn Fn(usize, Row<'_>) -> bool,
    steps: &mut usize,
) -> Result<QueryResult, Diagnostic> {
    let mut matches = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        *steps = steps
            .checked_sub(1)
            .ok_or_else(|| collection.site.error("The collection query is too costly"))?;
        let allowed = (*steps).min(50_000);
        let mut item_steps = allowed;
        if let Some(row) = prepare_row(
            collection,
            kind,
            *row,
            index,
            context,
            &mut item_steps,
            matches_row,
        )? {
            matches.push(row);
        }
        *steps -= allowed - item_steps;
    }
    let sort_steps = matches
        .len()
        .saturating_mul(matches.len().checked_ilog2().unwrap_or(0) as usize)
        .saturating_mul(collection.sort.len());
    let total_steps = matches.len().saturating_mul(collection.totals.len());
    *steps = steps
        .checked_sub(sort_steps.saturating_add(total_steps))
        .ok_or_else(|| collection.site.error("The collection query is too costly"))?;
    matches.sort_by(|a, b| {
        for (at, column) in collection.sort.iter().enumerate() {
            if column.field == "manual" {
                return a.index.cmp(&b.index);
            }
            let order = match (&a.sort[at], &b.sort[at]) {
                (Value::Empty, Value::Empty) => Ordering::Equal,
                (Value::Empty, _) => return Ordering::Greater,
                (_, Value::Empty) => return Ordering::Less,
                (a, b) => sort_values(a, b),
            };
            if !order.is_eq() {
                return if column.descending {
                    order.reverse()
                } else {
                    order
                };
            }
        }
        a.index.cmp(&b.index)
    });
    let aggregate_rows =
        |items: &[&Matched], steps: &mut usize| -> Result<Vec<Value>, Diagnostic> {
            collection
                .totals
                .iter()
                .enumerate()
                .map(|(index, total)| {
                    let values = items
                        .iter()
                        .map(|row| &row.totals[index])
                        .filter(|value| **value != Value::Empty)
                        .map(|value| charged_copy(Some(value), steps, collection.site))
                        .collect::<Result<Vec<_>, _>>()?;
                    aggregate(&total.function, values, collection.site)
                })
                .collect()
        };
    let totals = aggregate_rows(&matches.iter().collect::<Vec<_>>(), steps)?;
    let mut groups: Vec<(Value, Vec<&Matched>)> = Vec::new();
    for row in &matches {
        let keys = match &row.group {
            Some(Value::List(keys)) => keys.as_slice(),
            Some(key) => std::slice::from_ref(key),
            None => continue,
        };
        let mut seen = Vec::new();
        for key in keys {
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            *steps = steps
                .checked_sub(
                    groups
                        .len()
                        .saturating_add(collection.totals.len())
                        .saturating_add(1),
                )
                .ok_or_else(|| collection.site.error("The collection has too many groups"))?;
            if let Some((_, members)) = groups.iter_mut().find(|(group_key, _)| group_key == key) {
                members.push(row);
            } else {
                let key = charged_copy(Some(key), steps, collection.site)?;
                groups.push((key, vec![row]));
            }
        }
    }
    let groups = groups
        .into_iter()
        .map(|(key, members)| {
            Ok(QueryGroup {
                key,
                matched: members.len(),
                totals: aggregate_rows(&members, steps)?,
            })
        })
        .collect::<Result<_, Diagnostic>>()?;
    let matched = matches.len();
    let rows = matches
        .into_iter()
        .skip(page.offset)
        .take(page.limit)
        .map(|row| QueryRow {
            index: row.index,
            computed: row.computed,
            groups: match row.group {
                Some(Value::List(keys)) => keys,
                Some(key) => vec![key],
                None => Vec::new(),
            },
        })
        .collect();
    let mut named_totals = Fields::new();
    let mut named_total_errors = BTreeMap::new();
    let metadata = Fields::new();
    for name in &collection.totals_computed {
        let field = &collection.named_totals[name];
        let row = Row {
            fields: &named_totals,
            metadata: &metadata,
        };
        let allowed = (*steps).min(50_000);
        let mut budget = allowed;
        let value =
            evaluate_field(field, row, context, &mut budget, true).unwrap_or_else(|error| {
                named_total_errors.insert(name.clone(), error);
                Value::Empty
            });
        *steps -= allowed - budget;
        named_totals.insert(name.clone(), value);
    }
    Ok(QueryResult {
        rows,
        matched,
        groups,
        totals,
        named_totals,
        named_total_errors,
    })
}

fn prepare_row(
    collection: &Collection,
    kind: &Kind,
    row: Row<'_>,
    index: usize,
    context: &EvalContext<'_>,
    steps: &mut usize,
    matches_row: &dyn Fn(usize, Row<'_>) -> bool,
) -> Result<Option<Matched>, Diagnostic> {
    let mut row_context = *context;
    row_context.periods = None;
    if let Some(per_row) = context.rows.and_then(|rows| match row.metadata.get("id") {
        Some(Value::Text(id)) => rows.get(id),
        _ => None,
    }) {
        row_context.repeat_start = per_row.repeat_start.or(context.repeat_start);
        row_context.periods = per_row.periods;
        row_context.state = per_row.state;
        row_context.records = per_row.records;
        row_context.record_positions = per_row.record_positions;
        row_context.reverse = per_row.reverse;
        row_context.outgoing_links = per_row.outgoing_links;
    }
    for (key, value) in row.fields {
        charge_copy(
            key.len()
                .saturating_add(value.copy_bytes())
                .saturating_mul(2),
            steps,
            collection.site,
        )?;
    }
    let stored = typed_fields(&kind.fields, row.fields, |_| kind.site, true)?;
    let fields = computed_values(
        stored,
        row.metadata,
        &kind.fields,
        &kind.computed,
        &row_context,
        steps,
    )?;
    let fields = computed_values(
        fields,
        row.metadata,
        &collection.fields,
        &collection.computed,
        &row_context,
        steps,
    )?;
    let row = Row {
        fields: &fields,
        metadata: row.metadata,
    };
    if !collection.ignoring_default
        && let Some(condition) = &kind.default_where
        && !evaluate_in(condition, row, &row_context, steps)?.truth()
    {
        return Ok(None);
    }
    if let Some(condition) = &collection.condition
        && !evaluate_in(condition, row, &row_context, steps)?.truth()
    {
        return Ok(None);
    }
    if !matches_row(index, row) {
        return Ok(None);
    }
    let group = collection
        .group
        .as_ref()
        .map(|group| evaluate_in(group, row, &row_context, steps))
        .transpose()?;
    let sort = collection
        .sort
        .iter()
        .map(|sort| charged_copy(row.get(&sort.field), steps, collection.site))
        .collect::<Result<_, _>>()?;
    let totals = collection
        .totals
        .iter()
        .map(|total| match &total.field {
            Some(field) => charged_copy(row.get(field), steps, collection.site),
            None => Ok(Value::Bool(true)),
        })
        .collect::<Result<_, _>>()?;
    let mut fields = fields;
    let mut computed = Fields::new();
    for field in kind.computed.iter().chain(&collection.computed) {
        if let Some(value) = fields.remove(field) {
            charge_copy(
                field.len().saturating_add(std::mem::size_of::<Value>()),
                steps,
                collection.site,
            )?;
            computed.insert(field.clone(), value);
        }
    }
    Ok(Some(Matched {
        index,
        sort,
        totals,
        group,
        computed,
    }))
}

fn charge_copy(
    bytes: usize,
    steps: &mut usize,
    site: crate::lines::Site,
) -> Result<(), Diagnostic> {
    *steps = steps
        .checked_sub(bytes.div_ceil(64))
        .ok_or_else(|| site.error("The collection query is too costly"))?;
    Ok(())
}

fn charged_copy(
    value: Option<&Value>,
    steps: &mut usize,
    site: crate::lines::Site,
) -> Result<Value, Diagnostic> {
    if let Some(value) = value {
        charge_copy(value.copy_bytes(), steps, site)?;
    }
    Ok(value.cloned().unwrap_or_default())
}

fn sort_values(a: &Value, b: &Value) -> Ordering {
    use Value::*;
    let rank = |value: &Value| match value {
        Empty => 0,
        Bool(_) => 1,
        Number(_) => 2,
        Money { .. } => 3,
        Date(_) | Time(_) => 4,
        Duration { .. } => 5,
        Text(_) => 6,
        List(_) => 7,
        Map(_) => 8,
    };
    if rank(a) != rank(b) {
        return rank(a).cmp(&rank(b));
    }
    let instant = |value: &Value| match value {
        Date(date) => crate::dates::day(date).map(|day| day * 86400),
        Time(time) => crate::dates::time(time).map(|(seconds, offset)| seconds - i64::from(offset)),
        _ => None,
    };
    match (a, b) {
        (Bool(a), Bool(b)) => a.cmp(b),
        (Number(a), Number(b)) => a.cmp(b),
        (
            Money {
                amount: a,
                currency: ac,
            },
            Money {
                amount: b,
                currency: bc,
            },
        ) => ac.cmp(bc).then_with(|| a.cmp(b)),
        (Date(_) | Time(_), Date(_) | Time(_)) => instant(a).cmp(&instant(b)),
        (
            Duration {
                months: a,
                seconds: x,
            },
            Duration {
                months: b,
                seconds: y,
            },
        ) => (a, x).cmp(&(b, y)),
        (Text(a), Text(b)) => a.cmp(b),
        _ => Ordering::Equal,
    }
}

struct Matched {
    index: usize,
    sort: Vec<Value>,
    totals: Vec<Value>,
    group: Option<Value>,
    computed: Fields,
}

pub(crate) fn computed_values(
    mut values: Fields,
    metadata: &Fields,
    schema: &BTreeMap<String, Field>,
    order: &[String],
    context: &EvalContext<'_>,
    steps: &mut usize,
) -> Result<Fields, Diagnostic> {
    for key in order {
        let value = evaluate_field(
            &schema[key],
            Row {
                fields: &values,
                metadata,
            },
            context,
            steps,
            false,
        );
        let value = match value {
            Err(error) if schema[key].period.is_some() && is_list_limit(&error) => Value::Empty,
            result => result?,
        };
        values.insert(key.clone(), value);
    }
    Ok(values)
}
