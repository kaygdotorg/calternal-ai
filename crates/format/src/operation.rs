use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, EvalContext, Expression, Field, FieldType, Fields, Item, Kind, Row, Value, dates,
    evaluate,
    expr::{Node, NodeKind, Op, parse},
    field,
    lines::{self, Line, Site},
};

/// One permitted gesture or operation on a kind.
#[derive(Clone, Debug)]
pub struct Operation {
    /// Operation name used across host surfaces.
    pub name: String,
    /// Execution form declared by the kind.
    pub form: OperationForm,
    /// Optional precondition over the current item and viewer context.
    pub condition: Option<Expression>,
    /// UI placement names such as menu, swipe and primary.
    pub placements: Vec<String>,
    /// Typed input declaration for coded operations and slot bindings.
    pub inputs: BTreeMap<String, Field>,
    pub(crate) site: Site,
}

/// The runtime behavior requested by a checked operation line.
#[derive(Clone, Debug)]
pub enum OperationForm {
    /// Create an item from form fields and declared defaults.
    Add {
        /// Fields exposed in the add form.
        fields: Vec<String>,
        /// Whether the composer may fill them from a phrase.
        phrases: bool,
    },
    /// Change an item's submitted fields, optionally limited to these names.
    Change(Option<Vec<String>>),
    /// Move the item to Trash through the host.
    Remove,
    /// Copy stored fields under a new ID.
    Duplicate,
    /// Set item or personal fields atomically.
    Set(Vec<Assignment>),
    /// Open a matching item or query.
    Open(Expression),
    /// Schedule a named operation after a delay.
    Timer {
        /// Formula that evaluates to a duration.
        delay: Expression,
        /// Name of the operation run once the timer fires.
        after: String,
    },
    /// Dispatch the named operation's declared code.
    Code,
    /// Append a new record with these form fields.
    AppendRecord(Vec<String>),
    /// Change a record within an optional author edit window.
    ChangeRecord {
        /// Fields exposed in the record editor.
        fields: Vec<String>,
        /// Optional maximum record age.
        within: Option<Value>,
    },
    /// Move a record to Trash through the host.
    RemoveRecord,
}

/// One target and value in a set operation.
#[derive(Clone, Debug)]
pub struct Assignment {
    /// Declared field or state key.
    pub target: String,
    /// Whether the target is viewer state.
    pub state: bool,
    /// Formula to evaluate, or no formula when the value comes from the form.
    pub value: Option<Expression>,
    site: Site,
}

impl Operation {
    /// Whether the operation requires the server's plugin process.
    pub fn is_coded(&self) -> bool {
        matches!(self.form, OperationForm::Code)
    }

    pub(crate) fn verify(
        &self,
        fields: &BTreeMap<String, Field>,
        state: &BTreeMap<String, Field>,
        records: &BTreeMap<String, Field>,
        unique: bool,
    ) -> Result<(), Diagnostic> {
        let names = match &self.form {
            OperationForm::Duplicate if unique => {
                return Err(self.site.error("A unique kind cannot declare duplicate"));
            }
            OperationForm::Add { fields: names, .. } | OperationForm::Change(Some(names)) => {
                Some((names, fields))
            }
            OperationForm::AppendRecord(names)
            | OperationForm::ChangeRecord { fields: names, .. } => Some((names, records)),
            OperationForm::Set(assignments) => {
                for assign in assignments {
                    target_field(
                        if assign.state { state } else { fields },
                        &assign.target,
                        assign.site,
                    )?;
                }
                None
            }
            _ => None,
        };
        if let Some((names, schema)) = names {
            for name in names {
                target_field(schema, name, self.site)?;
            }
        }
        Ok(())
    }
}

fn target_field<'a>(
    schema: &'a BTreeMap<String, Field>,
    name: &str,
    site: Site,
) -> Result<Option<&'a FieldType>, Diagnostic> {
    if name == "body" {
        return Ok(None);
    }
    match schema
        .get(name)
        .and_then(|field| field.field_type.as_ref().filter(|_| !field.from_metadata))
    {
        Some(FieldType::Data) => {
            Err(site.error(format!("Only declared plugin code may change {name}")))
        }
        Some(kind) => Ok(Some(kind)),
        None => Err(site.error(format!("Name a writable declared field instead of {name}"))),
    }
}

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
                lines::field_name(name, line.site)?;
                let field = field::declaration(value, site)?;
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
        if let OperationForm::Timer { after, .. } = &operation.form {
            if !operations.contains_key(after) {
                return Err(operation
                    .site
                    .error(format!("Declare the timer operation {after}")));
            }
        }
    }
    Ok(operations)
}

