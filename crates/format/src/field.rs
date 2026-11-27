use crate::{
    Decimal, Diagnostic, Expression, Value, dates,
    expr::{functions, parse},
    lines::{self, Line, Site},
};
use std::collections::BTreeMap;

/// A stored or computed field declaration, compiled once at install.
#[derive(Clone, Debug)]
pub struct Field {
    /// Declared stored type. Computed fields derive a value from their expression.
    pub field_type: Option<FieldType>,
    /// Formula evaluated for the item or record.
    pub expression: Option<Expression>,
    /// Whether the field must have a value when the item is saved.
    pub required: bool,
    /// The expression used only when adding an item.
    pub default: Option<Expression>,
    /// Whether the field tracks the first heading of the body.
    pub heading: bool,
    /// Whether a true computed value is drawn as a mark.
    pub mark: bool,
    /// Previous names for host-managed update checks.
    pub previous_names: Vec<String>,
    /// Embedded metadata supplies the field.
    pub from_metadata: bool,
    /// A sidecar supplies the field. An empty name uses the declared field name.
    pub sidecar: Option<String>,
    /// Whether this setting has a per-person value.
    pub per_person: bool,
    pub(crate) badge: Option<Site>,
    pub(crate) viewer_dependent: bool,
    pub(crate) site: Site,
}

impl Field {
    /// Whether a numeric field is marked for badge presentation in rows.
    pub fn is_badge(&self) -> bool {
        self.badge.is_some()
    }

    /// Whether a compiled value must be evaluated in the viewer's context.
    /// Such a value must not be published in a shared index.
    pub fn is_per_viewer(&self) -> bool {
        self.viewer_dependent
    }
}

/// The type checked when a value enters a stored field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldType {
    /// One line of text.
    Text,
    /// Text that can span multiple lines.
    LongText,
    /// Exact decimal, with the storage unit first if units are declared.
    Number(Vec<String>),
    /// A currency amount with an optional fixed currency.
    Money(Option<String>),
    /// A yes/no value.
    Bool,
    /// A civil date.
    Date,
    /// A civil time with an optional offset.
    Time,
    /// A civil date or time.
    When,
    /// A duration in calendar months and elapsed seconds.
    Duration,
    /// One text value in declaration order.
    Choice(Vec<String>),
    /// An absolute HTTP, HTTPS or mailto address.
    Url,
    /// An item reference, optionally to a record in that kind.
    Link(String),
    /// A person identifier supplied by the host.
    Person,
    /// A recurrence rule.
    Repeat,
    /// Latitude, longitude and name.
    Place,
    /// A stored-file reference.
    File,
    /// A six-digit RGB colour.
    Colour,
    /// Structured plugin-owned data.
    Data,
    /// Ordered values of a single type.
    List(Box<FieldType>),
    /// An ordered list of small structured records.
    Records(BTreeMap<String, FieldType>),
    /// A record anchor in host ledger order.
    Anchor,
}

pub(crate) fn declaration(value: &str, site: Site, line: &Line) -> Result<Field, Diagnostic> {
    let parts = line.parts(value)?;
    declaration_parts(&parts, site, |formula, _| {
        let start = formula.as_ptr() as usize - line.text.as_ptr() as usize;
        parse::parse_at(formula, |offset| line.site_at(start + offset))
    })
}

#[cfg(test)]
pub(crate) fn test_declaration(value: &str, site: Site) -> Result<Field, Diagnostic> {
    declaration_parts(&lines::parts(value, site)?, site, parse::parse)
}

