use std::collections::BTreeMap;

mod extension;
mod parser;
pub(crate) use extension::plan_extension;
use extension::{dispatch, set_values};
use parser::parse_operation;
pub(crate) use parser::{assignment, parse};

use crate::{
    Diagnostic, EvalContext, Expression, Field, FieldType, Fields, Item, Kind, Row, Value,
    evaluate,
    expr::{Node, NodeKind, Op},
    field,
    lines::Site,
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

/// Evaluate all of a kind's operation preconditions for one authorized row.
/// Uses precomputed row fields and one shared expression budget. An evaluation
/// failure leaves its operation unavailable for this row.
pub fn operation_conditions(
    kind: &Kind,
    row: Row<'_>,
    computed: &Fields,
    context: &EvalContext<'_>,
) -> Result<BTreeMap<String, bool>, Diagnostic> {
    let mut fields = typed_fields(&kind.fields, row.fields, |_| kind.site, true)?;
    fields.extend(
        computed
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    let row = Row {
        fields: &fields,
        metadata: row.metadata,
    };
    let mut steps = 50_000;
    Ok(kind
        .operations
        .iter()
        .map(|(name, operation)| {
            let available = operation.condition.as_ref().is_none_or(|condition| {
                crate::expr::eval::evaluate_in(condition, row, context, &mut steps)
                    .is_ok_and(|value| value.truth())
            });
            (name.clone(), available)
        })
        .collect())
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

    pub(crate) fn refusal_input<'a>(&'a self, kind: &'a Kind, name: &str) -> Option<&'a FieldType> {
        let schema = match &self.form {
            OperationForm::Code => return self.inputs.get(name)?.field_type.as_ref(),
            OperationForm::Add { .. } => &kind.fields,
            OperationForm::Change(only)
                if only
                    .as_ref()
                    .is_none_or(|fields| fields.iter().any(|field| field == name)) =>
            {
                &kind.fields
            }
            OperationForm::AppendRecord(fields) | OperationForm::ChangeRecord { fields, .. }
                if fields.iter().any(|field| field == name) =>
            {
                &kind.records
            }
            OperationForm::Set(assignments)
                if assignments.iter().any(|assignment| {
                    assignment.target == name && assignment.value.is_none() && !assignment.state
                }) =>
            {
                &kind.fields
            }
            _ => return None,
        };
        target_field(schema, name, self.site)
            .ok()
            .map(|ty| ty.unwrap_or(&FieldType::LongText))
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

impl Kind {
    /// Default add, change and remove actions absent from an outside-editable
    /// kind's explicit operation list. These reuse the format's operation parser.
    pub fn implicit_operations(&self) -> impl Iterator<Item = Operation> + '_ {
        ["add", "change", "remove"]
            .into_iter()
            .filter(|name| !self.operations.contains_key(*name))
            .filter_map(|name| implicit_operation(self, name))
    }
}

pub(crate) fn implicit_operation(kind: &Kind, name: &str) -> Option<Operation> {
    if kind.editable == crate::Editability::Outside && matches!(name, "add" | "change" | "remove") {
        return parse_operation(name, "", kind.site).ok();
    }
    None
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
    if kind.checks.is_empty() {
        return Ok(());
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
        None => {
            fallback = implicit_operation(kind, operation).ok_or_else(|| {
                kind.site
                    .error(format!("The kind does not declare {operation}"))
            })?;
            &fallback
        }
    };
    let site = declared.site;
    let empty = Fields::new();
    let row = input.item.unwrap_or(Row {
        fields: &empty,
        metadata: &empty,
    });
    let stored = typed_fields(&kind.fields, row.fields, |_| site, true)?;
    let computed = if declared.condition.is_some()
        || matches!(
            declared.form,
            OperationForm::Set(_) | OperationForm::Open(_) | OperationForm::Timer { .. }
        ) {
        Some(crate::query::computed_values(
            stored.clone(),
            row.metadata,
            &kind.fields,
            &kind.computed,
            context,
            &mut 50_000,
        )?)
    } else {
        None
    };
    let view = Row {
        fields: computed.as_ref().unwrap_or(&stored),
        metadata: row.metadata,
    };
    if let Some(condition) = &declared.condition
        && !evaluate(condition, view, context)?.truth()
    {
        return Err(condition
            .root
            .site
            .error("The operation's condition does not hold"));
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
            let (fields, state) = set_values(
                assignments,
                &kind.fields,
                &kind.state,
                view,
                input.fields,
                context,
            )?;
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
            vec![dispatch(declared, view, input.fields, context)?]
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
        if !fields.contains_key(name)
            && let Some(default) = &field.default
        {
            fields.insert(name.clone(), evaluate(default, row, context)?);
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