fn parse_operation(name: &str, value: &str, site: Site) -> Result<Operation, Diagnostic> {
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
                condition = Some(parse::parse(&part[5..], site.after("when "))?)
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
            parse::parse(&terms[0].0[11..], terms[0].1.after("open where "))?,
        ),
        _ if terms.len() == 2
            && terms[0].0.starts_with("timer ")
            && terms[1].0.starts_with("after ") =>
        {
            let delay = parse::parse(&terms[0].0[6..], terms[0].1.after("timer "))?;
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
        Some(parse::parse(formula, site.after(target).after(" = "))?)
    };
    Ok(Assignment {
        target: name.into(),
        state,
        value,
        site,
    })
}

/// In-memory inputs for an item or record operation.
#[derive(Clone, Copy, Debug)]
pub struct OperationInput<'a> {
    /// Existing item, absent on an add.
    pub item: Option<Row<'a>>,
    /// Values submitted by the caller with the operation.
    pub fields: &'a Fields,
    /// Authoritative record anchor for a change or removal.
    pub record: Option<&'a str>,
}

/// A requested change for the host to authorize and apply atomically.
#[derive(Clone, Debug)]
pub enum Change {
    /// Create a new item with these logical fields.
    Add(Fields),
    /// Overlay these fields on the existing item.
    Update(Fields),
    /// Move the existing item to Trash.
    Remove,
    /// Copy these fields and the source body's records under a new ID.
    Duplicate(Fields),
    /// Set these fields for the current viewer.
    State(Fields),
    /// Open matches and, when eligible, add these equality fields on no match.
    Open {
        /// Filter to apply to already-authorized candidate items.
        condition: Expression,
        /// Fields for creation if the condition nails down a unique key.
        create: Option<Fields>,
    },
    /// Schedule an operation with the host's receipt time.
    Timer {
        /// Delay to add to the receipt time.
        delay: Value,
        /// Declared destination operation.
        operation: String,
    },
    /// Append a record with a host-owned anchor and stamps.
    AppendRecord(Fields),
    /// Overlay fields on an existing record after authority and age checks.
    UpdateRecord {
        /// Host-supplied record anchor.
        anchor: String,
        /// Changes to the record fields or body.
        fields: Fields,
        /// Age ceiling for this form, to be enforced by the host.
        within: Option<Value>,
    },
    /// Move a record with this anchor to Trash.
    RemoveRecord(String),
    /// Dispatch code with the validated inputs in the same call.
    Dispatch {
        /// Declared operation name.
        operation: String,
        /// Submitted typed inputs.
        fields: Fields,
    },
}

/// Validate fields and checks against an item and host-supplied context.
pub fn check_item(
    kind: &Kind,
    item: &Item<'_>,
    context: &EvalContext<'_>,
) -> Result<(), Diagnostic> {
    let fields = typed_fields(&kind.fields, item.fields(), |name| item.site(name), true)?;
    let empty = Fields::new();
    check_values(
        kind,
        &fields,
        context.item.map_or(&empty, |row| row.metadata),
        context,
    )
}

pub(crate) fn typed_fields(
    schema: &BTreeMap<String, Field>,
    input: &Fields,
    site: impl Fn(&str) -> Site,
    keep_unknown: bool,
) -> Result<Fields, Diagnostic> {
    let mut fields = if keep_unknown {
        input.clone()
    } else {
        Fields::new()
    };
    for (name, decl) in schema {
        if let Some(value) = input.get(name) {
            if let Some(kind) = &decl.field_type {
                fields.insert(
                    name.clone(),
                    field::coerce(kind, value, site(name)).map_err(|mut error| {
                        error.message = format!("{name}: {}", error.message);
                        error
                    })?,
                );
            } else {
                fields.remove(name);
            }
        }
    }
    Ok(fields)
}

