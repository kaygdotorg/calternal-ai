use std::collections::{BTreeMap, BTreeSet};

use super::{Assignment, Operation, OperationForm};
use crate::{
    Diagnostic, dates,
    expr::parse as expression,
    field,
    lines::{self, Line, Site},
};

pub(crate) fn parse(
    lines: &[Line],
    depth: usize,
) -> Result<BTreeMap<String, Operation>, Diagnostic> {
    let mut operations = BTreeMap::new();
    let mut at = 0;
    while let Some(line) = lines.get(at) {
        if line.depth != depth {
            return Err(line
                .site
                .error("Indent an operation one step under operations"));
        }
        let (name, value, site) = match line.entry()? {
            Some(entry) => entry,
            None => (line.content()?, "", line.site),
        };
        let mut operation = parse_operation(name, value, site)?;
        at += 1;
        if let Some(input) = lines.get(at).filter(|line| line.depth > depth) {
            if !operation.is_coded()
                || input.depth != depth + 1
                || !input
                    .entry()?
                    .is_some_and(|(key, value, _)| key == "inputs" && value.is_empty())
            {
                return Err(input
                    .site
                    .error("Only coded operations may nest an inputs block"));
            }
            let header = at;
            at += 1;
            while lines.get(at).is_some_and(|line| line.depth > depth + 1) {
                at += 1;
            }
            if header + 1 == at {
                return Err(input.site.error("Declare at least one input field"));
            }
            for line in &lines[header + 1..at] {
                if line.depth != depth + 2 {
                    return Err(line.site.error("Indent an input once under inputs"));
                }
                let (name, value, site) = line
                    .entry()?
                    .ok_or_else(|| line.site.error("Declare a name and type for the input"))?;
                if name != "item" {
                    lines::field_name(name, line.site)?;
                }
                let field = field::declaration(value, site, line)?;
                if let Some(site) = field.badge {
                    return Err(site.error("A badge is a row field flag"));
                }
                if field.field_type.is_none()
                    || field.heading
                    || field.sidecar.is_some()
                    || field.from_metadata
                    || field.per_person
                {
                    return Err(site.error("An operation input must declare a stored type"));
                }
                if operation.inputs.insert(name.into(), field).is_some() {
                    return Err(site.error(format!("The operation input {name} is written twice")));
                }
            }
        }
        if operations.insert(name.into(), operation).is_some() {
            return Err(line
                .site
                .error(format!("The operation {name} is written twice")));
        }
    }
    for operation in operations.values() {
        if let OperationForm::Timer { after, .. } = &operation.form
            && !operations.contains_key(after)
        {
            return Err(operation
                .site
                .error(format!("Declare the timer operation {after}")));
        }
    }
    Ok(operations)
}

pub(super) fn parse_operation(
    name: &str,
    value: &str,
    site: Site,
) -> Result<Operation, Diagnostic> {
    let builtin = matches!(
        name,
        "add"
            | "change"
            | "remove"
            | "duplicate"
            | "append record"
            | "change record"
            | "remove record"
    );
    if !builtin {
        lines::operation_name(name, site)?
    }
    let mut condition = None;
    let mut placements = Vec::new();
    let mut within = None;
    let mut phrases = false;
    let parts = lines::parts(value, site)?;
    let mut terms = Vec::new();
    for (part, site) in parts {
        match part {
            "swipe" | "menu" | "primary" => {
                if placements.iter().any(|p| p == part) {
                    return Err(site.error("List each operation placement once"));
                }
                placements.push(part.to_owned());
            }
            "phrases" if name == "add" && !phrases => phrases = true,
            _ if part.starts_with("when ") && condition.is_none() => {
                condition = Some(expression::parse(&part[5..], site.after("when "))?)
            }
            _ if part.starts_with("within ") && within.is_none() && name == "change record" => {
                let duration = dates::duration(&part[7..])
                    .filter(|_| !part[7..].starts_with('-'))
                    .ok_or_else(|| {
                        site.error("Use a nonnegative record change window such as 48h")
                    })?;
                within = Some(duration);
            }
            _ => terms.push((part, site)),
        }
    }
    let names = |strip_only| -> Result<Vec<String>, Diagnostic> {
        terms
            .iter()
            .enumerate()
            .map(|(index, (name, site))| {
                let name = if index == 0 && strip_only {
                    name.strip_prefix("only ")
                        .ok_or_else(|| site.error("Start a restricted change with only"))?
                } else {
                    name
                };
                lines::field_name(name, *site)?;
                Ok(name.into())
            })
            .collect()
    };
    let code = terms.len() == 1 && terms[0].0 == "code";
    let form = match name {
        "add" | "change" | "remove" | "duplicate" if code => OperationForm::Code,
        "add" => OperationForm::Add {
            fields: names(false)?,
            phrases,
        },
        "change" => OperationForm::Change(if terms.is_empty() {
            None
        } else {
            Some(names(true)?)
        }),
        "remove" if terms.is_empty() => OperationForm::Remove,
        "duplicate" if terms.is_empty() => OperationForm::Duplicate,
        "append record" => OperationForm::AppendRecord(names(false)?),
        "change record" => OperationForm::ChangeRecord {
            fields: names(false)?,
            within,
        },
        "remove record" if terms.is_empty() => OperationForm::RemoveRecord,
        _ if builtin => return Err(site.error("Unknown modifier for the built-in operation")),
        _ if code => OperationForm::Code,
        _ if terms
            .first()
            .is_some_and(|(value, _)| value.starts_with("set ")) =>
        {
            let assignments = terms
                .into_iter()
                .map(|(value, site)| assignment(value, site))
                .collect::<Result<Vec<_>, _>>()?;
            let mut targets = BTreeSet::new();
            if assignments
                .iter()
                .any(|assign| !targets.insert((assign.state, &assign.target)))
            {
                return Err(site.error("Set each target only once in an operation"));
            }
            OperationForm::Set(assignments)
        }
        _ if terms.len() == 1 && terms[0].0.starts_with("open where ") => OperationForm::Open(
            expression::parse(&terms[0].0[11..], terms[0].1.after("open where "))?,
        ),
        _ if terms.len() == 2
            && terms[0].0.starts_with("timer ")
            && terms[1].0.starts_with("after ") =>
        {
            let delay = expression::parse(&terms[0].0[6..], terms[0].1.after("timer "))?;
            let after = terms[1].0[6..].to_owned();
            OperationForm::Timer { delay, after }
        }
        _ => return Err(site.error("Use set, open where, timer or code for a named operation")),
    };
    Ok(Operation {
        name: name.into(),
        form,
        condition,
        placements,
        inputs: BTreeMap::new(),
        site,
    })
}

pub(crate) fn assignment(value: &str, site: Site) -> Result<Assignment, Diagnostic> {
    let (target, formula) = value
        .strip_prefix("set ")
        .and_then(|value| value.split_once(" = "))
        .ok_or_else(|| site.error("Write a set line as set <field> = <value>"))?;
    let (state, name) = target
        .strip_prefix("state.")
        .map_or((false, target), |name| (true, name));
    let name = name
        .strip_prefix('[')
        .and_then(|name| name.strip_suffix(']'))
        .unwrap_or(name);
    let site = site.after("set ");
    lines::field_name(name, site)?;
    let value = if formula == "ask" {
        None
    } else {
        Some(expression::parse(formula, site.after(target).after(" = "))?)
    };
    Ok(Assignment {
        target: name.into(),
        state,
        value,
        site,
    })
}