fn declaration_parts(
    parts: &[(&str, Site)],
    site: Site,
    parse_formula: impl FnOnce(&str, Site) -> Result<Expression, Diagnostic>,
) -> Result<Field, Diagnostic> {
    let Some(&(first, first_site)) = parts.first() else {
        return Err(site.error("Declare a field type or formula"));
    };
    let (field_type, expression) = if let Some(formula) = first.strip_prefix("= ") {
        (None, Some(parse_formula(formula, first_site.after("= "))?))
    } else {
        (Some(field_type(first, first_site)?), None)
    };
    let mut field = Field {
        field_type,
        expression,
        required: false,
        default: None,
        heading: false,
        mark: false,
        badge: None,
        previous_names: Vec::new(),
        from_metadata: false,
        sidecar: None,
        per_person: false,
        viewer_dependent: false,
        site,
    };
    for &(flag, site) in &parts[1..] {
        match (flag, &field.field_type) {
            ("mark", None) if !field.mark && !field.is_badge() => field.mark = true,
            ("badge", None | Some(FieldType::Number(_))) if !field.mark && !field.is_badge() => {
                field.badge = Some(site);
            }
            ("required", Some(_)) if !field.required => field.required = true,
            ("heading", Some(FieldType::Text)) if !field.heading => field.heading = true,
            ("from metadata", Some(_)) if !field.from_metadata => field.from_metadata = true,
            ("in sidecar", Some(_)) if field.sidecar.is_none() => {
                field.sidecar = Some(String::new())
            }
            ("per person", Some(_)) if !field.per_person => field.per_person = true,
            ("per space", Some(_)) => {}
            (_, Some(kind)) if flag.starts_with("default ") && field.default.is_none() => {
                let text = &flag[8..];
                let position = site.after("default ");
                field.default = Some(match (text, kind) {
                    ("today", FieldType::Date | FieldType::When) => {
                        parse::parse("today()", position)?
                    }
                    ("now", FieldType::Time | FieldType::When) => parse::parse("now()", position)?,
                    _ => {
                        let text = lines::unquote(text, position)?;
                        let value = coerce(kind, &Value::Text(text), position)?;
                        Expression {
                            root: crate::expr::Node {
                                kind: crate::expr::NodeKind::Value(value),
                                site: position,
                            },
                        }
                    }
                });
            }
            (_, Some(_)) if flag.starts_with("was ") && field.previous_names.is_empty() => {
                for name in flag[4..].split('|') {
                    lines::field_name(name, site)?;
                    field.previous_names.push(name.into());
                }
            }
            (_, Some(_)) if flag.starts_with("in sidecar ") && field.sidecar.is_none() => {
                field.sidecar = Some(flag[11..].into())
            }
            _ => {
                return Err(site.error(format!("Unknown, repeated or misplaced field flag {flag}")));
            }
        }
    }
    Ok(field)
}

fn field_type(text: &str, site: Site) -> Result<FieldType, Diagnostic> {
    use FieldType as T;
    let list = |sub| T::List(Box::new(sub));
    let kind = match text {
        "text" => T::Text,
        "long text" => T::LongText,
        "number" => T::Number(Vec::new()),
        "money" => T::Money(None),
        "yes/no" => T::Bool,
        "date" => T::Date,
        "dates" => list(T::Date),
        "time" => T::Time,
        "when" => T::When,
        "duration" => T::Duration,
        "durations" => list(T::Duration),
        "tags" => list(T::Text),
        "url" => T::Url,
        "person" => T::Person,
        "people" => list(T::Person),
        "repeat" => T::Repeat,
        "place" => T::Place,
        "file" => T::File,
        "colour" => T::Colour,
        "data" => T::Data,
        "record anchor" => T::Anchor,
        _ if text.starts_with("number ") => {
            let units: Vec<_> = text[7..].split('|').map(str::to_owned).collect();
            if units.len() > 2
                || units
                    .iter()
                    .any(|unit| unit.is_empty() || unit.contains(' '))
            {
                return Err(site.error("Declare one or two units separated by |"));
            }
            if units.len() == 2 && unit_ratio(&units[0], &units[1]).is_none() {
                return Err(site.error("The engine cannot convert between these units"));
            }
            T::Number(units)
        }
        _ if text.starts_with("money ") && currency(&text[6..]) => T::Money(Some(text[6..].into())),
        _ if group(text, "choice").is_some() => {
            let values: Vec<_> = group(text, "choice")
                .unwrap()
                .split('|')
                .map(|value| lines::unquote(value, site))
                .collect::<Result<_, _>>()?;
            let mut seen = std::collections::BTreeSet::new();
            if values
                .iter()
                .any(|value| value.is_empty() || !seen.insert(value))
            {
                return Err(site.error("List distinct nonempty choice values"));
            }
            T::Choice(values)
        }
        _ if group(text, "link").is_some() || group(text, "links").is_some() => {
            let (target, many) = if let Some(target) = group(text, "link") {
                (target, false)
            } else {
                (group(text, "links").unwrap(), true)
            };
            let kind = target.strip_suffix(" records").unwrap_or(target);
            for id in kind.split('.') {
                lines::id(id, site)?
            }
            if kind.matches('.').count() > 1 {
                return Err(site.error("Name a kind or a plugin and kind"));
            }
            let link = T::Link(target.into());
            if many { list(link) } else { link }
        }
        _ if group(text, "records").is_some() => {
            let mut fields = BTreeMap::new();
            for (part, position) in
                lines::parts(group(text, "records").unwrap(), site.after("records("))?
            {
                let (name, kind) = part
                    .split_once(": ")
                    .ok_or_else(|| position.error("Write a name and type for the record field"))?;
                lines::field_name(name, position)?;
                if kind.starts_with("records(") {
                    return Err(position.error("Use a scalar or list field in a small record"));
                }
                if fields
                    .insert(
                        name.into(),
                        field_type(kind, position.after(name).after(": "))?,
                    )
                    .is_some()
                {
                    return Err(position.error(format!("Record field {name} is written twice")));
                }
            }
            T::Records(fields)
        }
        _ => return Err(site.error(format!("Unknown field type {text}"))),
    };
    Ok(kind)
}