fn check_values(
    kind: &Kind,
    fields: &Fields,
    metadata: &Fields,
    context: &EvalContext<'_>,
) -> Result<(), Diagnostic> {
    for (name, field) in &kind.fields {
        if field.required && fields.get(name).is_none_or(|value| *value == Value::Empty) {
            return Err(field
                .site
                .error(format!("Fill in the required field {name}")));
        }
    }
    let values = crate::query::computed_values(
        fields.clone(),
        metadata,
        &kind.fields,
        &kind.computed,
        context,
        &mut 50_000,
    )?;
    for (condition, message) in &kind.checks {
        if !evaluate(
            condition,
            Row {
                fields: &values,
                metadata,
            },
            context,
        )?
        .truth()
        {
            return Err(condition.root.site.error(
                message
                    .as_deref()
                    .unwrap_or("The item does not pass its save condition"),
            ));
        }
    }
    Ok(())
}

/// Apply a declared operation as a pure function of the item, inputs and context.
pub fn apply_operation(
    kind: &Kind,
    operation: &str,
    input: &OperationInput<'_>,
    context: &EvalContext<'_>,
) -> Result<Vec<Change>, Diagnostic> {
    let fallback;
    let declared = match kind.operations.get(operation) {
        Some(operation) => operation,
        None if kind.editable == crate::Editability::Outside
            && matches!(operation, "add" | "change" | "remove") =>
        {
            fallback = parse_operation(operation, "", kind.site)?;
            &fallback
        }
        None => {
            return Err(kind
                .site
                .error(format!("The kind does not declare {operation}")));
        }
    };
    let site = declared.site;
    let empty = Fields::new();
    let row = input.item.unwrap_or(Row {
        fields: &empty,
        metadata: &empty,
    });
    let stored = typed_fields(&kind.fields, row.fields, |_| site, true)?;
    let computed = crate::query::computed_values(
        stored.clone(),
        row.metadata,
        &kind.fields,
        &kind.computed,
        context,
        &mut 50_000,
    )?;
    let view = Row {
        fields: &computed,
        metadata: row.metadata,
    };
    if let Some(condition) = &declared.condition {
        if !evaluate(condition, view, context)?.truth() {
            return Err(condition
                .root
                .site
                .error("The operation's condition does not hold"));
        }
    }

    let update = |patch: &Fields| -> Result<(), Diagnostic> {
        let mut updated = stored.clone();
        updated.extend(patch.clone());
        check_values(kind, &updated, row.metadata, context)
    };
    let anchor = || {
        input
            .record
            .filter(|anchor| field::anchor(anchor))
            .map(str::to_owned)
            .ok_or_else(|| site.error("Supply the selected record's host anchor"))
    };
    let changes = match &declared.form {
        OperationForm::Add { .. } => {
            let mut fields = checked_input(&kind.fields, input.fields, None, site)?;
            defaults(&kind.fields, &mut fields, view, context)?;
            check_values(kind, &fields, row.metadata, context)?;
            vec![Change::Add(fields)]
        }
        OperationForm::Change(only) => {
            let patch = checked_input(&kind.fields, input.fields, only.as_deref(), site)?;
            update(&patch)?;
            vec![Change::Update(patch)]
        }
        OperationForm::Remove => vec![Change::Remove],
        OperationForm::Duplicate => {
            if kind.unique.is_some() {
                return Err(site.error("A unique kind cannot be duplicated"));
            }
            let copy = stored
                .into_iter()
                .filter(|(key, _)| {
                    kind.fields
                        .get(key)
                        .and_then(|field| field.field_type.as_ref())
                        .is_some_and(|kind| kind != &FieldType::Data)
                })
                .collect();
            vec![Change::Duplicate(copy)]
        }
        OperationForm::Set(assignments) => {
            let mut fields = Fields::new();
            let mut state = Fields::new();
            for assign in assignments {
                let value =
                    match &assign.value {
                        Some(expression) => evaluate(expression, view, context)?,
                        None => input.fields.get(&assign.target).cloned().ok_or_else(|| {
                            assign.site.error(format!("Supply {}", assign.target))
                        })?,
                    };
                let ty = target_field(
                    if assign.state {
                        &kind.state
                    } else {
                        &kind.fields
                    },
                    &assign.target,
                    assign.site,
                )?;
                let value = field::coerce(ty.unwrap_or(&FieldType::LongText), &value, assign.site)?;
                let output = if assign.state {
                    &mut state
                } else {
                    &mut fields
                };
                output.insert(assign.target.clone(), value);
            }
            if !fields.is_empty() {
                update(&fields)?
            }
            let mut changes = Vec::new();
            if !fields.is_empty() {
                changes.push(Change::Update(fields))
            }
            if !state.is_empty() {
                changes.push(Change::State(state))
            }
            changes
        }
        OperationForm::Open(condition) => {
            let create = if let Some(unique) = &kind.unique {
                let mut fields = Fields::new();
                if equalities(&condition.root, &mut fields, view, context)?
                    && fields.contains_key(unique)
                {
                    Some(checked_input(&kind.fields, &fields, None, site)?)
                } else {
                    None
                }
            } else {
                None
            };
            vec![Change::Open {
                condition: condition.clone(),
                create,
            }]
        }
        OperationForm::Timer { delay, after } => {
            let delay = evaluate(delay, view, context)?;
            if !matches!(delay, Value::Duration { months, seconds } if months >= 0 && seconds >= 0)
            {
                return Err(site.error("The timer delay must be a nonnegative duration"));
            }
            vec![Change::Timer {
                delay,
                operation: after.clone(),
            }]
        }
        OperationForm::AppendRecord(names) => {
            let mut fields = checked_input(
                &kind.records,
                input.fields,
                (!names.is_empty()).then_some(names),
                site,
            )?;
            defaults(&kind.records, &mut fields, view, context)?;
            vec![Change::AppendRecord(fields)]
        }
        OperationForm::ChangeRecord { fields, within } => vec![Change::UpdateRecord {
            anchor: anchor()?,
            fields: checked_input(
                &kind.records,
                input.fields,
                (!fields.is_empty()).then_some(fields),
                site,
            )?,
            within: within.clone(),
        }],
        OperationForm::RemoveRecord => vec![Change::RemoveRecord(anchor()?)],
        OperationForm::Code => {
            let mut fields = if declared.inputs.is_empty() {
                input.fields.clone()
            } else {
                checked_input(&declared.inputs, input.fields, None, site)?
            };
            defaults(&declared.inputs, &mut fields, view, context)?;
            vec![Change::Dispatch {
                operation: operation.into(),
                fields,
            }]
        }
    };
    Ok(changes)
}

