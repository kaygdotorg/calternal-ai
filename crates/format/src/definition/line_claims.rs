//! Compile line forms and checkbox field mappings.

use std::collections::{BTreeMap, BTreeSet};

use super::{Kind, Section, list};
use crate::{Diagnostic, FieldType, Value, field::coerce, lines};

/// The two note line forms understood by the engine.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LineForm {
    /// Markdown list items with a checkbox.
    Checkbox,
    /// Civil times and all-day entries.
    Time,
}

/// A kind's claim to one notation in the declared source kinds.
#[derive(Clone, Debug)]
pub struct LineClaim {
    /// The claimed notation.
    pub form: LineForm,
    /// Local kind IDs or qualified plugin and kind IDs.
    pub targets: Vec<String>,
    /// A checkbox field mapping, when the kind claims checkbox lines.
    pub checkbox: Option<CheckboxMapping>,
}

impl LineClaim {
    /// Project a scanned time line into the kind's fields, using the source
    /// note's civil date. An end clock before its start falls on the next day.
    pub fn time_fields(
        &self,
        item: &crate::LineItem<'_>,
        date: &str,
    ) -> Result<crate::Fields, Diagnostic> {
        let site = lines::Site::START;
        let crate::LineItem::Time { start, end, title } = item else {
            return Err(site.error("Select a scanned time line"));
        };
        let day = crate::dates::day(date)
            .filter(|_| self.form == LineForm::Time)
            .ok_or_else(|| {
                site.error("Time lines require a time claim and the note's valid civil date")
            })?;
        let start_value = start.map_or_else(
            || Value::Date(date.into()),
            |clock| Value::Time(format!("{date}T{clock}:00")),
        );
        let mut fields = crate::Fields::from([
            ("start".into(), start_value),
            ("all day".into(), Value::Bool(start.is_none())),
            ("title".into(), Value::Text((*title).into())),
        ]);
        if let Some(end) = end {
            let end_day = day + i64::from(start.is_some_and(|start| *end < start));
            let end_date = crate::dates::render_day(end_day)
                .ok_or_else(|| site.error("The end clock crosses the supported date range"))?;
            fields.insert("end".into(), Value::Time(format!("{end_date}T{end}:00")));
        }
        Ok(fields)
    }
}

/// How a linked item's stored field mirrors its checkbox.
#[derive(Clone, Debug)]
pub struct CheckboxMapping {
    /// The field updated through the linked item's ordinary change operation.
    pub field: String,
    /// Checked values keyed by the character inside the box.
    pub values: BTreeMap<char, Value>,
}

impl CheckboxMapping {
    /// Look up a mark, treating uppercase X as the standard checked mark.
    pub fn value(&self, mark: char) -> Option<&Value> {
        self.values.get(&if mark == 'X' { 'x' } else { mark })
    }
}

pub(super) fn parse(entry: &Section<'_>, kind: &Kind) -> Result<LineClaim, Diagnostic> {
    let (form, rest) = entry.value.split_once(" in ").ok_or_else(|| {
        entry
            .site
            .error("Write lines: checkbox or time in followed by kinds")
    })?;
    let form = match form {
        "checkbox" => LineForm::Checkbox,
        "time" => LineForm::Time,
        _ => return Err(entry.site.error("Choose checkbox or time for line items")),
    };
    let (targets, mapping) = rest
        .split_once(", box ")
        .map_or((rest, None), |(a, b)| (a, Some(b)));
    let targets = list(targets, entry.site)?
        .into_iter()
        .map(|target| {
            target
                .strip_prefix('[')
                .and_then(|target| target.strip_suffix(']'))
                .map_or(target.clone(), str::to_owned)
        })
        .collect::<Vec<_>>();
    let mut unique = BTreeSet::new();
    for target in &targets {
        if let Some((plugin, kind)) = target.split_once('.') {
            lines::id(plugin, entry.site)?;
            lines::kind_id(kind, entry.site)?;
        } else {
            lines::kind_id(target, entry.site)?;
        }
        if !unique.insert(target) {
            return Err(entry.site.error("List each line source once"));
        }
    }
    if targets.is_empty() || kind.hidden {
        return Err(entry
            .site
            .error("Line items require visible kinds and at least one source"));
    }
    let checkbox = match form {
        LineForm::Checkbox => Some(checkbox(mapping, entry, kind)?),
        LineForm::Time => {
            if mapping.is_some() {
                return Err(entry.site.error("Only checkbox lines take a box mapping"));
            }
            for (name, allowed) in [
                ("title", &[FieldType::Text][..]),
                ("start", &[FieldType::When][..]),
                ("end", &[FieldType::When][..]),
                ("all day", &[FieldType::Bool][..]),
            ] {
                if kind
                    .fields
                    .get(name)
                    .and_then(|field| field.field_type.as_ref())
                    .is_none_or(|ty| !allowed.contains(ty))
                {
                    return Err(entry.site.error(format!(
                        "Time lines need a stored {name} field of the declared type"
                    )));
                }
            }
            None
        }
    };
    Ok(LineClaim {
        form,
        targets,
        checkbox,
    })
}

fn checkbox(
    mapping: Option<&str>,
    entry: &Section<'_>,
    kind: &Kind,
) -> Result<CheckboxMapping, Diagnostic> {
    let (field, marks) = match mapping {
        Some(mapping) => mapping.split_once(": ").ok_or_else(|| {
            entry
                .site
                .error("Write box <field>: followed by its mark mappings")
        })?,
        None => ("done", "`[ ]` no, `[x]` yes"),
    };
    let ty = kind
        .fields
        .get(field)
        .and_then(|field| field.field_type.as_ref())
        .ok_or_else(|| entry.site.error("Map the checkbox to a stored field"))?;
    if mapping.is_none() && ty != &FieldType::Bool {
        return Err(entry
            .site
            .error("An unmapped checkbox requires a yes/no done field"));
    }
    let mut values = BTreeMap::new();
    for (part, site) in lines::parts(marks, entry.site)? {
        let (mark, value) = part
            .strip_prefix("`[")
            .and_then(|text| text.split_once("]` "))
            .ok_or_else(|| {
                site.error("Write a checkbox mark in backticks followed by its value")
            })?;
        let mut chars = mark.chars();
        let mark = match (chars.next(), chars.next()) {
            (Some(mark @ (' ' | '/' | 'x' | '-')), None) => mark,
            _ => return Err(site.error("Map one of the four declared checkbox marks")),
        };
        let raw = Value::Text(lines::unquote(value, site)?);
        let value = coerce(ty, &raw, site)?;
        if values.insert(mark, value).is_some() {
            return Err(site.error("Map each checkbox mark once"));
        }
    }
    if !values.contains_key(&' ') || !values.contains_key(&'x') {
        return Err(entry
            .site
            .error("Map at least the unchecked and checked marks"));
    }
    Ok(CheckboxMapping {
        field: field.into(),
        values,
    })
}
