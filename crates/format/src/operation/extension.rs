//! Shared assignment and dispatch preparation for extension operations.

use super::{
    Assignment, Change, Diagnostic, EvalContext, Field, FieldType, Fields, Operation,
    OperationForm, Row, checked_input, defaults, evaluate, field, target_field,
};
use std::collections::BTreeMap;

pub(super) fn set_values(
    assignments: &[Assignment],
    fields_schema: &BTreeMap<String, Field>,
    state_schema: &BTreeMap<String, Field>,
    row: Row<'_>,
    input: &Fields,
    context: &EvalContext<'_>,
) -> Result<(Fields, Fields), Diagnostic> {
    let mut fields = Fields::new();
    let mut state = Fields::new();
    for assign in assignments {
        let value = match &assign.value {
            Some(expression) => evaluate(expression, row, context)?,
            None => input
                .get(&assign.target)
                .cloned()
                .ok_or_else(|| assign.site.error(format!("Supply {}", assign.target)))?,
        };
        let ty = target_field(
            if assign.state {
                state_schema
            } else {
                fields_schema
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
    Ok((fields, state))
}

pub(super) fn dispatch(
    declared: &Operation,
    row: Row<'_>,
    input: &Fields,
    context: &EvalContext<'_>,
) -> Result<Change, Diagnostic> {
    let mut fields = if declared.inputs.is_empty() {
        input.clone()
    } else {
        checked_input(&declared.inputs, input, None, declared.site)?
    };
    defaults(&declared.inputs, &mut fields, row, context)?;
    Ok(Change::Dispatch {
        operation: declared.name.clone(),
        fields,
    })
}

pub(crate) fn plan_extension(
    operation: &Operation,
    schema: &BTreeMap<String, Field>,
    plugin: &str,
    row: Row<'_>,
    input: &Fields,
    context: &EvalContext<'_>,
) -> Result<Vec<Change>, Diagnostic> {
    if let Some(condition) = &operation.condition
        && !evaluate(condition, row, context)?.truth()
    {
        return Err(operation
            .site
            .error("The extension operation's condition does not hold"));
    }
    let change = match &operation.form {
        OperationForm::Set(assignments) => {
            let (fields, _) =
                set_values(assignments, schema, &BTreeMap::new(), row, input, context)?;
            Change::Update(
                fields
                    .into_iter()
                    .map(|(name, value)| (format!("{plugin}.{name}"), value))
                    .collect(),
            )
        }
        OperationForm::Open(condition) => Change::Open {
            condition: condition.clone(),
            create: None,
        },
        OperationForm::Code => {
            if let Some(name) = input
                .keys()
                .find(|name| !operation.inputs.contains_key(*name))
            {
                return Err(operation
                    .site
                    .error(format!("The extension operation has no typed input {name}")));
            }
            dispatch(operation, row, input, context)?
        }
        _ => {
            return Err(operation
                .site
                .error("This operation form cannot extend a kind"));
        }
    };
    Ok(vec![change])
}