fn checked_input(
    schema: &BTreeMap<String, Field>,
    input: &Fields,
    allowed: Option<&[String]>,
    site: Site,
) -> Result<Fields, Diagnostic> {
    let mut result = Fields::new();
    for (name, value) in input {
        if name != "body" && allowed.is_some_and(|allowed| !allowed.contains(name)) {
            return Err(site.error(format!("This operation does not accept {name}")));
        }
        let field = target_field(schema, name, site)?;
        result.insert(
            name.clone(),
            crate::field::coerce(field.unwrap_or(&FieldType::LongText), value, site)?,
        );
    }
    Ok(result)
}

fn defaults(
    schema: &BTreeMap<String, Field>,
    fields: &mut Fields,
    row: Row<'_>,
    context: &EvalContext<'_>,
) -> Result<(), Diagnostic> {
    for (name, field) in schema {
        if !fields.contains_key(name) {
            if let Some(default) = &field.default {
                fields.insert(name.clone(), evaluate(default, row, context)?);
            }
        }
        if field.required && fields.get(name).is_none_or(|value| *value == Value::Empty) {
            return Err(field
                .site
                .error(format!("Fill in the required field {name}")));
        }
    }
    Ok(())
}

fn equalities(
    node: &Node,
    result: &mut Fields,
    row: Row<'_>,
    context: &EvalContext<'_>,
) -> Result<bool, Diagnostic> {
    match &node.kind {
        NodeKind::Binary(Op::And, left, right) => {
            Ok(equalities(left, result, row, context)? && equalities(right, result, row, context)?)
        }
        NodeKind::Binary(Op::Eq, left, right) => {
            let NodeKind::Name(name) = &left.kind else {
                return Ok(false);
            };
            let value = evaluate(
                &Expression {
                    root: *right.clone(),
                },
                row,
                context,
            )?;
            Ok(value != Value::Empty && result.insert(name.clone(), value).is_none())
        }
        _ => Ok(false),
    }
}