fn group<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.strip_prefix(name)?
        .strip_prefix('(')?
        .strip_suffix(')')
}

fn currency(text: &str) -> bool {
    text.len() == 3 && text.bytes().all(|byte| byte.is_ascii_uppercase())
}

pub(crate) fn coerce(kind: &FieldType, value: &Value, site: Site) -> Result<Value, Diagnostic> {
    use FieldType as T;
    if value == &Value::Empty {
        return Ok(Value::Empty);
    }
    let text = match value {
        Value::Text(s) | Value::Date(s) | Value::Time(s) => Some(s.as_str()),
        _ => None,
    };
    let invalid = || site.error("The value does not match the declared field type");
    let result = match kind {
        T::Text | T::LongText => {
            let content = text
                .map(str::to_owned)
                .or_else(|| match value {
                    Value::Number(n) => Some(n.to_string()),
                    Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
                .ok_or_else(invalid)?;
            if kind == &T::Text && content.contains(['\r', '\n']) {
                return Err(invalid());
            }
            Value::Text(content)
        }
        T::Number(units) => match value {
            Value::Number(_) => value.clone(),
            _ => {
                let text = text.ok_or_else(invalid)?;
                let (amount, unit) = text.split_once(' ').unwrap_or((text, ""));
                let amount: Decimal = amount.parse().map_err(|_| invalid())?;
                if unit.is_empty() || units.first().is_some_and(|primary| primary == unit) {
                    Value::Number(amount)
                } else if let Some(primary) = units
                    .first()
                    .filter(|_| units.iter().any(|value| value == unit))
                {
                    let (factor, divisor) = unit_ratio(unit, primary).ok_or_else(invalid)?;
                    Value::Number(
                        amount
                            .mul(factor)
                            .and_then(|n| n.div(divisor))
                            .ok_or_else(invalid)?,
                    )
                } else {
                    return Err(invalid());
                }
            }
        },
        T::Money(fixed) => {
            let (amount, currency) = match value {
                Value::Money { amount, currency } => (*amount, currency.as_str()),
                Value::Number(number) => (*number, fixed.as_deref().ok_or_else(invalid)?),
                _ => {
                    let text = text.ok_or_else(invalid)?;
                    let (number, currency) = text
                        .split_once(' ')
                        .or_else(|| fixed.as_deref().map(|currency| (text, currency)))
                        .ok_or_else(invalid)?;
                    (number.parse().map_err(|_| invalid())?, currency)
                }
            };
            if !self::currency(currency) || fixed.as_ref().is_some_and(|fixed| fixed != currency) {
                return Err(invalid());
            }
            Value::Money {
                amount,
                currency: currency.into(),
            }
        }
        T::Bool => match value {
            Value::Bool(_) => value.clone(),
            Value::Text(text) if matches!(text.as_str(), "true" | "yes" | "false" | "no") => {
                Value::Bool(matches!(text.as_str(), "true" | "yes"))
            }
            _ => return Err(invalid()),
        },
        T::Date | T::When if text.is_some_and(|text| dates::day(text).is_some()) => {
            Value::Date(text.unwrap().into())
        }
        T::Time | T::When if text.is_some_and(|text| dates::time(text).is_some()) => {
            Value::Time(text.unwrap().into())
        }
        T::Duration => match value {
            Value::Duration { .. } => value.clone(),
            _ => text
                .and_then(dates::duration)
                .ok_or_else(|| site.error("Use a duration such as 2min, 2h or 2mo"))?,
        },
        T::Choice(choices)
            if text.is_some_and(|text| choices.iter().any(|choice| choice == text)) =>
        {
            value.clone()
        }
        T::Url if text.is_some_and(valid_url) => value.clone(),
        T::Link(target)
            if text.is_some_and(|text| {
                !text.is_empty() && (!target.ends_with(" records") || text.contains("#^"))
            }) =>
        {
            value.clone()
        }
        T::File | T::Person if text.is_some_and(|text| !text.is_empty()) => value.clone(),
        T::Anchor if text.is_some_and(anchor) => value.clone(),
        T::Repeat if text.is_some_and(|text| functions::repeat(text).is_some()) => value.clone(),
        T::Colour
            if text.is_some_and(|text| {
                text.len() == 7
                    && text.starts_with('#')
                    && text[1..].bytes().all(|c| c.is_ascii_hexdigit())
            }) =>
        {
            value.clone()
        }
        T::Data if matches!(value, Value::Map(_)) => value.clone(),
        T::Place if matches!(value, Value::Map(map) if valid_place(map)) => value.clone(),
        T::List(sub) => {
            let values = match value {
                Value::List(values) => values.clone(),
                Value::Text(text) if **sub == T::Text => text
                    .split(',')
                    .map(|tag| Value::Text(tag.trim().trim_start_matches('#').into()))
                    .collect(),
                _ => return Err(invalid()),
            };
            Value::List(
                values
                    .iter()
                    .map(|value| coerce(sub, value, site))
                    .collect::<Result<_, _>>()?,
            )
        }
        T::Records(schema) => {
            let Value::List(records) = value else {
                return Err(invalid());
            };
            let mut result = Vec::new();
            for record in records {
                let Value::Map(fields) = record else {
                    return Err(invalid());
                };
                let mut fields = fields.clone();
                for (name, ty) in schema {
                    if let Some(value) = fields.get_mut(name) {
                        *value = coerce(ty, value, site)?;
                    }
                }
                result.push(Value::Map(fields));
            }
            Value::List(result)
        }
        _ => return Err(invalid()),
    };
    Ok(result)
}

pub(crate) fn anchor(text: &str) -> bool {
    text.len() == 16
        && text
            .to_ascii_lowercase()
            .bytes()
            .all(|c| b"0123456789abcdefghjkmnpqrstvwxyz".contains(&c))
}

pub(crate) fn valid_url(text: &str) -> bool {
    !text.contains(|ch: char| ch.is_control() || ch.is_whitespace() || ch == '\\')
        && (functions::domain(text).is_some()
            || text.split_once(':').is_some_and(|(scheme, rest)| {
                scheme.eq_ignore_ascii_case("mailto") && !rest.is_empty()
            }))
}

fn valid_place(map: &crate::Fields) -> bool {
    matches!(map.get("lat"), Some(Value::Number(n)) if *n >= Decimal::integer(-90) && *n <= Decimal::integer(90))
        && matches!(map.get("lon"), Some(Value::Number(n)) if *n >= Decimal::integer(-180) && *n <= Decimal::integer(180))
        && matches!(map.get("name"), Some(Value::Text(_)))
}

fn unit_ratio(from: &str, to: &str) -> Option<(Decimal, Decimal)> {
    let ratio = match (from, to) {
        (from, to) if from == to => ("1", "1"),
        ("lb", "kg") => ("0.45359237", "1"),
        ("kg", "lb") => ("1", "0.45359237"),
        ("mi", "km") => ("1.609344", "1"),
        ("km", "mi") => ("1", "1.609344"),
        ("in", "cm") => ("2.54", "1"),
        ("cm", "in") => ("1", "2.54"),
        ("ft", "m") => ("0.3048", "1"),
        ("m", "ft") => ("1", "0.3048"),
        _ => return None,
    };
    Some((ratio.0.parse().ok()?, ratio.1.parse().ok()?))
}
