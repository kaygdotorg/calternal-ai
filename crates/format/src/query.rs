use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    Collection, Diagnostic, EvalContext, Field, Fields, Kind, Row, Value,
    expr::eval::{aggregate, compare, evaluate_in},
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
}

/// One row in the result, backreferenced to the host input.
#[derive(Clone, Debug)]
pub struct QueryRow {
    /// Index into the supplied rows.
    pub index: usize,
    /// Computed columns for this row.
    pub computed: Fields,
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
    let mut steps = 1_000_000_usize;
    let mut matches = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        steps = steps
            .checked_sub(1)
            .ok_or_else(|| collection.site.error("The collection query is too costly"))?;
        let mut row_context = *context;
        if let Some(per_row) = context.rows.and_then(|rows| match row.metadata.get("id") {
            Some(Value::Text(id)) => rows.get(id),
            _ => None,
        }) {
            row_context.state = per_row.state;
            row_context.records = per_row.records;
            row_context.reverse = per_row.reverse;
        }
        let stored = typed_fields(&kind.fields, row.fields, |_| kind.site, true)?;
        let fields = computed_values(
            stored,
            row.metadata,
            &kind.fields,
            &kind.computed,
            &row_context,
            &mut steps,
        )?;
        let fields = computed_values(
            fields,
            row.metadata,
            &collection.fields,
            &collection.computed,
            &row_context,
            &mut steps,
        )?;
        let row = Row {
            fields: &fields,
            metadata: row.metadata,
        };
        if !collection.ignoring_default {
            if let Some(condition) = &kind.default_where {
                if !evaluate_in(condition, row, &row_context, &mut steps)?.truth() {
                    continue;
                }
            }
        }
        if let Some(condition) = &collection.condition {
            if !evaluate_in(condition, row, &row_context, &mut steps)?.truth() {
                continue;
            }
        }
        let group = collection
            .group
            .as_ref()
            .map(|group| evaluate_in(group, row, &row_context, &mut steps))
            .transpose()?;
        matches.push(Matched {
            index,
            sort: collection
                .sort
                .iter()
                .map(|sort| row.value(&sort.field))
                .collect(),
            totals: collection
                .totals
                .iter()
                .map(|total| {
                    total
                        .field
                        .as_ref()
                        .map_or(Value::Bool(true), |field| row.value(field))
                })
                .collect(),
            group,
            computed: kind
                .computed
                .iter()
                .chain(&collection.computed)
                .filter_map(|field| {
                    fields
                        .get(field)
                        .map(|value| (field.clone(), value.clone()))
                })
                .collect(),
        });
    }
    let mut error = None;
    matches.sort_by(|a, b| {
        for (at, column) in collection.sort.iter().enumerate() {
            if column.field == "manual" {
                continue;
            }
            let order = match (&a.sort[at], &b.sort[at]) {
                (Value::Empty, Value::Empty) => Ordering::Equal,
                (Value::Empty, _) => return Ordering::Greater,
                (_, Value::Empty) => return Ordering::Less,
                (a, b) => match compare(a, b, collection.site) {
                    Ok(order) => order.unwrap_or(Ordering::Equal),
                    Err(diagnostic) => {
                        error = Some(diagnostic);
                        Ordering::Equal
                    }
                },
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
    if let Some(error) = error {
        return Err(error);
    }
    let aggregate_rows = |items: &[&Matched]| -> Result<Vec<Value>, Diagnostic> {
        collection
            .totals
            .iter()
            .enumerate()
            .map(|(index, total)| {
                let values = items
                    .iter()
                    .map(|row| row.totals[index].clone())
                    .filter(|value| *value != Value::Empty)
                    .collect();
                aggregate(&total.function, values, collection.site)
            })
            .collect()
    };
    let totals = aggregate_rows(&matches.iter().collect::<Vec<_>>())?;
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
            steps = steps
                .checked_sub(groups.len() + 1)
                .ok_or_else(|| collection.site.error("The collection has too many groups"))?;
            if let Some((_, members)) = groups.iter_mut().find(|(group_key, _)| group_key == key) {
                members.push(row);
            } else {
                groups.push((key.clone(), vec![row]));
            }
        }
    }
    let groups = groups
        .into_iter()
        .map(|(key, members)| {
            Ok(QueryGroup {
                key,
                matched: members.len(),
                totals: aggregate_rows(&members)?,
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
        })
        .collect();
    Ok(QueryResult {
        rows,
        matched,
        groups,
        totals,
    })
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
        let expression = schema[key].expression.as_ref().unwrap();
        let value = evaluate_in(
            expression,
            Row {
                fields: &values,
                metadata,
            },
            context,
            steps,
        )?;
        values.insert(key.clone(), value);
    }
    Ok(values)
}
