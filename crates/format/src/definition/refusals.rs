//! Install checks and pure evaluation for enabled before-operation refusals.

use super::{InstallOptions, Kind, Manifest, Refusal, formulas, install};
use crate::{
    Diagnostic, EvalContext, Fields, Row, expr::eval::evaluate_in, field::coerce,
    operation::implicit_operation,
};

/// An authorized, ordinarily checked operation about to dispatch or write.
#[derive(Clone, Copy, Debug)]
pub struct RefusalCall<'a> {
    /// ID of the plugin that owns the called kind.
    pub owner: &'a str,
    /// The called kind's checked declaration.
    pub kind: &'a Kind,
    /// Name of the called operation on that kind.
    pub operation: &'a str,
    /// The target's authorized stored fields, absent on add.
    pub target: Option<&'a Fields>,
    /// The operation's checked input values.
    pub inputs: &'a Fields,
    /// The host verified that this removes the actor's own item or record.
    pub own_removal: bool,
}

/// The first enabled rule to refuse a call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefusalMatch<'a> {
    /// ID of the plugin that declared the rule.
    pub plugin_id: &'a str,
    /// Display name to present with the reason.
    pub plugin_name: &'a str,
    /// The rule's published caller-facing reason.
    pub message: &'a str,
}

fn protected(name: &str) -> bool {
    ["stop", "undo", "leave", "leave space", "leave-space"]
        .iter()
        .any(|protected| name.eq_ignore_ascii_case(protected))
}

pub(super) fn verify(
    manifest: &mut Manifest,
    options: &InstallOptions<'_>,
) -> Result<(), Diagnostic> {
    for refusal in &mut manifest.refusals {
        let (owner, target) = refusal
            .target
            .split_once('.')
            .expect("checked qualified operation");
        let name = target.split_once('.').map_or(target, |(_, name)| name);
        if protected(name) {
            return Err(refusal
                .site
                .error("This operation cannot be refused by a plugin"));
        }
        let target_plugin = options
            .installed
            .iter()
            .find(|target| target.manifest.id == owner && owner != manifest.id)
            .ok_or_else(|| {
                refusal
                    .site
                    .error("Refuse only an installed operation belonging to another plugin")
            })?;
        let implicit;
        let (kind, op) = if let Some(pair) = install::operation(target_plugin, target) {
            pair
        } else if let Some((kind_id, operation)) = target.split_once('.')
            && let Some(kind) = target_plugin.kinds.get(kind_id)
            && let Some(op) = implicit_operation(kind, operation)
        {
            implicit = op;
            (kind, &implicit)
        } else {
            return Err(refusal
                .site
                .error("The refusal target must name one installed operation"));
        };
        if kind.hidden {
            return Err(refusal
                .site
                .error("A refusal cannot inspect an operation on another plugin's hidden kind"));
        }
        if op.is_coded() && op.inputs.is_empty() {
            return Err(refusal
                .site
                .error("The refusal target has no typed operation inputs"));
        }
        formulas::refusal_condition(&refusal.condition.root, kind, op)?;
        refusal.resolved = format!("{owner}.{}.{name}", kind.id);
    }
    Ok(())
}

/// Evaluate enabled, installed rules in declaring plugin ID and file order.
/// The caller supplies the ordinary authorization and actor-ownership facts.
/// Formula failures return a diagnostic for fail-closed handling by the host.
pub fn evaluate_refusals<'a>(
    enabled: &[(&'a Manifest, &'a Refusal)],
    call: &RefusalCall<'_>,
) -> Result<Option<RefusalMatch<'a>>, Diagnostic> {
    if call.own_removal || call.kind.hidden || protected(call.operation) {
        return Ok(None);
    }
    let target = format!("{}.{}.{}", call.owner, call.kind.id, call.operation);
    let mut matched = enabled
        .iter()
        .copied()
        .filter(|(_, refusal)| refusal.resolved == target)
        .collect::<Vec<_>>();
    matched.sort_by_key(|(manifest, refusal)| {
        (manifest.id.as_str(), refusal.site.line, refusal.site.column)
    });
    let Some((_, first)) = matched.first() else {
        return Ok(None);
    };
    let fallback;
    let operation = if let Some(operation) = call.kind.operations.get(call.operation) {
        operation
    } else {
        fallback = implicit_operation(call.kind, call.operation).ok_or_else(|| {
            first
                .site
                .error("The checked refusal operation is no longer installed")
        })?;
        &fallback
    };
    let mut fields = Fields::new();
    if operation.name != "add"
        && let Some(target) = call.target
    {
        for (name, value) in target {
            if let Some(ty) = call
                .kind
                .fields
                .get(name)
                .and_then(|field| field.field_type.as_ref())
            {
                fields.insert(name.clone(), coerce(ty, value, first.site)?);
            }
        }
    }
    for (name, value) in call.inputs {
        if let Some(ty) = operation.refusal_input(call.kind, name) {
            fields.insert(name.clone(), coerce(ty, value, first.site)?);
        }
    }
    let metadata = Fields::new();
    let row = Row {
        fields: &fields,
        metadata: &metadata,
    };
    let mut steps = 50_000;
    for (manifest, refusal) in matched {
        if evaluate_in(&refusal.condition, row, &EvalContext::default(), &mut steps)?.truth() {
            return Ok(Some(RefusalMatch {
                plugin_id: &manifest.id,
                plugin_name: &manifest.name,
                message: &refusal.message,
            }));
        }
    }
    Ok(None)
}
